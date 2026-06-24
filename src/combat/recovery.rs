//! 组织度恢复: 非战斗中的师每小时回升 org(陆战循环)
//!
//! 恢复率: max_org × DAILY_ORG_RECOVERY_RATE / 24, 受补给充足度影响。
//! 战斗中的师不恢复(在互殴)。
use crate::runtime::entities::OrderState;
use crate::runtime::World;

/// 每日组织度恢复率(占 max_org 的比例)。约 12%/天 → 一周恢复满。
const DAILY_ORG_RECOVERY_RATE: f64 = 0.12;
/// 行军中每小时组织度损失(原版 HOURLY_ORG_MOVEMENT_IMPACT=-0.2)
const HOURLY_ORG_MOVEMENT_IMPACT: f64 = -0.2;

/// 对所有非战斗师执行组织度恢复(每小时调用)
pub fn recover_org(world: &mut World) {
    // 收集所有在交战地块的师 id(前线 + 预备队, 都不恢复org)
    let in_combat: std::collections::HashSet<u64> = world
        .battles
        .iter()
        .flat_map(|b| {
            b.attackers.iter()
                .chain(b.defenders.iter())
                .chain(b.reserve_attackers.iter())
                .chain(b.reserve_defenders.iter())
                .copied()
        })
        .collect();

    for div in world.divisions.values_mut() {
        if in_combat.contains(&div.id) {
            continue; // 战斗中不恢复
        }
        // 按状态机决定 org 变化
        // Moving 到敌方非己方地块: 每小时 -0.2, 不恢复; 其余状态恢复
        if let OrderState::Moving { dest, hostile: true, .. } = div.order {
            // 内联字段访问(不用 province_controller 方法, 避免借整个 &self 与 values_mut 冲突)
            // Rust 分离字段借用: provinces/states 不可变 + divisions 可变, 不冲突
            let is_friendly = world.provinces.get(&dest)
                .and_then(|p| world.states.get(&p.state_id))
                .map(|s| s.controller == div.owner_tag)
                .unwrap_or(false);
            if !is_friendly {
                div.org = (div.org + HOURLY_ORG_MOVEMENT_IMPACT).max(0.0);
                continue;
            }
        }
        // 恢复 org; Retreating 满血时转 Idle
        if div.org >= div.max_org {
            if div.is_withdrawing() {
                div.order = OrderState::Idle; // 撤退师恢复满, 退出撤退态
            }
            continue;
        }
        let hourly = div.max_org * DAILY_ORG_RECOVERY_RATE / 24.0;
        let org_mult = div.modifiers.multiplier(crate::combat::modifier::ModifierStat::OrgRegain);
        let recovery = hourly * (0.5 + 0.5 * div.supply_ratio()) * org_mult;
        div.org = (div.org + recovery).min(div.max_org);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::entities::Division;

    fn div_with_org(org: f64, max_org: f64) -> Division {
        Division {
            id: 0,
            owner_tag: "X".into(),
            org,
            max_org,
            ..Default::default()
        }
    }

    #[test]
    fn t_non_combat_recovers_org() {
        let mut w = World::new();
        let did = w.add_division(div_with_org(30.0, 60.0));
        assert!((w.divisions.get(&did).unwrap().org - 30.0).abs() < 1e-9);
        // 推进 24 小时(无战斗)
        for _ in 0..24 {
            recover_org(&mut w);
        }
        let after = w.divisions.get(&did).unwrap().org;
        assert!(after > 30.0, "24h 后 org 应回升: {after}");
        // 约 12%/天 → 24h 升约 7.2(补给满), 但受 supply_ratio 影响
        assert!(after < 60.0, "不应超过 max_org");
    }

    #[test]
    fn t_full_org_no_change() {
        let mut w = World::new();
        let did = w.add_division(div_with_org(60.0, 60.0));
        recover_org(&mut w);
        assert!((w.divisions.get(&did).unwrap().org - 60.0).abs() < 1e-9, "满 org 不应变");
    }

    #[test]
    fn t_in_combat_no_recovery() {
        let mut w = World::new();
        let did = w.add_division(div_with_org(30.0, 60.0));
        // 放入战斗
        w.battles.push(crate::runtime::entities::Battle {
            id: 1,
            province: 1,
            attackers: vec![did],
            defenders: vec![],
            ..Default::default()
        });
        recover_org(&mut w);
        assert!((w.divisions.get(&did).unwrap().org - 30.0).abs() < 1e-9, "战斗中不应恢复");
    }
}
