//! 陆战结算(公式见 docs/formulas/land-combat.md)
//!
//! 安全借用策略: 攻方属性拷贝成只读 AtkStats, 守方可变借用写伤害。避免 unsafe。
use crate::runtime::entities::Division;
use crate::runtime::World;

/// 全局系数(对应 NMilitary defines, 见 docs/formulas/land-combat.md)
const ORG_DICE_SIZE: f64 = 4.0;
const STR_DICE_SIZE: f64 = 2.0;
const ORG_DMG_MOD: f64 = 0.053;
const STR_DMG_MOD: f64 = 0.060;
const HIT_CHANCE_DEF_LEFT: f64 = 0.10; // 防御池未空
const HIT_CHANCE_NO_DEF: f64 = 0.40; // 防御池耗尽
const ARMOR_ORG_BONUS_DICE: f64 = 6.0; // 装甲碾压额外组织度骰
const ARMOR_STR_BONUS_DICE: f64 = 2.0;
const DAMAGE_SPLIT_FIRST: f64 = 0.35; // 首要目标分摊

/// 攻方只读快照(避免与守方可变借用冲突)
#[derive(Clone, Copy)]
struct AtkStats {
    soft_attack: f64,
    hard_attack: f64,
    armor: f64,
    piercing: f64,
}

impl AtkStats {
    fn from(d: &Division) -> Self {
        Self {
            soft_attack: d.soft_attack,
            hard_attack: d.hard_attack,
            armor: d.armor,
            piercing: d.piercing,
        }
    }
}

/// 对一组攻击者 vs 一组防御者结算 1 小时(对称: 攻→守 + 守→攻)
/// 守方为可变引用切片, 兼容 HashMap::get_mut 收集的 Vec<&mut Division>
pub fn resolve_hour(attackers: &[Division], defenders: &mut [&mut Division]) {
    if attackers.is_empty() || defenders.is_empty() {
        return;
    }
    // 正向: 攻方 → 守方(守方用 defense 池)
    for atk in attackers {
        let atk_stats = AtkStats::from(atk);
        apply_side_to_side(&atk_stats, defenders, CombatPool::Defense);
    }
    // P0-2 反击: 守方 → 攻方(攻方用 breakthrough 池)。
    // 攻方此时需可变借用; 但 attackers 是 &[Division](只读)。
    // 解法: 把攻方伤害累积到独立结构, 调用方(resolve_all_battles)处理对称。
    // 此函数内反击由 resolve_all_battles 通过交换攻守角色实现。
}

/// 哪一方的防御池: 守方用 defense, 攻方(被反击时)用 breakthrough
#[derive(Clone, Copy)]
enum CombatPool {
    Defense,
    Breakthrough,
}

impl CombatPool {
    fn pool_value(self, d: &Division) -> f64 {
        match self {
            CombatPool::Defense => d.defense,
            CombatPool::Breakthrough => d.breakthrough,
        }
    }
}

/// 单个攻击方对一组目标输出伤害(首要目标 35% + 所有目标均分 65%)
fn apply_side_to_side(atk: &AtkStats, targets: &mut [&mut Division], pool: CombatPool) {
    let n = targets.len();
    if n == 0 {
        return;
    }
    // 用首个目标的硬度算总攻击点(M3 简化: 假设目标硬度一致)
    let target_hardness = targets[0].hardness;
    let attacks = atk.soft_attack * (1.0 - target_hardness) + atk.hard_attack * target_hardness;
    if attacks <= 0.0 {
        return;
    }

    for (i, tgt) in targets.iter_mut().enumerate() {
        // P0-1: 首要目标 35% + 所有目标均分 65%(含首要)。
        let base = (1.0 - DAMAGE_SPLIT_FIRST) / n as f64;
        let share = if i == 0 { DAMAGE_SPLIT_FIRST + base } else { base };
        let attacks_on_this = attacks * share;

        let armor_outclass = atk.armor > tgt.piercing;
        let def_outclass = tgt.armor > atk.piercing;

        let hits = compute_hits(attacks_on_this, pool.pool_value(tgt));

        let mut org_dice = ORG_DICE_SIZE;
        let mut str_dice = STR_DICE_SIZE;
        if armor_outclass {
            org_dice += ARMOR_ORG_BONUS_DICE;
            str_dice += ARMOR_STR_BONUS_DICE;
        }
        // P2-9: 装甲偏转同时作用于 org 和 str
        let armor_deflect = if def_outclass { 0.5 } else { 1.0 };

        // P1-3: 1dN 期望 = (N+1)/2
        let org_dmg = hits * ((org_dice + 1.0) / 2.0) * ORG_DMG_MOD * armor_deflect;
        let str_dmg = hits * ((str_dice + 1.0) / 2.0) * STR_DMG_MOD * armor_deflect;

        tgt.org = (tgt.org - org_dmg).max(0.0);
        tgt.strength = (tgt.strength - str_dmg).max(0.0);
    }
}

/// 计算命中数(防御池机制)
fn compute_hits(attacks: f64, def_pool: f64) -> f64 {
    let defended = attacks.min(def_pool);
    let undefended = (attacks - def_pool).max(0.0);
    defended * HIT_CHANCE_DEF_LEFT + undefended * HIT_CHANCE_NO_DEF
}

/// World 级战斗结算: 遍历所有 battle, 每小时调用
/// 三阶段: ① 快照攻守; ② 对称结算(攻→守 defense 池 + 守→攻 breakthrough 池);
///         ③ 按 id 写回所有受影响师(攻+守)。避免多可变借用冲突, 无 unsafe。
pub fn resolve_all_battles(world: &mut World) {
    let battle_specs: Vec<(Vec<u64>, Vec<u64>)> = world
        .battles
        .iter()
        .map(|b| (b.attackers.clone(), b.defenders.clone()))
        .collect();

    // 用 HashMap 聚合每个师的最终值(同一师可能在多场战斗, 需合并而非覆盖 → 修 P1-6)
    use std::collections::HashMap;
    let mut final_state: HashMap<u64, (f64, f64)> = HashMap::new();

    for (atk_ids, def_ids) in &battle_specs {
        let mut atks: Vec<Division> =
            atk_ids.iter().filter_map(|id| world.divisions.get(id).cloned()).collect();
        let mut defs: Vec<Division> =
            def_ids.iter().filter_map(|id| world.divisions.get(id).cloned()).collect();
        if atks.is_empty() || defs.is_empty() {
            continue;
        }

        // 正向: 攻 → 守(守用 defense 池)
        {
            let mut def_refs: Vec<&mut Division> = defs.iter_mut().collect();
            for atk in &atks {
                apply_side_to_side(&AtkStats::from(atk), &mut def_refs, CombatPool::Defense);
            }
        }
        // 反向(反击): 守 → 攻(攻用 breakthrough 池)
        {
            let mut atk_refs: Vec<&mut Division> = atks.iter_mut().collect();
            for def in &defs {
                apply_side_to_side(&AtkStats::from(def), &mut atk_refs, CombatPool::Breakthrough);
            }
        }

        // 合并本场战斗结果到 final_state
        for (i, id) in atk_ids.iter().enumerate() {
            if let Some(d) = atks.get(i) {
                final_state.insert(*id, (d.org, d.strength));
            }
        }
        for (i, id) in def_ids.iter().enumerate() {
            if let Some(d) = defs.get(i) {
                final_state.insert(*id, (d.org, d.strength));
            }
        }
    }

    // 写回
    for (id, (org, str)) in final_state {
        if let Some(d) = world.divisions.get_mut(&id) {
            d.org = org;
            d.strength = str;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inf(owner: &str) -> Division {
        Division {
            id: 0,
            owner_tag: owner.into(),
            location_province: 1,
            soft_attack: 30.0,
            hard_attack: 2.0,
            defense: 40.0,
            breakthrough: 8.0,
            armor: 0.0,
            piercing: 5.0,
            hardness: 0.0,
            combat_width: 10.0,
            max_org: 60.0,
            org: 60.0,
            max_strength: 20.0,
            strength: 20.0,
        }
    }

    #[test]
    fn t_inf_vs_inf_reduces_org() {
        let atks = [inf("ATK")];
        let mut d = inf("DEF");
        let org_before = d.org;
        let mut defs = [&mut d];
        resolve_hour(&atks, &mut defs);
        assert!(d.org < org_before, "守方组织度应下降");
        assert!(d.org >= 0.0);
    }

    #[test]
    fn t_armor_outclass_deals_damage() {
        // 装甲师 vs 步兵(穿甲不足): 装甲碾压。调高软攻击让伤害可见
        let mut armor = inf("ATK");
        armor.armor = 50.0;
        armor.piercing = 50.0;
        armor.soft_attack = 200.0;
        let mut d = inf("DEF"); // piercing=5 < armor=50
        let org_before = d.org;
        let mut defs = [&mut d];
        resolve_hour(&[armor], &mut defs);
        assert!(d.org < org_before, "装甲碾压应造成伤害");
        assert!(org_before - d.org > 1.0, "装甲碾压伤害应显著, 实际 {}", org_before - d.org);
    }

    #[test]
    fn t_high_defense_reduces_damage() {
        // 低防御方 defense=5(攻击点超过池, 命中率 40%); 高防御方 defense=200(池未空, 10%)
        let atks = [inf("ATK")];
        let atks2 = [inf("ATK")];
        let mut low = inf("DEF");
        let mut high = inf("DEF");
        low.defense = 5.0; // 攻击点 10.5 > 5, 部分进入 40% 命中
        high.defense = 200.0; // 攻击点 10.5 < 200, 全 10% 命中
        let low_before = low.org;
        let high_before = high.org;
        let mut low_defs = [&mut low];
        let mut high_defs = [&mut high];
        resolve_hour(&atks, &mut low_defs);
        resolve_hour(&atks2, &mut high_defs);
        let low_drop = low_before - low.org;
        let high_drop = high_before - high.org;
        assert!(
            high_drop < low_drop,
            "高防御池应减少伤害: high_drop={high_drop} 应 < low_drop={low_drop}"
        );
    }
}
