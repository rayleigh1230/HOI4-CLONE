//! 战斗宽度 + 增援队列(陆战循环)
//!
//! 基础宽度 70; 超出的师进预备队; 前线退下后预备队按概率补位。
use crate::runtime::World;

/// 基础战斗宽度(原版地形文件, 平原/森林/丘陵均70)
pub const BASE_COMBAT_WIDTH: f64 = 70.0;
/// 每小时从预备队加入前线的概率(原版 REINFORCE_CHANCE)
const REINFORCE_CHANCE: f64 = 0.02;

/// 简单确定性伪随机: 基于(hour, div_id)哈希, 返回 [0,1)
/// 不引入 rand crate, 但每个师每小时有稳定且分布均匀的"随机"值
fn pseudo_random(seed: u64) -> f64 {
    // xorshift 风格哈希, 映射到 [0,1)
    let mut x = seed.wrapping_mul(2654435761);
    x ^= x >> 15;
    x = x.wrapping_mul(2246822519);
    x ^= x >> 13;
    (x % 10000) as f64 / 10000.0
}

/// 该师本小时是否通过增援概率判定(2%)
fn reinforce_triggered(hour: u64, div_id: u64) -> bool {
    let r = pseudo_random(hour * 100003 + div_id);
    r < REINFORCE_CHANCE
}

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
        let hour = world.hour;
        // 攻方预备队补位: 每个师独立 2% 概率判定(原版 REINFORCE_CHANCE)
        let mut joined_atk = Vec::new();
        for div_id in &res_atk {
            let width = world.divisions.get(div_id).map(|d| d.combat_width).unwrap_or(0.0);
            let frontline = &world.battles[idx].attackers;
            if can_join_frontline(world, frontline, width) && reinforce_triggered(hour, *div_id) {
                joined_atk.push(*div_id);
            }
        }
        // 守方预备队补位
        let mut joined_def = Vec::new();
        for div_id in &res_def {
            let width = world.divisions.get(div_id).map(|d| d.combat_width).unwrap_or(0.0);
            let frontline = &world.battles[idx].defenders;
            if can_join_frontline(world, frontline, width) && reinforce_triggered(hour, *div_id) {
                joined_def.push(*div_id);
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
        // 预备队补位: 2%概率/小时, 推进200小时必然触发
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
        // 推进200小时, 2%概率下几乎必然补位(期望4次)
        for _ in 0..200 {
            world.hour += 1;
            reinforce_reserves(&mut world);
            if world.battles[0].attackers.contains(&id2) { break; }
        }
        assert!(world.battles[0].attackers.contains(&id2), "200小时内预备队应补入前线(2%/h)");
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
