//! 装备数据模型: 底盘/模块/装备变体的结构与属性汇总
//!
//! 统一模型(spec §3.2): 所有装备 = 底盘 + 模块组合。
//! 整件装备(步兵/炮)是 slots 为空的底盘; 模块化装备(坦克)有槽位。
//! 属性汇总(spec §3.3): raw = base + Σ add_stats; final = raw × Π(1 + multiply_stats)

use crate::data::EquipStats;
use crate::parser::Block;

/// 从一个 Block 提取装备属性字段(soft_attack/defense/armor_value 等)成 EquipStats
/// 用于: 底盘基础属性、模块 add_stats、模块 multiply_stats
///
/// 字段名映射(原版名 → EquipStats 字段):
///   soft_attack → soft_attack
///   hard_attack → hard_attack
///   defense → defense
///   breakthrough → breakthrough
///   armor_value → armor
///   ap_attack → piercing
///   hardness → hardness
///   build_cost_ic → build_cost_ic
///   maximum_speed → maximum_speed
///   reliability → reliability
pub fn extract_stats(block: &Block) -> EquipStats {
    let mut s = EquipStats::default();
    for f in &block.fields {
        match f.key.as_str() {
            "soft_attack" => s.soft_attack = f.value.as_scalar_num().unwrap_or(0.0),
            "hard_attack" => s.hard_attack = f.value.as_scalar_num().unwrap_or(0.0),
            "defense" => s.defense = f.value.as_scalar_num().unwrap_or(0.0),
            "breakthrough" => s.breakthrough = f.value.as_scalar_num().unwrap_or(0.0),
            "armor_value" => s.armor = f.value.as_scalar_num().unwrap_or(0.0),
            "ap_attack" => s.piercing = f.value.as_scalar_num().unwrap_or(0.0),
            "hardness" => s.hardness = f.value.as_scalar_num().unwrap_or(0.0),
            "build_cost_ic" => s.build_cost_ic = f.value.as_scalar_num().unwrap_or(0.0),
            "maximum_speed" => s.maximum_speed = f.value.as_scalar_num().unwrap_or(0.0),
            "reliability" => s.reliability = f.value.as_scalar_num().unwrap_or(0.0),
            _ => {}
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn t_extract_infantry_stats() {
        // 原版 infantry_equipment archetype 的属性块(实证)
        let src = "defense = 20\nbreakthrough = 2\nhardness = 0\narmor_value = 0\n\
                   soft_attack = 3\nhard_attack = 0.5\nap_attack = 1\nbuild_cost_ic = 0.43";
        let b = parse(src).unwrap();
        let s = extract_stats(&b);
        assert!((s.soft_attack - 3.0).abs() < 1e-9);
        assert!((s.defense - 20.0).abs() < 1e-9);
        assert!((s.breakthrough - 2.0).abs() < 1e-9);
        assert!((s.piercing - 1.0).abs() < 1e-9);
        assert!((s.build_cost_ic - 0.43).abs() < 1e-9);
    }

    #[test]
    fn t_extract_tank_stats() {
        // 原版 light_tank_chassis_1: armor_value = 15
        let src = "armor_value = 15\nbuild_cost_ic = 2.35\nmaximum_speed = 5\nreliability = 0.95";
        let b = parse(src).unwrap();
        let s = extract_stats(&b);
        assert!((s.armor - 15.0).abs() < 1e-9);
        assert!((s.build_cost_ic - 2.35).abs() < 1e-9);
    }

    #[test]
    fn t_extract_ignores_unknown_fields() {
        // 未知字段(year/picture/type 等)应被忽略, 不报错
        let src = "year = 1936\npicture = foo\nsoft_attack = 3\ntype = infantry";
        let b = parse(src).unwrap();
        let s = extract_stats(&b);
        assert!((s.soft_attack - 3.0).abs() < 1e-9);
    }
}
