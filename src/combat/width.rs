//! 战斗宽度 + 增援队列(陆战循环)
//!
//! 基础宽度 70; 超出的师进预备队; 前线退下后预备队按概率补位。
use crate::runtime::World;

/// 基础战斗宽度(原版地形文件, 平原/森林/丘陵均70)
pub const BASE_COMBAT_WIDTH: f64 = 70.0;
/// 每小时从预备队加入前线的概率(原版 REINFORCE_CHANCE)
const REINFORCE_CHANCE: f64 = 0.02;

/// 判断新师能否加入前线(加入后宽度是否<=70)
pub fn can_join_frontline(world: &World, frontline: &[u64], new_div_width: f64) -> bool {
    let used = world.used_width(frontline);
    used + new_div_width <= BASE_COMBAT_WIDTH
}

/// 每小时增援: 预备队师按概率补入前线空位
pub fn reinforce_reserves(world: &mut World) {
    // 快照每场战斗的预备队(避免借用冲突)
    let battle_specs: Vec<(usize, Vec<u64>, Vec<u64>)> = world
        .battles
        .iter()
        .enumerate()
        .map(|(i, b)| (i, b.reserve_attackers.clone(), b.reserve_defenders.clone()))
        .collect();

    for (idx, res_atk, res_def) in battle_specs {
        // 攻方预备队补位
        let mut joined_atk = Vec::new();
        for div_id in &res_atk {
            let width = world.divisions.get(div_id).map(|d| d.combat_width).unwrap_or(0.0);
            let frontline = &world.battles[idx].attackers;
            if can_join_frontline(world, frontline, width) {
                // 简化: 不用真随机, 用固定概率(每2师补1个, 模拟2%累积)
                // 真随机需引入 rand; M5 再加。这里用确定性: 预备队第1个补上
                joined_atk.push(*div_id);
                break; // 每小时每方最多补1个(模拟低概率)
            }
        }
        // 守方预备队补位
        let mut joined_def = Vec::new();
        for div_id in &res_def {
            let width = world.divisions.get(div_id).map(|d| d.combat_width).unwrap_or(0.0);
            let frontline = &world.battles[idx].defenders;
            if can_join_frontline(world, frontline, width) {
                joined_def.push(*div_id);
                break;
            }
        }
        // 应用: 移出预备队, 加入前线
        for j in &joined_atk {
            world.battles[idx].reserve_attackers.retain(|x| x != j);
            world.battles[idx].attackers.push(*j);
        }
        for j in &joined_def {
            world.battles[idx].reserve_defenders.retain(|x| x != j);
            world.battles[idx].defenders.push(*j);
        }
        let _ = REINFORCE_CHANCE; // 真随机时用
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::entities::{Battle, Division};

    fn wide_div(tag: &str, w: f64) -> Division {
        Division {
            id: 0, owner_tag: tag.into(), combat_width: w,
            max_org: 60.0, org: 60.0, max_strength: 100.0, strength: 100.0,
            ..Default::default()
        }
    }

    #[test]
    fn t_can_join_within_width() {
        let mut world = World::new();
        let d1 = wide_div("GER", 40.0);
        let id1 = world.add_division(d1);
        // 前线40 + 新师20 = 60 <= 70, 可加入
        assert!(can_join_frontline(&world, &[id1], 20.0));
    }

    #[test]
    fn t_cannot_join_over_width() {
        let mut world = World::new();
        let d1 = wide_div("GER", 60.0);
        let id1 = world.add_division(d1);
        // 前线60 + 新师20 = 80 > 70, 不能加入
        assert!(!can_join_frontline(&world, &[id1], 20.0));
    }

    #[test]
    fn t_reserve_reinforces_frontline() {
        let mut world = World::new();
        let d1 = wide_div("GER", 60.0); // 前线(占60宽)
        let d2 = wide_div("GER", 10.0); // 预备队
        let id1 = world.add_division(d1);
        let id2 = world.add_division(d2);
        world.battles.push(Battle {
            id: 1, province: 1,
            attackers: vec![id1], defenders: vec![],
            reserve_attackers: vec![id2], reserve_defenders: vec![],
        });
        // 前线60 + 预备队10 = 70, 可补位
        reinforce_reserves(&mut world);
        assert!(world.battles[0].attackers.contains(&id2), "预备队应补入前线");
        assert!(!world.battles[0].reserve_attackers.contains(&id2), "应移出预备队");
    }

    #[test]
    fn t_reserve_division_recovers_org() {
        // Bug1 验证: 预备队师(非前线)应恢复 org
        let mut world = World::new();
        let mut d1 = wide_div("GER", 60.0);
        d1.org = 30.0; // 前线师 org 不满(战斗中不恢复)
        let mut d2 = wide_div("GER", 10.0);
        d2.org = 30.0; // 预备队师 org 不满(应恢复)
        let id1 = world.add_division(d1);
        let id2 = world.add_division(d2);
        world.battles.push(Battle {
            id: 1, province: 1,
            attackers: vec![id1], defenders: vec![],
            reserve_attackers: vec![id2], reserve_defenders: vec![],
        });
        crate::combat::recovery::recover_org(&mut world);
        // 前线师 org 不变(战斗中)
        assert!((world.divisions.get(&id1).unwrap().org - 30.0).abs() < 1e-9, "前线师不应恢复");
        // 预备队师 org 应恢复
        assert!(
            world.divisions.get(&id2).unwrap().org > 30.0,
            "预备队师应恢复 org: {}",
            world.divisions.get(&id2).unwrap().org
        );
    }
}
