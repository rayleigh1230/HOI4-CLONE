//! 行军: 师在省份间移动(陆战循环)
//!
//! 三种移动(由 OrderState 表示):
//! - 普通移动(绿): Moving{hostile=false}, 目标无敌军, 正常推进
//! - 进攻移动(红): Moving{hostile=true}, 战斗+移动并行, 速度×0.33
//! - 撤退: Retreating{..}, 脱离战斗, 速度+25%, 对其他系统不可见
use crate::runtime::entities::OrderState;
use crate::runtime::World;

/// 每小时移动进度基准(约20小时到达一个省, 让行军过程可见)
const MOVE_RATE: f64 = 0.05;
const RETREAT_SPEED_BONUS: f64 = 0.25;
/// 进攻移动(战斗中)速度系数(原版 COMBAT_MOVEMENT_SPEED)
const COMBAT_MOVEMENT_SPEED: f64 = 0.33;
/// 占领省份时 org 损失比例(原版 ORG_LOSS_FACTOR_ON_CONQUER)
const ORG_LOSS_ON_CONQUER: f64 = 0.2;

/// 每小时检查: Moving/Pending 的师, 目标地块有敌军 → 立刻开战
/// (交战由"地块有无敌军"决定, 非到达决定)
///
/// 重要: Retreating 的师被完全忽略 — 不当攻方也不当守方。
/// 撤退师对其他战斗系统不可见(防止每 tick 被重新拉入战斗, org 归零后 str 持续掉直至歼灭)。
///
/// Pending 的师也参与判定: 它们已到达目标省待命(如撤退师到达敌方省后变攻方),
/// 若目标省有敌军 → 开战。
pub fn check_engagements(world: &mut World) {
    // 收集需要检查的师 (id, dest, owner) — 只看 Moving/Pending, 跳过 Retreating/Idle/Supporting
    let candidates: Vec<(u64, u32, String)> = world.divisions.iter()
        .filter_map(|(id, d)| {
            let dest = match d.order {
                OrderState::Moving { dest, .. } => dest,
                OrderState::Pending { dest } => dest,
                _ => return None, // Retreating/Idle/Supporting 不主动开战
            };
            Some((*id, dest, d.owner_tag.clone()))
        })
        .collect();
    // 已在战斗中的师(不重复触发)
    let in_battle: std::collections::HashSet<u64> = world.battles.iter()
        .flat_map(|b| b.attackers.iter().chain(b.defenders.iter())
            .chain(b.reserve_attackers.iter()).chain(b.reserve_defenders.iter()).copied())
        .collect();

    for (div_id, dest, owner) in candidates {
        if in_battle.contains(&div_id) {
            continue; // 已在战斗中
        }
        // 查目标地块有无敌军师 — 排除撤退师(撤退师不当守方被重新拉入)
        let enemies: Vec<u64> = world.divisions.values()
            .filter(|od| od.location_province == dest && od.owner_tag != owner
                && !od.is_annihilated() && !od.is_withdrawing())
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

/// 清理支援攻击: 若支援目标省的战斗已结束(不在 world.battles) → 转 Idle。
/// 对应规则7"如果没战斗支援攻击就自动取消"。
/// 放在 check_engagements 之后、resolve 之前, 让战斗已结束的支援师在本 tick
/// resolve 时不再被当攻方(避免它已被移出 battle 但还在 Supporting 的瞬态)。
pub fn cancel_finished_supports(world: &mut World) {
    let active_provinces: std::collections::HashSet<u32> = world.battles.iter()
        .map(|b| b.province).collect();
    for d in world.divisions.values_mut() {
        if let OrderState::Supporting { target } = d.order {
            if !active_provinces.contains(&target) {
                d.order = OrderState::Idle;
            }
        }
    }
}

/// 推进所有正在移动的师(每小时调用)
pub fn advance_movement(world: &mut World) {
    // 收集所有 Moving/Retreating 的师(需要推进进度)
    let moving_ids: Vec<u64> = world
        .divisions
        .iter()
        .filter_map(|(id, d)| {
            matches!(d.order, OrderState::Moving { .. } | OrderState::Retreating { .. }).then_some(*id)
        })
        .collect();

    // 第一阶段: 推进进度; 进度满的师收集"到达候选"(快照模式避免借用冲突)
    struct Arrival { id: u64, dest: u32, owner: String, remaining: Vec<u32> }
    enum ArrivalDecision {
        // 到达非己方空省 → 直接占领
        Capture(Arrival),
        // Moving 组: 进入 Pending 等战斗(目标省有战斗/敌军), location 不改
        Pending { id: u64, dest: u32 },
        // Retreating 组到达敌方有敌军省: 强制归属(location=dest) + 进入战场
        RetreatIntoEnemy { id: u64, dest: u32 },
    }
    let mut decisions: Vec<ArrivalDecision> = Vec::new();
    {
        // 规则3: "自身未处于任何战场"才能变更归属地。
        // 收集所有在战场里的师 id(自身作为攻/守/预备参与战斗的师)。
        let in_battle: std::collections::HashSet<u64> = world.battles.iter()
            .flat_map(|b| b.attackers.iter().chain(b.defenders.iter())
                .chain(b.reserve_attackers.iter()).chain(b.reserve_defenders.iter()).copied())
            .collect();

        // 第一阶段a: 推进进度, 收集到达候选 (id, dest, owner, was_retreat, remaining)
        // 进度条是物理移动, 照常推进(战斗中只是变慢, hostile×0.33)。
        let mut arrived: Vec<(u64, u32, String, bool, Vec<u32>)> = Vec::new();
        for id in moving_ids {
            let Some(d) = world.divisions.get_mut(&id) else { continue };
            // 按状态决定速度系数
            let rate = match d.order {
                OrderState::Retreating { .. } => MOVE_RATE * (1.0 + RETREAT_SPEED_BONUS),
                OrderState::Moving { hostile: true, .. } => MOVE_RATE * COMBAT_MOVEMENT_SPEED,
                OrderState::Moving { hostile: false, .. } => MOVE_RATE,
                _ => continue,
            };
            let reached = match d.order {
                OrderState::Moving { ref mut progress, .. } => {
                    *progress += rate;
                    *progress >= 1.0
                }
                OrderState::Retreating { ref mut progress, .. } => {
                    *progress += rate;
                    *progress >= 1.0
                }
                _ => false,
            };
            if reached {
                // 规则3: Moving 师自身在战场里 → 进度满也不变更归属地。
                // 保持 Moving 状态(进度满), 等战斗结束(师离开 battle 列表)后下 tick 再结算。
                // Retreating 师对战场不可见(不在 battle 列表), 不受此限制。
                if in_battle.contains(&id) && d.is_moving() {
                    continue; // 不收集为到达候选, 进度保持满值
                }
                // 取出 dest + owner + 是否撤退 + 剩余路径, 把状态置为 Idle(后续第二阶段根据判定改写)
                let (dest, owner, was_retreat, remaining) = match d.order {
                    OrderState::Moving { dest, ref remaining, .. } =>
                        (dest, d.owner_tag.clone(), false, remaining.clone()),
                    OrderState::Retreating { dest, .. } =>
                        (dest, d.owner_tag.clone(), true, Vec::new()),
                    _ => continue,
                };
                d.order = OrderState::Idle; // 临时置 Idle, 第二阶段再决定
                arrived.push((id, dest, owner, was_retreat, remaining));
            }
        }
        // 第一阶段b: 对每个到达候选判定(此时 d 借用已释放, 可查 world)
        for (id, dest, owner, was_retreat, remaining) in arrived {
            let dest_has_battle = world.battles.iter().any(|b| b.province == dest);
            // 无正在进行的战斗 → 查目标省有无敌军部队(规则1: 同省异国师立刻开战)
            let has_enemies = world.divisions.values()
                .any(|od| od.location_province == dest && od.owner_tag != owner
                    && !od.is_annihilated());
            if was_retreat {
                // Retreating 组到达(独立判定, 规则见 order-state-semantics.md):
                //   - 己方/敌方无敌军 → Capture(归属 + Idle; 敌方空省占领)
                //   - 敌方有敌军(或有战斗) → RetreatIntoEnemy(强制归属 + 进入战场)
                if has_enemies || dest_has_battle {
                    decisions.push(ArrivalDecision::RetreatIntoEnemy { id, dest });
                } else {
                    decisions.push(ArrivalDecision::Capture(Arrival { id, dest, owner, remaining }));
                }
            } else {
                // Moving 组到达:
                //   - 有战斗/敌军 → Pending(location 不改, 规则3)
                //   - 无战斗 + 无敌军 → Capture(占领)
                if dest_has_battle || has_enemies {
                    decisions.push(ArrivalDecision::Pending { id, dest });
                } else {
                    decisions.push(ArrivalDecision::Capture(Arrival { id, dest, owner, remaining }));
                }
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
                    d.order = OrderState::Idle; // 临时, 下面续走可能覆盖
                }
                // 【决策5】检查路径剩余: remaining 非空 → 续走下一段
                if !a.remaining.is_empty() {
                    let next = a.remaining[0];
                    let new_remaining = a.remaining[1..].to_vec();
                    if let Some(d) = world.divisions.get_mut(&a.id) {
                        let hostile = world.provinces.get(&next)
                            .map(|p| p.controller != a.owner)
                            .unwrap_or(false);
                        d.order = OrderState::Moving {
                            dest: next, progress: 0.0,
                            hostile, origin: a.dest, // 出发地 = 刚占领的省
                            remaining: new_remaining,
                        };
                        // 续走时不在此开战, 交给下一 tick 的 check_engagements
                    }
                }
                arrivals.push(a);
            }
            ArrivalDecision::Pending { id, dest } => {
                if let Some(d) = world.divisions.get_mut(&id) {
                    // 规则3: Pending 时 location 不变(师在战场, 归属地保持上一个占领省)。
                    // 师向 dest 有进度箭头(UI), 但归属地仍是出发地。
                    // 战斗胜后由第四阶段改 location; 战败则从当前归属地撤退。
                    d.order = OrderState::Pending { dest };
                }
            }
            ArrivalDecision::RetreatIntoEnemy { id, dest } => {
                // Retreating 组到达敌方有敌军省: 强制归属(location=dest) + 进入战场
                // (撤退组的独立规则: 即便没占领, 归属地也带过来, 当攻方开战)
                if let Some(d) = world.divisions.get_mut(&id) {
                    d.location_province = dest;
                    d.order = OrderState::Pending { dest };
                }
                // 开战由下一 tick 的 check_engagements 处理(师已是 Pending, location=dest)
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
    // 第四阶段: 检查 Pending 的师(进度满+等战斗胜)
    // 如果目标省已无战斗 且 无敌军(敌人全撤/歼灭) → 真正到达(改location+占领)
    // 注意: 必须同时检查"无敌军", 否则刚进 Pending 的师(战斗还没被
    //       check_engagements 创建)会被误判为"战斗已结束"而错误占领。
    let pending: Vec<u64> = world.divisions.iter()
        .filter_map(|(id, d)| d.is_pending().then_some(*id))
        .collect();
    for id in pending {
        // 快照决策所需只读信息(dest, owner), 避免与后续 get_mut 借用冲突
        let (dest, owner) = match world.divisions.get(&id) {
            Some(d) => match d.pending_dest() {
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
            d.order = OrderState::Idle;
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
    use crate::runtime::entities::{Division, OrderState};

    #[test]
    fn t_division_moves_to_destination() {
        let mut w = World::new();
        let d = Division {
            id: 0, owner_tag: "GER".into(), location_province: 1,
            order: OrderState::Moving { dest: 2, progress: 0.0, hostile: false, origin: 1, remaining: vec![] },
            ..Default::default()
        };
        let did = w.add_division(d);
        // MOVE_RATE=0.05, 19次=95%, 第20次到达
        for _ in 0..19 {
            advance_movement(&mut w);
        }
        assert!((w.divisions.get(&did).unwrap().move_progress() - 0.95).abs() < 1e-9);
        assert_eq!(w.divisions.get(&did).unwrap().location_province, 1, "未到不应换省");
        advance_movement(&mut w);
        assert_eq!(w.divisions.get(&did).unwrap().location_province, 2);
        assert!(w.divisions.get(&did).unwrap().is_idle(), "到达后应转 Idle");
    }

    #[test]
    fn t_retreat_moves_faster() {
        let mut w = World::new();
        let d1 = Division {
            id: 0, owner_tag: "X".into(), location_province: 1,
            order: OrderState::Moving { dest: 2, progress: 0.0, hostile: false, origin: 1, remaining: vec![] },
            ..Default::default()
        };
        let d2 = Division {
            id: 0, owner_tag: "X".into(), location_province: 1,
            order: OrderState::Retreating { dest: 2, progress: 0.0 },
            ..Default::default()
        };
        let id1 = w.add_division(d1);
        let id2 = w.add_division(d2);
        advance_movement(&mut w);
        let p1 = w.divisions.get(&id1).unwrap().move_progress();
        let p2 = w.divisions.get(&id2).unwrap().move_progress();
        assert!(p2 > p1, "撤退应更快: normal={p1} retreat={p2}");
    }

    #[test]
    fn t_attack_move_slower() {
        let mut w = World::new();
        let d1 = Division {
            id: 0, owner_tag: "X".into(), location_province: 1,
            order: OrderState::Moving { dest: 2, progress: 0.0, hostile: false, origin: 1, remaining: vec![] },
            ..Default::default()
        };
        let d2 = Division {
            id: 0, owner_tag: "X".into(), location_province: 1,
            order: OrderState::Moving { dest: 2, progress: 0.0, hostile: true, origin: 1, remaining: vec![] },
            ..Default::default()
        };
        let id1 = w.add_division(d1);
        let id2 = w.add_division(d2);
        advance_movement(&mut w);
        let p1 = w.divisions.get(&id1).unwrap().move_progress();
        let p2 = w.divisions.get(&id2).unwrap().move_progress();
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
            order: OrderState::Moving { dest: 2, progress: 0.99, hostile: true, origin: 1, remaining: vec![] },
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

    /// 师A(GER)归属省1, 正在进攻省2(Moving dest=2, location仍=1);
    /// 师B(FRA)从省3向省1进军 → 省1应爆发战斗, A 应自动成为省1的**防守方**。
    #[test]
    fn t_p2_division_defends_own_province_while_attacking_elsewhere() {
        let mut w = World::new();
        // 师A: GER, 归属省1, 正进攻省2
        let mut a = live_div();
        a.owner_tag = "GER".into();
        a.location_province = 1;
        a.order = OrderState::Moving { dest: 2, progress: 0.0, hostile: true, origin: 1, remaining: vec![] };
        let a = w.add_division(a);
        // 师B: FRA, 在省3, 向省1进军
        let mut b = live_div();
        b.owner_tag = "FRA".into();
        b.location_province = 3;
        b.order = OrderState::Moving { dest: 1, progress: 0.0, hostile: true, origin: 3, remaining: vec![] };
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
        a.order = OrderState::Moving { dest: 2, progress: 0.0, hostile: true, origin: 1, remaining: vec![] };
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
        b.order = OrderState::Moving { dest: 1, progress: 0.0, hostile: true, origin: 3, remaining: vec![] };
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

    // ===== 多段路径: 占领中途省后续走(决策5)=====

    /// 3 省链 1-2-3, 省2 是敌方空省(待占领), 省3 己方
    fn chain_1_2_3_world() -> World {
        let mut w = World::new();
        w.provinces.insert(1, crate::runtime::Province {
            id: 1, owner: "GER".into(), controller: "GER".into(),
            terrain: "plains".into(), neighbors: vec![2],
        });
        w.provinces.insert(2, crate::runtime::Province {
            id: 2, owner: "FRA".into(), controller: "FRA".into(),
            terrain: "plains".into(), neighbors: vec![1, 3],
        });
        w.provinces.insert(3, crate::runtime::Province {
            id: 3, owner: "GER".into(), controller: "GER".into(),
            terrain: "plains".into(), neighbors: vec![2],
        });
        w
    }

    #[test]
    fn t_multihop_continues_after_mid_capture() {
        // 师从省1 Moving 到省2(remaining=[3]), 占领省2 后应续走到省3
        let mut w = chain_1_2_3_world();
        let d = Division {
            id: 0, owner_tag: "GER".into(), location_province: 1,
            order: OrderState::Moving {
                dest: 2, progress: 0.99, hostile: true, origin: 1, remaining: vec![3],
            },
            max_org: 60.0, org: 60.0, max_strength: 20.0, strength: 20.0,
            ..Default::default()
        };
        let did = w.add_division(d);
        // 第 1 tick: 到达省2(progress 满)→ 占领 → 续走设 dest=3
        advance_movement(&mut w);
        let div = w.divisions.get(&did).unwrap();
        assert_eq!(div.location_province, 2, "占领省2 后 location 应更新为 2");
        assert!(div.is_moving(), "应续走(仍是 Moving)");
        assert_eq!(div.move_dest(), Some(3), "dest 应切到省3");
        assert_eq!(div.move_progress(), 0.0, "续走进度归零");
    }
}
