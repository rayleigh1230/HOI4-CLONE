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
    // ★ 资源属性(国家级三件套)
    Stability,        // stability / stability_factor
    WarSupport,       // war_support / war_support_factor
    PoliticalPower,   // political_power / political_power_factor
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
        "stability" => ModifierStat::Stability,
        "war_support" => ModifierStat::WarSupport,
        "political_power" => ModifierStat::PoliticalPower,
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
    /// 该战斗省份的攻方地形惩罚系数(0-1, 越低惩罚越重; 守方不受影响)。
    /// build 时按 battle.province 地形算好, 结算时只乘攻方(AtkStats::from)。
    attacker_terrain_penalty: f64,
}

impl CombatContext {
    /// 结算前构造: 遍历 battle 攻守双方, 为每个师算 modifier 汇总
    /// = 国家modifier + 该师所在省modifier + 师自身modifier
    pub fn build(world: &World, battle: &Battle) -> CombatContext {
        let mut stacks = HashMap::new();
        // 攻方地形惩罚: 按 battle.province 地形查表(守方不享受此系数)
        let attacker_terrain_penalty = world.provinces.get(&battle.province)
            .map(|p| terrain_attacker_penalty(&p.terrain))
            .unwrap_or(1.0);
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
            // 省份层: 地形攻方惩罚不进此通用 stack(只作用于攻方, 见 AtkStats::from)。
            // 后续昼夜 modifier(night × darkness)在此 merge(昼夜对攻守都生效)。
            // 师自身: 堑壕/计划/经验
            stack.merge(&d.modifiers);
            stacks.insert(*div_id, stack);
        }
        CombatContext { stacks, attacker_terrain_penalty }
    }

    /// 取某师的 modifier 汇总(找不到则返回静态空栈引用, 不 panic)
    pub fn get(&self, div_id: u64) -> &ModifierStack {
        self.stacks.get(&div_id).unwrap_or_else(|| ModifierStack::empty_static())
    }

    /// 攻方地形惩罚系数(0-1; 守方不应使用此值)
    pub fn attacker_terrain_penalty(&self) -> f64 {
        self.attacker_terrain_penalty
    }

    /// 构造一个空上下文(无任何 modifier, 用于不关心 modifier 的调用点/测试)
    pub fn empty() -> CombatContext {
        CombatContext { stacks: HashMap::new(), attacker_terrain_penalty: 1.0 }
    }
}

/// 地形攻方惩罚系数(0.0-1.0, 越低惩罚越重)。
/// 原版 common/terrain/00_terrain.txt 的 `units = { attack = -X }`: 攻方 soft/hard_attack 打折。
/// 守方不受影响(享受地形优势)。数值取自原版数据文件(wiki 的 -20%/-60% 不准):
///   plains/desert 无 units 块 → 1.0 / forest -0.15 → 0.85 / hills -0.25 → 0.75 /
///   jungle -0.30 → 0.70 / urban -0.30 → 0.70 / marsh -0.40 → 0.60 / mountain -0.50 → 0.50。
/// 用于 AtkStats::from 攻方快照(只乘攻方, 不进 CombatContext 通用 stack)。
pub fn terrain_attacker_penalty(terrain: &str) -> f64 {
    match terrain {
        "plains" | "desert" => 1.0,
        "forest" => 0.85,
        "hills" => 0.75,
        "jungle" | "urban" => 0.70,
        "marsh" => 0.60,
        "mountain" => 0.50,
        _ => 1.0, // 未知地形回退平原(无惩罚)
    }
}

/// 地形战斗宽度(每种地形基础宽度不同; 原版 terrain.txt combat_width)。
/// 数值取自原版数据文件: plains/desert/hills 70 / forest/jungle 60 / marsh/mountain 50 / urban 80。
/// 多方向加宽(每多一进攻方向 +combat_support_width)本次不做(YAGNI, demo 单方向), 留 TODO。
pub fn terrain_combat_width(terrain: &str) -> f64 {
    match terrain {
        "plains" | "desert" | "hills" => 70.0,
        "forest" | "jungle" => 60.0,
        "marsh" | "mountain" => 50.0,
        "urban" => 80.0,
        _ => 70.0, // 未知地形回退平原
    }
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
        // 真正未知的属性仍返回 None(stability/political_power 现在是已知资源属性, 见 t_parse_resource_tokens)
        assert!(parse_modifier_token("ace_effectiveness_factor").is_none());
        assert!(parse_modifier_token("research_speed").is_none());
        assert!(parse_modifier_token("foo_bar").is_none());
    }

    #[test]
    fn t_parse_resource_tokens() {
        // 资源属性三件套: 无后缀=Add, _factor=Multiply(对齐原版)
        let (s, op) = parse_modifier_token("stability").unwrap();
        assert_eq!(s, ModifierStat::Stability);
        assert_eq!(op, ModifierOp::Add);
        let (s, op) = parse_modifier_token("stability_factor").unwrap();
        assert_eq!(s, ModifierStat::Stability);
        assert_eq!(op, ModifierOp::Multiply);

        let (s, op) = parse_modifier_token("war_support").unwrap();
        assert_eq!(s, ModifierStat::WarSupport);
        assert_eq!(op, ModifierOp::Add);
        let (s, _) = parse_modifier_token("war_support_factor").unwrap();
        assert_eq!(s, ModifierStat::WarSupport);

        let (s, op) = parse_modifier_token("political_power").unwrap();
        assert_eq!(s, ModifierStat::PoliticalPower);
        assert_eq!(op, ModifierOp::Add);
        let (s, _) = parse_modifier_token("political_power_factor").unwrap();
        assert_eq!(s, ModifierStat::PoliticalPower);
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

        w.states.insert(1000, crate::runtime::State {
            id: 1000, owner: "GER".into(), controller: "GER".into(),
            ..Default::default()
        });
        w.provinces.insert(1, crate::runtime::Province {
            id: 1, state_id: 1000, terrain: "plains".into(), neighbors: vec![], ..Default::default()
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

    #[test]
    fn t_terrain_attacker_penalty_values() {
        // 攻方惩罚系数(对齐原版 common/terrain/00_terrain.txt 的 units={attack=-X}):
        // 越恶劣越低。数值取自原版数据文件(wiki 的 -20%/-60% 不准)
        assert!((terrain_attacker_penalty("plains") - 1.0).abs() < 1e-9, "平原无 units 块无惩罚");
        assert!((terrain_attacker_penalty("desert") - 1.0).abs() < 1e-9);
        assert!((terrain_attacker_penalty("forest") - 0.85).abs() < 1e-9, "森林 attack=-0.15");
        assert!((terrain_attacker_penalty("hills") - 0.75).abs() < 1e-9, "丘陵 attack=-0.25");
        assert!((terrain_attacker_penalty("jungle") - 0.70).abs() < 1e-9, "丛林 attack=-0.30");
        assert!((terrain_attacker_penalty("urban") - 0.70).abs() < 1e-9, "城市 attack=-0.30");
        assert!((terrain_attacker_penalty("marsh") - 0.60).abs() < 1e-9, "沼泽 attack=-0.40");
        assert!((terrain_attacker_penalty("mountain") - 0.50).abs() < 1e-9, "山地 attack=-0.50");
        assert!((terrain_attacker_penalty("unknown_xyz") - 1.0).abs() < 1e-9, "未知回退平原");
    }

    #[test]
    fn t_terrain_combat_width_values() {
        // 地形宽度(对齐原版 common/terrain/00_terrain.txt combat_width)
        assert!((terrain_combat_width("plains") - 70.0).abs() < 1e-9);
        assert!((terrain_combat_width("hills") - 70.0).abs() < 1e-9);
        assert!((terrain_combat_width("forest") - 60.0).abs() < 1e-9);
        assert!((terrain_combat_width("mountain") - 50.0).abs() < 1e-9);
        assert!((terrain_combat_width("marsh") - 50.0).abs() < 1e-9, "沼泽50(原版, 非54)");
        assert!((terrain_combat_width("urban") - 80.0).abs() < 1e-9, "城市最宽80");
        assert!((terrain_combat_width("unknown_xyz") - 70.0).abs() < 1e-9, "未知回退平原");
    }

    #[test]
    fn t_context_attacker_terrain_penalty_from_battle_province() {
        // CombatContext::build 按 battle.province 地形填攻方惩罚系数
        let mut w = World::new();
        w.states.insert(1000, crate::runtime::State {
            id: 1000, owner: "GER".into(), controller: "GER".into(), ..Default::default()
        });
        // 山地省(惩罚 0.40)
        w.provinces.insert(1, crate::runtime::Province {
            id: 1, state_id: 1000, terrain: "mountain".into(), neighbors: vec![], ..Default::default()
        });
        let battle = Battle {
            id: 0, province: 1, attackers: vec![], defenders: vec![],
            reserve_attackers: vec![], reserve_defenders: vec![],
        };
        let ctx = CombatContext::build(&w, &battle);
        assert!((ctx.attacker_terrain_penalty() - 0.50).abs() < 1e-9,
            "山地战斗攻方惩罚应 0.50(原版 attack=-0.5), 实际 {}", ctx.attacker_terrain_penalty());

        // 平原省(无惩罚)
        w.provinces.insert(2, crate::runtime::Province {
            id: 2, state_id: 1000, terrain: "plains".into(), neighbors: vec![], ..Default::default()
        });
        let battle2 = Battle { id: 0, province: 2, attackers: vec![], defenders: vec![],
            reserve_attackers: vec![], reserve_defenders: vec![] };
        let ctx2 = CombatContext::build(&w, &battle2);
        assert!((ctx2.attacker_terrain_penalty() - 1.0).abs() < 1e-9, "平原无惩罚");

        // 空上下文默认无惩罚(不破坏现有调用点)
        assert!((CombatContext::empty().attacker_terrain_penalty() - 1.0).abs() < 1e-9);
    }
}
