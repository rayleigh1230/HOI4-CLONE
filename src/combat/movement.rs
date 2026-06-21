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

/// 每小时检查: 移动中的师, 目标地块出现敌军 → 立刻开战
/// (交战由"地块有无敌军"决定, 非到达决定)
pub fn check_engagements(world: &mut World) {
    // 收集需要检查的师 (id, dest, owner)
    let moving: Vec<(u64, u32, String)> = world.divisions.iter()
        .filter_map(|(id, d)| {
            d.destination.map(|dest| (*id, dest, d.owner_tag.clone()))
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
        // 查目标地块有无敌军师
        let enemies: Vec<u64> = world.divisions.values()
            .filter(|od| od.location_province == dest && od.owner_tag != owner && !od.is_annihilated())
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

/// 推进所有正在移动的师(每小时调用)
pub fn advance_movement(world: &mut World) {
    let moving: Vec<u64> = world
        .divisions
        .iter()
        .filter_map(|(id, d)| d.destination.map(|_| *id))
        .collect();

    // 第一阶段: 推进进度, 收集到达的师(释放借用)
    struct Arrival { id: u64, dest: u32, owner: String, was_attacking: bool }
    let mut arrivals: Vec<Arrival> = Vec::new();
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
                d.location_province = dest;
                d.move_progress = 0.0;
                let was_attacking = d.attacking;
                d.attacking = false;
                arrivals.push(Arrival {
                    id, dest, owner: d.owner_tag.clone(), was_attacking,
                });
            }
        }
    }
    // 第二阶段: 到达后占领(交战由 check_engagements 每小时统一判定)
    for a in arrivals {
        // 地块非己方 → 占领(先到先占; 竞速场景)
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
}
