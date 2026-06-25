//! 装备数据模型: 底盘/模块/装备变体的结构与属性汇总
//!
//! 统一模型(spec §3.2): 所有装备 = 底盘 + 模块组合。
//! 整件装备(步兵/炮)是 slots 为空的底盘; 模块化装备(坦克)有槽位。
//! 属性汇总(spec §3.3): raw = base + Σ add_stats; final = raw × Π(1 + multiply_stats)

use crate::data::EquipStats;
use crate::parser::Block;
use std::collections::HashMap;

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

/// 底盘定义(archetype): 槽位结构 + 默认模块
/// 整件装备(步兵/炮)的 slots 为空; 模块化装备(坦克)有槽位
#[derive(Debug, Clone)]
pub struct ChassisDef {
    pub name: String,              // "light_tank_chassis" / "infantry_equipment"
    pub equip_type: String,        // "armor" / "infantry" / "artillery"
    pub year: u32,
    pub is_archetype: bool,        // archetype 不可生产
    pub base_stats: EquipStats,    // 底盘自带基础属性
    pub slots: Vec<SlotDef>,       // 槽位定义(整件装备为空)
    pub default_modules: HashMap<String, String>, // slot_name → module_name(预设组合)
}

#[derive(Debug, Clone)]
pub struct SlotDef {
    pub name: String,                        // "turret_type_slot"
    pub required: bool,
    pub allowed_categories: Vec<String>,     // ["tank_light_turret_type"]
}

/// 模块定义(原版 00_tank_modules.txt 里的每个条目)
#[derive(Debug, Clone)]
pub struct ModuleDef {
    pub name: String,            // "tank_welded_armor"
    pub category: String,        // "tank_armor_type"
    pub add_stats: EquipStats,
    pub multiply_stats: EquipStats,
}

/// 可生产装备(挂在营 need 里的名字)
/// = 底盘 + 各槽位选定模块的汇总结果
#[derive(Debug, Clone)]
pub struct EquipmentDef {
    pub name: String,              // "infantry_equipment_1"(archetype 型号名)
    pub chassis: String,           // 指向 ChassisDef.name
    pub year: u32,
    pub equip_type: String,        // "armor" / "infantry" / "artillery"
    pub stats: EquipStats,         // 最终属性(加载时按公式算好缓存)
}

/// 给定底盘基础 + 模块选择, 按公式算最终装备属性(spec §3.3)
/// raw_stat = chassis_base + Σ module.add_stats
/// final_stat = raw_stat × Π (1 + module.multiply_stats)
pub fn compute_equipment_stats(chassis_base: &EquipStats, modules: &[ModuleDef]) -> EquipStats {
    let mut stats = chassis_base.clone();
    // 第1步: 加法汇总
    for m in modules {
        stats.add(&m.add_stats);
    }
    // 第2步: 乘法修正
    for m in modules {
        stats.multiply(&m.multiply_stats);
    }
    stats
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

    #[test]
    fn t_compute_stats_pure_base() {
        // 无模块: final = base
        let base = EquipStats { soft_attack: 3.0, defense: 20.0, ..Default::default() };
        let s = compute_equipment_stats(&base, &[]);
        assert!((s.soft_attack - 3.0).abs() < 1e-9);
        assert!((s.defense - 20.0).abs() < 1e-9);
    }

    #[test]
    fn t_compute_stats_add_only() {
        // base + add: soft 10 + 5 = 15
        let base = EquipStats { soft_attack: 10.0, ..Default::default() };
        let modules = vec![ModuleDef {
            name: "gun".into(), category: "x".into(),
            add_stats: EquipStats { soft_attack: 5.0, ..Default::default() },
            multiply_stats: EquipStats::default(),
        }];
        let s = compute_equipment_stats(&base, &modules);
        assert!((s.soft_attack - 15.0).abs() < 1e-9);
    }

    #[test]
    fn t_compute_stats_add_then_multiply() {
        // spec §3.3 例: base armor 10, welded_armor multiply +0.3, turret multiply +0.1
        // = 10 × 1.3 × 1.1 = 14.3
        let base = EquipStats { armor: 10.0, ..Default::default() };
        let modules = vec![
            ModuleDef {
                name: "welded".into(), category: "tank_armor_type".into(),
                add_stats: EquipStats::default(),
                multiply_stats: EquipStats { armor: 0.3, ..Default::default() },
            },
            ModuleDef {
                name: "turret".into(), category: "tank_light_turret_type".into(),
                add_stats: EquipStats::default(),
                multiply_stats: EquipStats { armor: 0.1, ..Default::default() },
            },
        ];
        let s = compute_equipment_stats(&base, &modules);
        assert!((s.armor - 14.3).abs() < 1e-9, "装甲汇总应 14.3, 实际 {}", s.armor);
    }

    #[test]
    fn t_chassis_default_modules_empty_for_integral() {
        // 整件装备(步兵)无槽位
        let c = ChassisDef {
            name: "infantry_equipment".into(), equip_type: "infantry".into(),
            year: 1936, is_archetype: true,
            base_stats: EquipStats::default(),
            slots: vec![], default_modules: HashMap::new(),
        };
        assert!(c.slots.is_empty());
        assert!(c.default_modules.is_empty());
    }
}
