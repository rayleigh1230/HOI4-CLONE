//! 营定义(sub_units): 结构属性 + need 装备 + battalion_mult
//!
//! 营的战斗属性来自两处:
//! - 结构属性(hp/org/width/manpower): sub_unit 定义自身
//! - 战斗属性(攻/防/装甲): 来自 need 装备 × 件数比例

use crate::data::{EquipStats, GameData};
use crate::parser::{Block, Value};
use std::collections::HashMap;

/// 营定义(原版 sub_units 里的一个条目)
#[derive(Debug, Clone, Default)]
pub struct SubUnitDef {
    pub name: String,           // "infantry" / "medium_armor" / "engineer"
    pub group: String,          // "infantry" / "armor" / "support"
    pub categories: Vec<String>,// ["category_light_infantry"](battalion_mult 匹配用)
    pub combat_width: f64,
    pub max_strength: f64,      // HP
    pub max_organisation: f64,
    pub default_morale: f64,
    pub manpower: f64,
    /// 师速度上限(km/h)。原版: 取师内最慢营的 max_speed。
    /// infantry=4, light_armor=12 等。解析 sub_units 文件的 max_speed = 字段。
    pub max_speed: f64,
    /// 满编需求: equipment_name → 件数
    pub need: HashMap<String, f64>,
    /// (支援连)对其它营的修正
    pub battalion_mults: Vec<BattalionMult>,
}

/// 支援连的 battalion_mult(给匹配 category 的营加成)
#[derive(Debug, Clone)]
pub struct BattalionMult {
    pub category: String,   // "category_light_infantry"
    pub stat: String,       // "entrenchment" / "max_strength"
    pub value: f64,
    pub add: bool,          // true=加法, false=乘法
}

impl SubUnitDef {
    /// 营的战斗属性(从 need 装备算, 查 GameData)
    ///
    /// 两类属性计算方式不同:
    /// - 攻/防/突(soft/hard/defense/breakthrough): 按件数 × need_qty/100
    /// - 装甲/穿甲/硬度(armor/piercing/hardness): 取装备值不×件数(师层加权混合)
    ///
    /// 接收 &GameData 而非闭包: 闭包返回借用会触发 HRTB 'static 约束,
    /// 直接传 GameData 让引用生命周期自然绑定(GameData 在调用者处存活)。
    pub fn combat_stats(&self, data: &GameData) -> EquipStats {
        let mut s = EquipStats::default();
        for (eq_name, qty) in &self.need {
            if let Some(eq) = data.equipment.get(eq_name) {
                let factor = qty / 100.0;
                // 按件数比例
                s.soft_attack += eq.stats.soft_attack * factor;
                s.hard_attack += eq.stats.hard_attack * factor;
                s.defense += eq.stats.defense * factor;
                s.breakthrough += eq.stats.breakthrough * factor;
                // 不×件数(营固有等级)
                s.armor += eq.stats.armor;
                s.piercing += eq.stats.piercing;
                s.hardness += eq.stats.hardness;
            }
        }
        s
    }
}

/// 从 Block 解析一个 sub_unit
pub fn parse_sub_unit(name: &str, block: &Block) -> SubUnitDef {
    let num = |k: &str| {
        block
            .fields
            .iter()
            .find(|f| f.key == k)
            .and_then(|f| f.value.as_scalar_num())
            .unwrap_or(0.0)
    };
    let str_val = |k: &str| {
        block
            .fields
            .iter()
            .find(|f| f.key == k)
            .and_then(|f| f.value.as_scalar_str())
            .unwrap_or("")
            .to_string()
    };

    let group = str_val("group");
    let categories = block
        .fields
        .iter()
        .find(|f| f.key == "categories")
        .and_then(|f| if let Value::Block(b) = &f.value { Some(b) } else { None })
        .map(|b| {
            b.fields
                .iter()
                .filter_map(|f| f.value.as_scalar_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let need = parse_need(block);
    let battalion_mults = parse_battalion_mults(block);
    // max_speed: 文件有则用, 无则默认 4.0(原版最慢步兵速度, 保证有速度不卡死)
    let max_speed = block
        .fields
        .iter()
        .find(|f| f.key == "max_speed")
        .and_then(|f| f.value.as_scalar_num())
        .unwrap_or(4.0);

    SubUnitDef {
        name: name.into(),
        group,
        categories,
        combat_width: num("combat_width"),
        max_strength: num("max_strength"),
        max_organisation: num("max_organisation"),
        default_morale: num("default_morale"),
        manpower: num("manpower"),
        max_speed,
        need,
        battalion_mults,
    }
}

/// 解析 need = { infantry_equipment = 100 } 块
fn parse_need(block: &Block) -> HashMap<String, f64> {
    let mut need = HashMap::new();
    if let Some(nb) = block
        .fields
        .iter()
        .find(|f| f.key == "need")
        .and_then(|f| if let Value::Block(b) = &f.value { Some(b) } else { None })
    {
        for f in &nb.fields {
            if let Some(qty) = f.value.as_scalar_num() {
                need.insert(f.key.clone(), qty);
            }
        }
    }
    need
}

/// 解析 battalion_mult 块(可能有多个)
fn parse_battalion_mults(block: &Block) -> Vec<BattalionMult> {
    block
        .fields
        .iter()
        .filter(|f| f.key == "battalion_mult")
        .filter_map(|f| {
            if let Value::Block(b) = &f.value {
                let category = b
                    .fields
                    .iter()
                    .find(|bf| bf.key == "category")
                    .and_then(|bf| bf.value.as_scalar_str())
                    .unwrap_or("")
                    .to_string();
                // add = yes(lexer 转成 "true")或缺省(乘法)
                let is_add = b.fields.iter().any(|bf| {
                    bf.key == "add" && matches!(bf.value.as_scalar_str(), Some("yes") | Some("true"))
                });
                // category/add 之后的数值字段是 stat=value
                b.fields
                    .iter()
                    .filter(|bf| !matches!(bf.key.as_str(), "category" | "add"))
                    .filter_map(|bf| {
                        bf.value.as_scalar_num().map(|v| BattalionMult {
                            category: category.clone(),
                            stat: bf.key.clone(),
                            value: v,
                            add: is_add,
                        })
                    })
                    .next()
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::equipment::EquipmentDef;
    use crate::data::GameData;

    fn inf_eq() -> EquipmentDef {
        EquipmentDef {
            name: "infantry_equipment_1".into(),
            chassis: "infantry_equipment".into(),
            year: 1936,
            equip_type: "infantry".into(),
            stats: EquipStats {
                soft_attack: 3.0,
                defense: 20.0,
                piercing: 1.0,
                ..Default::default()
            },
            resources: Vec::new(),
        }
    }

    #[test]
    fn t_combat_stats_infantry_battalion() {
        // infantry 营 need infantry_equipment×100
        // soft = 3 × 100/100 = 3; defense = 20; piercing = 1(不×件数)
        let su = SubUnitDef {
            name: "infantry".into(),
            group: "infantry".into(),
            categories: vec![],
            combat_width: 2.0,
            max_strength: 25.0,
            max_organisation: 60.0,
            default_morale: 0.3,
            manpower: 1000.0,
            need: HashMap::from([("infantry_equipment_1".into(), 100.0)]),
            battalion_mults: vec![],
            max_speed: 4.0,
        };
        let mut data = GameData::default();
        data.equipment.insert("infantry_equipment_1".into(), inf_eq());
        let s = su.combat_stats(&data);
        assert!((s.soft_attack - 3.0).abs() < 1e-9);
        assert!((s.defense - 20.0).abs() < 1e-9);
        assert!((s.piercing - 1.0).abs() < 1e-9);
    }

    #[test]
    fn t_parse_sub_unit_from_block() {
        let src = "infantry = {
            group = infantry
            combat_width = 2
            max_strength = 25
            max_organisation = 60
            default_morale = 0.3
            manpower = 1000
            need = { infantry_equipment = 100 }
        }";
        let b = crate::parser::parse(src).unwrap();
        // 顶层有一个 infantry 条目
        let entry = &b.fields[0];
        let inner = if let Value::Block(ib) = &entry.value { ib } else { panic!() };
        let su = parse_sub_unit("infantry", inner);
        assert_eq!(su.group, "infantry");
        assert!((su.combat_width - 2.0).abs() < 1e-9);
        assert!((su.max_strength - 25.0).abs() < 1e-9);
        assert!((su.need.get("infantry_equipment").copied().unwrap_or(0.0) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn t_parse_battalion_mult() {
        let src = "engineer = {
            group = support
            battalion_mult = {
                category = category_light_infantry
                entrenchment = 0.20
                add = yes
            }
        }";
        let b = crate::parser::parse(src).unwrap();
        let entry = &b.fields[0];
        let inner = if let Value::Block(ib) = &entry.value { ib } else { panic!() };
        let su = parse_sub_unit("engineer", inner);
        assert_eq!(su.battalion_mults.len(), 1);
        let m = &su.battalion_mults[0];
        assert_eq!(m.category, "category_light_infantry");
        assert_eq!(m.stat, "entrenchment");
        assert!((m.value - 0.20).abs() < 1e-9);
        assert!(m.add);
    }
}
