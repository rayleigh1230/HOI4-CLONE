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

/// 对一组攻击者 vs 一组防御者结算 1 小时
/// 守方为可变引用切片, 兼容 HashMap::get_mut 收集的 Vec<&mut Division>
pub fn resolve_hour(attackers: &[Division], defenders: &mut [&mut Division]) {
    if attackers.is_empty() || defenders.is_empty() {
        return;
    }
    // 累积所有攻方的伤害到每个守方
    for atk in attackers {
        let atk_stats = AtkStats::from(atk);
        apply_attacker_to_defenders(&atk_stats, defenders);
    }
}

/// 单个攻方对一组守方输出伤害(首要目标 35%, 其余均分 65%)
fn apply_attacker_to_defenders(atk: &AtkStats, defenders: &mut [&mut Division]) {
    let n = defenders.len();
    if n == 0 {
        return;
    }
    // 用首个守方的硬度算总攻击点(M3 简化: 假设守方硬度一致)
    let target_hardness = defenders[0].hardness;
    let attacks = atk.soft_attack * (1.0 - target_hardness) + atk.hard_attack * target_hardness;
    if attacks <= 0.0 {
        return;
    }

    for (i, def) in defenders.iter_mut().enumerate() {
        let share = if i == 0 {
            DAMAGE_SPLIT_FIRST
        } else {
            (1.0 - DAMAGE_SPLIT_FIRST) / (n - 1).max(1) as f64
        };
        let attacks_on_this = attacks * share;

        let armor_outclass = atk.armor > def.piercing;
        let def_outclass = def.armor > atk.piercing;

        let hits = compute_hits(attacks_on_this, def.defense);

        let mut org_dice = ORG_DICE_SIZE;
        let mut str_dice = STR_DICE_SIZE;
        if armor_outclass {
            org_dice += ARMOR_ORG_BONUS_DICE;
            str_dice += ARMOR_STR_BONUS_DICE;
        }
        let armor_deflect = if def_outclass { 0.5 } else { 1.0 };

        // 期望伤害(骰子均值 = size/2)
        let org_dmg = hits * (org_dice / 2.0) * ORG_DMG_MOD;
        let str_dmg = hits * (str_dice / 2.0) * STR_DMG_MOD * armor_deflect;

        def.org = (def.org - org_dmg).max(0.0);
        def.strength = (def.strength - str_dmg).max(0.0);
    }
}

/// 计算命中数(防御池机制)
fn compute_hits(attacks: f64, def_pool: f64) -> f64 {
    let defended = attacks.min(def_pool);
    let undefended = (attacks - def_pool).max(0.0);
    defended * HIT_CHANCE_DEF_LEFT + undefended * HIT_CHANCE_NO_DEF
}

/// World 级战斗结算: 遍历所有 battle, 每小时调用
/// 两阶段: ① 读阶段(快照)计算每个守方应受伤害; ② 写阶段按 id 写回。避免多可变借用冲突。
pub fn resolve_all_battles(world: &mut World) {
    // 收集每场战斗的攻守 id
    let battle_specs: Vec<(Vec<u64>, Vec<u64>)> = world
        .battles
        .iter()
        .map(|b| (b.attackers.clone(), b.defenders.clone()))
        .collect();

    // 阶段1: 读 + 计算 → (def_id, new_org, new_str) 列表(克隆守方结算后取最终值)
    let mut results: Vec<(u64, f64, f64)> = Vec::new();
    for (atk_ids, def_ids) in &battle_specs {
        let atks: Vec<Division> = atk_ids.iter().filter_map(|id| world.divisions.get(id).cloned()).collect();
        if atks.is_empty() {
            continue;
        }
        let mut defs: Vec<Division> =
            def_ids.iter().filter_map(|id| world.divisions.get(id).cloned()).collect();
        if defs.is_empty() {
            continue;
        }
        let mut def_refs: Vec<&mut Division> = defs.iter_mut().collect();
        resolve_hour(&atks, &mut def_refs);
        // 记录结算后每个守方的最终 org/str
        for (i, def_id) in def_ids.iter().enumerate() {
            if let Some(d) = defs.get(i) {
                results.push((*def_id, d.org, d.strength));
            }
        }
    }

    // 阶段2: 写回(每个 id 独立 get_mut, 无并发借用)
    for (def_id, new_org, new_str) in results {
        if let Some(d) = world.divisions.get_mut(&def_id) {
            d.org = new_org;
            d.strength = new_str;
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
