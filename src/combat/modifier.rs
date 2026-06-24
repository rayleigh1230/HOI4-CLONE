//! Modifier 层: 陆战结算的统一修正接口
//!
//! 设计(spec §1-2):
//! - op 由属性名后缀推导(对齐原版 Paradox 约定): 无后缀=Add, _factor=Multiply
//! - 叠加公式: (1+ΣAdd) × Π(1+Multiply)
//! - 空 ModifierStack 的 multiplier 返回 1.0(默认无修正, 精确还原现状)

/// 可被修正的属性(本次覆盖战斗属性+宽度+org恢复)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierStat {
    // 战斗属性(effective_* 的 6 个)
    SoftAttack,
    HardAttack,
    Defense,
    Breakthrough,
    Armor,
    Piercing,
    // 战斗宽度上限
    CombatWidth,
    // 组织度恢复率
    OrgRegain,
}

/// 修正的叠加方式(由属性名后缀推导)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModifierOp {
    /// 无后缀(soft_attack): 加进 add 池, 同类相加
    Add,
    /// _factor 后缀(soft_attack_factor): 独立乘一层
    Multiply,
}

/// 单个 modifier: 作用在某属性上的一个修正
#[derive(Debug, Clone)]
pub struct Modifier {
    pub stat: ModifierStat,
    pub value: f64,           // 0.05 = +5%
    pub op: ModifierOp,       // 由属性名后缀推导, 构造时填好
}

/// 一组 modifier 的集合, 按 stat 查询最终乘数
#[derive(Debug, Clone, Default)]
pub struct ModifierStack {
    mods: Vec<Modifier>,
}

impl ModifierStack {
    pub fn new() -> Self {
        Self { mods: vec![] }
    }

    /// 推入一个 modifier
    pub fn push(&mut self, m: Modifier) {
        self.mods.push(m);
    }

    /// 合并另一个 stack(用于三层汇总: 国家+省份+师)
    pub fn merge(&mut self, other: &ModifierStack) {
        self.mods.extend(other.mods.iter().cloned());
    }

    /// 算某属性的总系数(面板值 × 这个 = 最终值)
    /// 公式: (1 + Σ Add类) × Π(1 + Multiply类)
    /// 空栈返回 1.0(默认无修正)
    pub fn multiplier(&self, stat: ModifierStat) -> f64 {
        let add_sum: f64 = self
            .mods
            .iter()
            .filter(|m| m.stat == stat && m.op == ModifierOp::Add)
            .map(|m| m.value)
            .sum();
        let mult_prod = self
            .mods
            .iter()
            .filter(|m| m.stat == stat && m.op == ModifierOp::Multiply)
            .fold(1.0, |acc, m| acc * (1.0 + m.value));
        (1.0 + add_sum) * mult_prod
    }

    /// 是否为空(无任何 modifier)
    pub fn is_empty(&self) -> bool {
        self.mods.is_empty()
    }

    /// 返回一个静态空栈引用(CombatContext::get 兜底用)
    /// OnceLock 保证只初始化一次, 零外部依赖
    pub fn empty_static() -> &'static ModifierStack {
        use std::sync::OnceLock;
        static EMPTY: OnceLock<ModifierStack> = OnceLock::new();
        EMPTY.get_or_init(ModifierStack::new)
    }
}

/// 字符串属性名 → (stat, op)
/// 规则(对齐原版 Paradox 脚本约定):
///   soft_attack        → (SoftAttack, Add)       无后缀 = 加法
///   soft_attack_factor → (SoftAttack, Multiply)  _factor 后缀 = 乘法
///   defense / defence  → (Defense, Add)          兼容美式/英式
///   armor / armor_value → (Armor, Add)
///   piercing / ap_attack → (Piercing, Add)
/// 未知属性(如 stability_factor) → None(静默跳过)
pub fn parse_modifier_token(s: &str) -> Option<(ModifierStat, ModifierOp)> {
    let (base, op) = if let Some(b) = s.strip_suffix("_factor") {
        (b, ModifierOp::Multiply)
    } else {
        (s, ModifierOp::Add)
    };
    let stat = match base {
        "soft_attack" => ModifierStat::SoftAttack,
        "hard_attack" => ModifierStat::HardAttack,
        "defense" | "defence" => ModifierStat::Defense,
        "breakthrough" => ModifierStat::Breakthrough,
        "armor" | "armor_value" => ModifierStat::Armor,
        "piercing" | "ap_attack" => ModifierStat::Piercing,
        "combat_width" => ModifierStat::CombatWidth,
        "org_regain" | "local_org_regain" => ModifierStat::OrgRegain,
        _ => return None,
    };
    Some((stat, op))
}

use crate::runtime::{Battle, World};
use std::collections::HashMap;

/// 一场战斗的结算上下文(结算前算好, 结算中只读)
/// 把 国家+省份+师 三层 modifier 汇总到每个参战师, 避免结算时借用冲突。
/// 快照设计支持动态 modifier(昼夜/天气), 详见 spec §3.4。
pub struct CombatContext {
    /// 每个参战师的 modifier 汇总(按 division_id 索引)
    stacks: HashMap<u64, ModifierStack>,
}

impl CombatContext {
    /// 结算前构造: 遍历 battle 攻守双方, 为每个师算 modifier 汇总
    /// = 国家modifier + 该师所在省modifier + 师自身modifier
    pub fn build(world: &World, battle: &Battle) -> CombatContext {
        let mut stacks = HashMap::new();
        for div_id in battle
            .attackers
            .iter()
            .chain(&battle.defenders)
            .chain(&battle.reserve_attackers)
            .chain(&battle.reserve_defenders)
        {
            let Some(d) = world.divisions.get(div_id) else {
                continue;
            };
            let mut stack = ModifierStack::new();
            // 国家层: 科技/精神/ideas
            if let Some(c) = world.countries.get(&d.owner_tag) {
                stack.merge(&c.modifiers);
            }
            // 省份层: 地形(静态查表)
            if let Some(p) = world.provinces.get(&battle.province) {
                stack.merge(&terrain_modifiers(&p.terrain));
                // 后续昼夜: stack.merge(&night_modifier(world.darkness[battle.province]))
            }
            // 师自身: 堑壕/计划/经验
            stack.merge(&d.modifiers);
            stacks.insert(*div_id, stack);
        }
        CombatContext { stacks }
    }

    /// 取某师的 modifier 汇总(找不到则返回静态空栈引用, 不 panic)
    pub fn get(&self, div_id: u64) -> &ModifierStack {
        self.stacks.get(&div_id).unwrap_or_else(|| ModifierStack::empty_static())
    }

    /// 构造一个空上下文(无任何 modifier, 用于不关心 modifier 的调用点/测试)
    pub fn empty() -> CombatContext {
        CombatContext { stacks: HashMap::new() }
    }
}

/// 地形 modifier 查表(占位: 本次返回空栈, 无地形数据)
/// 后续地形系统实现时, 按 terrain 名返回真实修正(森林 attack -0.15 等)
/// 夜间修正(night modifier × darkness)后续也走这里, 详见 spec §3.4。
pub fn terrain_modifiers(_terrain: &str) -> ModifierStack {
    ModifierStack::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_empty_stack_returns_one() {
        let s = ModifierStack::new();
        assert!((s.multiplier(ModifierStat::SoftAttack) - 1.0).abs() < 1e-9);
        assert!(s.is_empty());
    }

    #[test]
    fn t_pure_add_sums() {
        // +5% + +10% → 1.15
        let mut s = ModifierStack::new();
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: 0.05, op: ModifierOp::Add });
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: 0.10, op: ModifierOp::Add });
        assert!((s.multiplier(ModifierStat::SoftAttack) - 1.15).abs() < 1e-9);
    }

    #[test]
    fn t_pure_multiply_products() {
        // ×5% × ×10% → 1.05 × 1.10 = 1.155
        let mut s = ModifierStack::new();
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: 0.05, op: ModifierOp::Multiply });
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: 0.10, op: ModifierOp::Multiply });
        assert!((s.multiplier(ModifierStat::SoftAttack) - 1.155).abs() < 1e-9);
    }

    #[test]
    fn t_mixed_add_then_multiply() {
        // (1+0.05) × (1+0.10) = 1.155
        let mut s = ModifierStack::new();
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: 0.05, op: ModifierOp::Add });
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: 0.10, op: ModifierOp::Multiply });
        assert!((s.multiplier(ModifierStat::SoftAttack) - 1.155).abs() < 1e-9);
    }

    #[test]
    fn t_negative_multiply_never_negative() {
        // -50% × -30% × -25% → 0.5 × 0.7 × 0.75 = 0.2625 (不负)
        let mut s = ModifierStack::new();
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: -0.50, op: ModifierOp::Multiply });
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: -0.30, op: ModifierOp::Multiply });
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: -0.25, op: ModifierOp::Multiply });
        let m = s.multiplier(ModifierStat::SoftAttack);
        assert!(m > 0.0, "乘法类负修正应保持正数, 实际 {}", m);
        assert!((m - 0.2625).abs() < 1e-9);
    }

    #[test]
    fn t_merge_combines_stacks() {
        let mut a = ModifierStack::new();
        a.push(Modifier { stat: ModifierStat::Defense, value: 0.10, op: ModifierOp::Add });
        let mut b = ModifierStack::new();
        b.push(Modifier { stat: ModifierStat::Defense, value: 0.20, op: ModifierOp::Add });
        a.merge(&b);
        assert!((a.multiplier(ModifierStat::Defense) - 1.30).abs() < 1e-9);
    }

    #[test]
    fn t_parse_no_suffix_is_add() {
        let (stat, op) = parse_modifier_token("soft_attack").unwrap();
        assert_eq!(stat, ModifierStat::SoftAttack);
        assert_eq!(op, ModifierOp::Add);
    }

    #[test]
    fn t_parse_factor_suffix_is_multiply() {
        let (stat, op) = parse_modifier_token("soft_attack_factor").unwrap();
        assert_eq!(stat, ModifierStat::SoftAttack);
        assert_eq!(op, ModifierOp::Multiply);
    }

    #[test]
    fn t_parse_defence_variant() {
        let (stat, _) = parse_modifier_token("defence").unwrap();
        assert_eq!(stat, ModifierStat::Defense);
        let (stat2, _) = parse_modifier_token("defense").unwrap();
        assert_eq!(stat2, ModifierStat::Defense);
    }

    #[test]
    fn t_parse_armor_value() {
        let (stat, _) = parse_modifier_token("armor_value").unwrap();
        assert_eq!(stat, ModifierStat::Armor);
    }

    #[test]
    fn t_parse_unknown_returns_none() {
        assert!(parse_modifier_token("stability_factor").is_none());
        assert!(parse_modifier_token("ace_effectiveness_factor").is_none());
        assert!(parse_modifier_token("political_power").is_none());
    }

    use crate::runtime::{Battle, World};

    #[test]
    fn t_empty_context_get_returns_empty_stack() {
        let ctx = CombatContext::empty();
        let m = ctx.get(999).multiplier(ModifierStat::SoftAttack);
        assert!((m - 1.0).abs() < 1e-9, "空 ctx 查任意师应返回 1.0");
    }

    #[test]
    fn t_build_aggregates_country_and_division_modifiers() {
        // 国家 GER 有 +10% soft(add), 师有 -15% soft(multiply)
        // 最终 = (1+0.10) × (1-0.15) = 1.10 × 0.85 = 0.935
        let mut w = World::new();
        let mut country = crate::runtime::Country::default();
        country.modifiers.push(Modifier {
            stat: ModifierStat::SoftAttack, value: 0.10, op: ModifierOp::Add,
        });
        w.countries.insert("GER".into(), country);

        let mut div = crate::runtime::Division::default();
        div.owner_tag = "GER".into();
        div.modifiers.push(Modifier {
            stat: ModifierStat::SoftAttack, value: -0.15, op: ModifierOp::Multiply,
        });
        let div_id = w.add_division(div);

        w.provinces.insert(1, crate::runtime::Province {
            id: 1, owner: "GER".into(), controller: "GER".into(),
            terrain: "plains".into(), neighbors: vec![],
        });

        let battle = Battle {
            id: 0, province: 1,
            attackers: vec![div_id], defenders: vec![],
            reserve_attackers: vec![], reserve_defenders: vec![],
        };
        let ctx = CombatContext::build(&w, &battle);
        let m = ctx.get(div_id).multiplier(ModifierStat::SoftAttack);
        assert!((m - 0.935).abs() < 1e-9, "国家+师 modifier 汇总应 0.935, 实际 {}", m);
    }

    #[test]
    fn t_build_skips_missing_division() {
        // battle 引用了不存在的师 id, build 不应 panic
        let w = World::new();
        let battle = Battle {
            id: 0, province: 1,
            attackers: vec![999], defenders: vec![],
            reserve_attackers: vec![], reserve_defenders: vec![],
        };
        let ctx = CombatContext::build(&w, &battle);
        assert!(ctx.get(999).is_empty());
    }
}
