//! 行军: 师在省份间移动(陆战循环)
//!
//! 三种移动:
//! - 普通移动(绿): 目标无敌军, 正常推进
//! - 进攻移动(红, attacking): 战斗+移动并行, 速度×0.33
//! - 撤退(retreating): 脱离战斗, 速度+25%
use crate::runtime::World;

/// 每小时移动进度基准(约20小时到达一个省, 让行军过程可见)
const MOVE_RATE: f64 = 0.05;
const RETREAT_SPEED_BONUS: f64 = 0.25;
/// 进攻移动(战斗中)速度系数(原版 COMBAT_MOVEMENT_SPEED)
const COMBAT_MOVEMENT_SPEED: f64 = 0.33;
/// 占领省份时 org 损失比例(原版 ORG_LOSS_FACTOR_ON_CONQUER)
const ORG_LOSS_ON_CONQUER: f64 = 0.2;

/// 每小时检查: 移动中 或 pending_arrival 的师, 目标地块有敌军 → 立刻开战
/// (交战由"地块有无敌军"决定, 非到达决定)
///
/// 重要: 撤退中(retreating)的师被完全忽略 — 不当攻方也不当守方。
/// 撤退师 location 仍可能在战场省(行军未到达撤退目标), 若不过滤会被每 tick
/// 重新拉入战斗, 导致 org 归零后 str 持续下降直至歼灭(用户报告的 bug)。
///
/// pending_arrival 的师也参与判定: 它们已到达目标省待命(如撤退师到达敌方省后
/// 变攻方), 若目标省有敌军 → 开战。
pub fn check_engagements(world: &mut World) {
    // 收集需要检查的师 (id, dest, owner) — 跳过撤退师(撤退 = 强制脱离战斗)
    // dest 来自 destination(行军中) 或 pending_arrival(已到达待命)
    let moving: Vec<(u64, u32, String)> = world.divisions.iter()
        .filter_map(|(id, d)| {
            if d.retreating { return None; } // 撤退师不主动开战
            let dest = d.destination.or(d.pending_arrival)?;
            Some((*id, dest, d.owner_tag.clone()))
        })
        .collect();
    // 已在战斗中的师(不重复触发)
    let in_battle: std::collections::HashSet<u64> = world.battles.iter()
        .flat_map(|b| b.attackers.iter().chain(b.defenders.iter())
            .chain(b.reserve_attackers.iter()).chain(b.reserve_defenders.iter()).copied())
        .collect();

    for (div_id, dest, owner) in moving {
        if in_battle.contains(&div_id) {
            continue; // 已在战斗中
        }
        // 查目标地块有无敌军师 — 排除撤退师(撤退师不当守方被重新拉入)
        let enemies: Vec<u64> = world.divisions.values()
            .filter(|od| od.location_province == dest && od.owner_tag != owner
                && !od.is_annihilated() && !od.retreating)
            .map(|od| od.id)
            .collect();
        if enemies.is_empty() {
            continue;
        }
        // 有敌军 → 开战
        let existing = world.battles.iter().position(|b| b.province == dest);
        if let Some(bidx) = existing {
            world.battles[bidx].attackers.push(div_id);
        } else {
            let bid = world.next_battle_id;
            world.next_battle_id += 1;
            world.battles.push(crate::runtime::entities::Battle {
                id: bid, province: dest,
                attackers: vec![div_id], defenders: enemies,
                ..Default::default()
            });
        }
    }
}

/// 清理支援攻击: 若支援目标省的战斗已结束(不在 world.battles) → 清 supporting。
/// 对应规则7"如果没战斗支援攻击就自动取消"。
/// 放在 check_engagements 之后、resolve 之前, 让战斗已结束的支援师在本 tick
/// resolve 时不再被当攻方(避免它已被移出 battle 但 supporting 还在的瞬态)。
pub fn cancel_finished_supports(world: &mut World) {
    let active_provinces: std::collections::HashSet<u32> = world.battles.iter()
        .map(|b| b.province).collect();
    for d in world.divisions.values_mut() {
        if let Some(t) = d.supporting {
            if !active_provinces.contains(&t) {
                d.supporting = None;
            }
        }
    }
}

/// 推进所有正在移动的师(每小时调用)
pub fn advance_movement(world: &mut World) {
    let moving: Vec<u64> = world
        .divisions
        .iter()
        .filter_map(|(id, d)| d.destination.map(|_| *id))
        .collect();

    // 第一阶段: 推进进度; 进度满的师收集"到达候选"(快照模式避免借用冲突)
    // 到达候选: (id, dest, owner, was_retreating)
    struct Arrival { id: u64, dest: u32, owner: String }
    enum ArrivalDecision {
        // 到达非己方空省 → 直接占领
        Capture(Arrival),
        // 进入 pending_arrival 等战斗(目标省有正在进行的战斗 或 有敌军)
        Pending { id: u64, dest: u32, clear_retreat: bool },
    }
    let mut decisions: Vec<ArrivalDecision> = Vec::new();
    {
        // 第一阶段a: 推进进度 + take destination, 收集到达候选
        // 到达判定需要查 world.divisions(敌军)和 world.battles, 故只在此块内推进+收集
        // 决策所需只读信息, 实际写回留到块外
        let mut arrived: Vec<(u64, u32, String, bool)> = Vec::new(); // (id, dest, owner, was_retreat)
        for id in moving {
            let Some(d) = world.divisions.get_mut(&id) else { continue };
            let rate = if d.retreating {
                MOVE_RATE * (1.0 + RETREAT_SPEED_BONUS)
            } else if d.attacking {
                MOVE_RATE * COMBAT_MOVEMENT_SPEED
            } else {
                MOVE_RATE
            };
            d.move_progress += rate;
            if d.move_progress >= 1.0 {
                if let Some(dest) = d.destination.take() {
                    d.move_progress = 0.0;
                    d.attacking = false;
                    let owner = d.owner_tag.clone();
                    let was_retreat = d.retreating;
                    arrived.push((id, dest, owner, was_retreat));
                }
            }
        }
        // 第一阶段b: 对每个到达候选判定(此时 d 借用已释放, 可查 world)
        for (id, dest, owner, was_retreat) in arrived {
            let dest_has_battle = world.battles.iter().any(|b| b.province == dest);
            if dest_has_battle {
                decisions.push(ArrivalDecision::Pending { id, dest, clear_retreat: false });
                continue;
            }
            // 无正在进行的战斗 → 查目标省有无敌军部队(规则1: 同省异国师立刻开战)
            let has_enemies = world.divisions.values()
                .any(|od| od.location_province == dest && od.owner_tag != owner
                    && !od.is_annihilated());
            if has_enemies {
                // 目标省有敌军 → 不能直接占领。
                // 撤退师到达敌方省: 即将变攻方, 清撤退状态让 check_engagements 开战。
                // 普通师: 同样进 pending 等战斗。
                decisions.push(ArrivalDecision::Pending { id, dest, clear_retreat: was_retreat });
            } else {
                // 无战斗 + 无敌军 → 到达(结算归属)
                decisions.push(ArrivalDecision::Capture(Arrival { id, dest, owner }));
            }
        }
    }
    // 第二阶段: 应用到达决策
    let mut arrivals: Vec<Arrival> = Vec::new();
    for dec in decisions {
        match dec {
            ArrivalDecision::Capture(a) => {
                if let Some(d) = world.divisions.get_mut(&a.id) {
                    d.location_province = a.dest;
                }
                arrivals.push(a);
            }
            ArrivalDecision::Pending { id, dest, clear_retreat } => {
                if let Some(d) = world.divisions.get_mut(&id) {
                    d.pending_arrival = Some(dest);
                    if clear_retreat {
                        d.retreating = false; // 不再撤退, 即将变攻方
                    }
                }
            }
        }
    }
    // 第三阶段: 结算到达(占领非己方地块)
    for a in arrivals {
        let is_own = world.provinces.get(&a.dest)
            .map(|p| p.controller == a.owner)
            .unwrap_or(false);
        if !is_own {
            if let Some(p) = world.provinces.get_mut(&a.dest) {
                p.controller = a.owner.clone();
                p.owner = a.owner;
            }
            if let Some(d) = world.divisions.get_mut(&a.id) {
                d.org = (d.org - d.max_org * ORG_LOSS_ON_CONQUER).max(0.0);
            }
        }
    }
    // 第四阶段: 检查 pending_arrival 的师(进度满+等战斗胜)
    // 如果目标省已无战斗 且 无敌军(敌人全撤/歼灭) → 真正到达(改location+占领)
    // 注意: 必须同时检查"无敌军", 否则刚进 pending 的师(战斗还没被
    //       check_engagements 创建)会被误判为"战斗已结束"而错误占领。
    let pending: Vec<u64> = world.divisions.iter()
        .filter_map(|(id, d)| d.pending_arrival.map(|_| *id))
        .collect();
    for id in pending {
        // 快照决策所需只读信息(dest, owner), 避免与后续 get_mut 借用冲突
        let (dest, owner) = match world.divisions.get(&id) {
            Some(d) => match d.pending_arrival {
                Some(p) => (p, d.owner_tag.clone()),
                None => continue,
            },
            None => continue,
        };
        let dest_has_battle = world.battles.iter().any(|b| b.province == dest);
        if dest_has_battle {
            continue; // 战斗进行中, 继续等
        }
        // 无正在进行的战斗 → 查目标省有无敌军部队
        let has_enemies = world.divisions.values()
            .any(|od| od.location_province == dest && od.owner_tag != owner
                && !od.is_annihilated());
        if has_enemies {
            continue; // 有敌军但战斗未开(等 check_engagements 下tick开战), 不占领
        }
        // 无战斗 + 无敌军 → 到达结算(敌人全撤/歼灭, 攻方占领)
        let is_own = world.provinces.get(&dest)
            .map(|p| p.controller == owner)
            .unwrap_or(false);
        if let Some(d) = world.divisions.get_mut(&id) {
            d.pending_arrival = None;
            d.location_province = dest;
        }
        if !is_own {
            if let Some(p) = world.provinces.get_mut(&dest) {
                p.controller = owner.clone();
                p.owner = owner;
            }
            if let Some(d) = world.divisions.get_mut(&id) {
                d.org = (d.org - d.max_org * ORG_LOSS_ON_CONQUER).max(0.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::entities::Division;

    #[test]
    fn t_division_moves_to_destination() {
        let mut w = World::new();
        let d = Division {
            id: 0, owner_tag: "GER".into(), location_province: 1,
            destination: Some(2), move_progress: 0.0,
            ..Default::default()
        };
        let did = w.add_division(d);
        // MOVE_RATE=0.05, 19次=95%, 第20次到达
        for _ in 0..19 {
            advance_movement(&mut w);
        }
        assert!((w.divisions.get(&did).unwrap().move_progress - 0.95).abs() < 1e-9);
        assert_eq!(w.divisions.get(&did).unwrap().location_province, 1, "未到不应换省");
        advance_movement(&mut w);
        assert_eq!(w.divisions.get(&did).unwrap().location_province, 2);
        assert!(w.divisions.get(&did).unwrap().destination.is_none());
    }

    #[test]
    fn t_retreat_moves_faster() {
        let mut w = World::new();
        let d1 = Division {
            id: 0, owner_tag: "X".into(), location_province: 1,
            destination: Some(2), move_progress: 0.0, retreating: false,
            ..Default::default()
        };
        let d2 = Division {
            id: 0, owner_tag: "X".into(), location_province: 1,
            destination: Some(2), move_progress: 0.0, retreating: true,
            ..Default::default()
        };
        let id1 = w.add_division(d1);
        let id2 = w.add_division(d2);
        advance_movement(&mut w);
        let p1 = w.divisions.get(&id1).unwrap().move_progress;
        let p2 = w.divisions.get(&id2).unwrap().move_progress;
        assert!(p2 > p1, "撤退应更快: normal={p1} retreat={p2}");
    }

    #[test]
    fn t_attack_move_slower() {
        let mut w = World::new();
        let d1 = Division {
            id: 0, owner_tag: "X".into(), location_province: 1,
            destination: Some(2), move_progress: 0.0, attacking: false,
            ..Default::default()
        };
        let d2 = Division {
            id: 0, owner_tag: "X".into(), location_province: 1,
            destination: Some(2), move_progress: 0.0, attacking: true,
            ..Default::default()
        };
        let id1 = w.add_division(d1);
        let id2 = w.add_division(d2);
        advance_movement(&mut w);
        let p1 = w.divisions.get(&id1).unwrap().move_progress;
        let p2 = w.divisions.get(&id2).unwrap().move_progress;
        assert!(p2 < p1, "进攻移动应更慢: normal={p1} attack={p2}");
    }

    #[test]
    fn t_conquering_loses_org() {
        let mut w = World::new();
        w.provinces.insert(2, crate::runtime::Province {
            id: 2, owner: "FRA".into(), controller: "FRA".into(),
            terrain: "plains".into(), neighbors: vec![1],
        });
        let d = Division {
            id: 0, owner_tag: "GER".into(), location_province: 1,
            destination: Some(2), move_progress: 0.99, attacking: true,
            max_org: 60.0, org: 60.0,
            ..Default::default()
        };
        let did = w.add_division(d);
        advance_movement(&mut w); // 到达
        let div = w.divisions.get(&did).unwrap();
        assert_eq!(div.location_province, 2);
        assert!(div.org < 60.0, "占领应掉org: {}", div.org);
        // 省份归 GER
        assert_eq!(w.provinces.get(&2).unwrap().controller, "GER");
    }

    // ===== P2: 地块被进攻 → 归属地师自动成防守方(即使该师正进攻别处) =====

    /// 活着的步兵师(strength>0 否则被 is_annihilated 过滤)
    fn live_div() -> Division {
        Division {
            max_strength: 20.0, strength: 20.0,
            max_org: 60.0, org: 60.0,
            ..Default::default()
        }
    }

    /// 师A(GER)归属省1, 正在进攻省2(destination=2, location仍=1);
    /// 师B(FRA)从省3向省1进军 → 省1应爆发战斗, A 应自动成为省1的**防守方**。
    #[test]
    fn t_p2_division_defends_own_province_while_attacking_elsewhere() {
        let mut w = World::new();
        // 师A: GER, 归属省1, 正进攻省2
        let mut a = live_div();
        a.owner_tag = "GER".into();
        a.location_province = 1;
        a.destination = Some(2);
        a.origin_province = 1;
        a.attacking = true;
        let a = w.add_division(a);
        // 师B: FRA, 在省3, 向省1进军
        let mut b = live_div();
        b.owner_tag = "FRA".into();
        b.location_province = 3;
        b.destination = Some(1);
        b.origin_province = 3;
        b.attacking = true;
        let b = w.add_division(b);

        check_engagements(&mut w);

        // 省1 应有一场战斗
        let battle1 = w.battles.iter().find(|bl| bl.province == 1);
        assert!(battle1.is_some(), "省1应爆发战斗(B向省1进军)");
        let bl = battle1.unwrap();
        // B 是省1战斗的攻方(向省1进军)
        assert!(bl.attackers.contains(&b), "B应是省1战斗攻方, attackers={:?}", bl.attackers);
        // A 是省1战斗的守方(归属省1, 即使正在进攻省2)
        assert!(
            bl.defenders.contains(&a),
            "A(归属省1)应自动成省1防守方, defenders={:?}", bl.defenders
        );
    }

    /// A 进攻省2 的战斗不应被破坏(A 仍是省2的攻方)。
    #[test]
    fn t_p2_original_attack_uninterrupted() {
        let mut w = World::new();
        let mut a = live_div();
        a.owner_tag = "GER".into();
        a.location_province = 1;
        a.destination = Some(2);
        a.origin_province = 1;
        a.attacking = true;
        let a = w.add_division(a);
        // C(FRA) 在省2防守 → A vs C 战斗(省2)
        let mut c = live_div();
        c.owner_tag = "FRA".into();
        c.location_province = 2;
        let c = w.add_division(c);
        // B(FRA) 从省3 进军省1 → 触发省1战斗, A 成省1守方
        let mut b = live_div();
        b.owner_tag = "FRA".into();
        b.location_province = 3;
        b.destination = Some(1);
        b.origin_province = 3;
        b.attacking = true;
        let b = w.add_division(b);

        check_engagements(&mut w);

        // 省2 战斗: A 仍是攻方
        let battle2 = w.battles.iter().find(|bl| bl.province == 2);
        assert!(battle2.is_some(), "省2战斗应存在(A进攻C)");
        let bl2 = battle2.unwrap();
        assert!(bl2.attackers.contains(&a), "A仍是省2攻方");
        assert!(bl2.defenders.contains(&c), "C是省2守方");

        // 省1 战斗: A 是守方(同时打两场)
        let battle1 = w.battles.iter().find(|bl| bl.province == 1);
        assert!(battle1.is_some(), "省1战斗应存在(B进攻省1)");
        let bl1 = battle1.unwrap();
        assert!(bl1.attackers.contains(&b), "B是省1攻方");
        assert!(bl1.defenders.contains(&a), "A同时是省1守方(状态共享, 多战场)");

        // A 同时出现在两场战斗中
        let a_in_battles = w.battles.iter()
            .filter(|bl| bl.attackers.contains(&a) || bl.defenders.contains(&a))
            .count();
        assert_eq!(a_in_battles, 2, "A应同时参与两场战斗");
    }
}
