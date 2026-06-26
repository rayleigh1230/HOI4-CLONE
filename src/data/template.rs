//! 师模板(division_template): 营列表 → Division 属性汇总
//!
//! 汇总公式(spec §4.3, 对齐 land-combat.md 第2节):
//! - 求和类(soft/hard/defense/breakthrough/combat_width/max_strength/manpower): Σ
//! - 加权混合(armor/piercing): 60%平均 + 40%最高
//! - 加权平均(hardness): 按 combat_width
//! - 加权平均(org): 按权重(支援连权重=1)

use crate::data::subunit::SubUnitDef;
use crate::data::{EquipStats, GameData};
use crate::parser::{Block, Value};
use std::collections::HashMap;

/// 师模板(原版 division_template)
#[derive(Debug, Clone, Default)]
pub struct DivisionTemplate {
    pub name: String,
    pub regiments: Vec<RegimentEntry>,  // 战斗营
    pub support: Vec<RegimentEntry>,    // 支援连
}

#[derive(Debug, Clone)]
pub struct RegimentEntry {
    pub sub_unit: String,
    pub x: u32,
    pub y: u32,
}

/// 汇总产出的中间结构(字段与现有 Division 的属性字段一一对应)
#[derive(Debug, Clone, Default)]
pub struct DivisionStats {
    pub soft_attack: f64,
    pub hard_attack: f64,
    pub defense: f64,
    pub breakthrough: f64,
    pub armor: f64,
    pub piercing: f64,
    pub hardness: f64,
    pub combat_width: f64,
    pub max_org: f64,
    pub max_strength: f64,
    pub manpower_need: f64,
    /// 师速度上限(km/h)。原版: 取最慢营的 max_speed(最小值)。无营默认 4.0。
    pub max_speed: f64,
    pub equipment_need: HashMap<String, f64>,
}

impl DivisionTemplate {
    /// 汇总成 Division 所需属性。返回 (统计, 未知营告警列表)。
    /// 未知营(不在 sub_units 里)进告警列表并跳过, 不 panic(对齐 Paradox 容错哲学)。
    pub fn to_division_stats(&self, data: &GameData) -> (DivisionStats, Vec<String>) {
        let mut warnings = Vec::new();

        // 收集战斗营: 已知营进入汇总, 未知营告警+跳过
        let regiments: Vec<(&SubUnitDef, EquipStats)> = self
            .regiments
            .iter()
            .filter_map(|r| match data.sub_units.get(&r.sub_unit) {
                Some(su) => {
                    let stats = su.combat_stats(data);
                    Some((su, stats))
                }
                None => {
                    warnings.push(format!(
                        "模板 \"{}\" 引用未知营 \"{}\", 已跳过",
                        self.name, r.sub_unit
                    ));
                    None
                }
            })
            .collect();

        let mut stats = DivisionStats::default();

        // 求和类: soft/hard/defense/breakthrough/combat_width/max_strength/manpower
        for (su, cs) in &regiments {
            stats.soft_attack += cs.soft_attack;
            stats.hard_attack += cs.hard_attack;
            stats.defense += cs.defense;
            stats.breakthrough += cs.breakthrough;
            stats.combat_width += su.combat_width;
            stats.max_strength += su.max_strength;
            stats.manpower_need += su.manpower;
        }

        // 师速度: 取最慢营(最小 max_speed)。无营默认 4.0。对齐原版。
        stats.max_speed = regiments.iter().map(|(su, _)| su.max_speed)
            .fold(4.0_f64, f64::min);

        // 加权混合(60%平均 + 40%最高): armor / piercing
        let n = regiments.len() as f64;
        if n > 0.0 {
            let armor_sum: f64 = regiments.iter().map(|(_, cs)| cs.armor).sum();
            let armor_max = regiments.iter().map(|(_, cs)| cs.armor).fold(0.0f64, f64::max);
            stats.armor = 0.6 * (armor_sum / n) + 0.4 * armor_max;

            let pierce_sum: f64 = regiments.iter().map(|(_, cs)| cs.piercing).sum();
            let pierce_max = regiments.iter().map(|(_, cs)| cs.piercing).fold(0.0f64, f64::max);
            stats.piercing = 0.6 * (pierce_sum / n) + 0.4 * pierce_max;
        }

        // 加权平均(按 combat_width): hardness
        let total_cw: f64 = regiments.iter().map(|(su, _)| su.combat_width).sum();
        if total_cw > 0.0 {
            stats.hardness = regiments
                .iter()
                .map(|(su, cs)| cs.hardness * su.combat_width)
                .sum::<f64>()
                / total_cw;
        }

        // 加权平均(按权重, 战斗营权重=combat_width): org
        let total_w: f64 = regiments.iter().map(|(su, _)| su.combat_width).sum();
        if total_w > 0.0 {
            stats.max_org = regiments
                .iter()
                .map(|(su, _)| su.max_organisation * su.combat_width)
                .sum::<f64>()
                / total_w;
        }

        // 支援连: 已知营汇总属性, 未知营告警+跳过
        for se in &self.support {
            match data.sub_units.get(&se.sub_unit) {
                Some(su) => {
                    let cs = su.combat_stats(data);
                    stats.soft_attack += cs.soft_attack;
                    stats.hard_attack += cs.hard_attack;
                    stats.defense += cs.defense;
                    stats.breakthrough += cs.breakthrough;
                    stats.max_strength += su.max_strength;
                    stats.manpower_need += su.manpower;
                    // battalion_mult 本次记录但不应用具体战斗修正(需匹配战斗营 category, 结构就位)
                }
                None => {
                    warnings.push(format!(
                        "模板 \"{}\" 支援连引用未知营 \"{}\", 已跳过",
                        self.name, se.sub_unit
                    ));
                }
            }
        }

        // 装备需求聚合
        for r in &self.regiments {
            if let Some(su) = data.sub_units.get(&r.sub_unit) {
                for (eq, qty) in &su.need {
                    *stats.equipment_need.entry(eq.clone()).or_insert(0.0) += qty;
                }
            }
        }
        for s in &self.support {
            if let Some(su) = data.sub_units.get(&s.sub_unit) {
                for (eq, qty) in &su.need {
                    *stats.equipment_need.entry(eq.clone()).or_insert(0.0) += qty;
                }
            }
        }

        (stats, warnings)
    }
}

/// 从 Block 解析一个 division_template
pub fn parse_template(block: &Block) -> DivisionTemplate {
    let name = block
        .fields
        .iter()
        .find(|f| f.key == "name")
        .and_then(|f| f.value.as_scalar_str())
        .unwrap_or("")
        .to_string();
    let regiments = find_block(block, "regiments")
        .map(|b| b.fields.iter().filter_map(parse_regiment_entry).collect())
        .unwrap_or_default();
    let support = find_block(block, "support")
        .map(|b| b.fields.iter().filter_map(parse_regiment_entry).collect())
        .unwrap_or_default();
    DivisionTemplate { name, regiments, support }
}

fn parse_regiment_entry(f: &crate::parser::Field) -> Option<RegimentEntry> {
    let Value::Block(rb) = &f.value else {
        return None;
    };
    let x = rb
        .fields
        .iter()
        .find(|rf| rf.key == "x")
        .and_then(|rf| rf.value.as_scalar_num())
        .unwrap_or(0.0) as u32;
    let y = rb
        .fields
        .iter()
        .find(|rf| rf.key == "y")
        .and_then(|rf| rf.value.as_scalar_num())
        .unwrap_or(0.0) as u32;
    Some(RegimentEntry { sub_unit: f.key.clone(), x, y })
}

fn find_block<'a>(block: &'a Block, key: &str) -> Option<&'a Block> {
    block
        .fields
        .iter()
        .find(|f| f.key == key)
        .and_then(|f| if let Value::Block(b) = &f.value { Some(b) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::equipment::EquipmentDef;

    #[test]
    fn t_unknown_subunit_warned_not_silent() {
        // 模板引用未知营 ghost_battalion → 应进告警列表 + 跳过, 不 panic, 已知营正常汇总
        let data = test_data();
        let tmpl = DivisionTemplate {
            name: "mixed".into(),
            regiments: vec![
                RegimentEntry { sub_unit: "infantry".into(), x: 0, y: 0 },
                RegimentEntry { sub_unit: "ghost_battalion".into(), x: 1, y: 0 },
            ],
            support: vec![],
        };
        let (stats, warnings) = tmpl.to_division_stats(&data);
        // 已知 infantry 营正常汇总(soft_attack 3)
        assert!((stats.soft_attack - 3.0).abs() < 1e-9, "已知营应正常汇总");
        // 未知营进告警列表
        assert_eq!(warnings.len(), 1, "应产生 1 条告警, 实际 {:?}", warnings);
        assert!(warnings[0].contains("ghost_battalion"), "告警应含未知营名");
    }

    /// 构造测试 GameData: 1个步兵装备 + 1个步兵营
    fn test_data() -> GameData {
        let mut d = GameData::default();
        d.equipment.insert(
            "infantry_equipment_1".into(),
            EquipmentDef {
                name: "infantry_equipment_1".into(),
                chassis: "x".into(),
                year: 1936,
                equip_type: "infantry".into(),
                stats: EquipStats {
                    soft_attack: 3.0,
                    defense: 20.0,
                    piercing: 1.0,
                    ..Default::default()
                },
                resources: Vec::new(),
            },
        );
        d.sub_units.insert(
            "infantry".into(),
            SubUnitDef {
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
            },
        );
        d
    }

    #[test]
    fn t_seven_infantry_division_stats() {
        // 7步师: 软攻 7×3=21, 防御 7×20=140, hp 7×25=175, 宽度 7×2=14
        let data = test_data();
        let tmpl = DivisionTemplate {
            name: "7inf".into(),
            regiments: vec![RegimentEntry { sub_unit: "infantry".into(), x: 0, y: 0 }; 7],
            support: vec![],
        };
        let (s, _warnings) = tmpl.to_division_stats(&data);
        assert!((s.soft_attack - 21.0).abs() < 1e-9, "soft 应 21, 实际 {}", s.soft_attack);
        assert!((s.defense - 140.0).abs() < 1e-9);
        assert!((s.max_strength - 175.0).abs() < 1e-9);
        assert!((s.combat_width - 14.0).abs() < 1e-9);
        assert!((s.manpower_need - 7000.0).abs() < 1e-9);
        // 装备需求: 7×100 = 700
        assert!(
            (s.equipment_need.get("infantry_equipment_1").copied().unwrap_or(0.0) - 700.0).abs()
                < 1e-9
        );
    }

    #[test]
    fn t_armor_weighted_blend() {
        // 加权混合: 60%平均+40%最高
        let mut data = test_data();
        // 加一个装甲营(armor=50)
        data.equipment.insert(
            "med_tank".into(),
            EquipmentDef {
                name: "med_tank".into(),
                chassis: "x".into(),
                year: 1936,
                equip_type: "armor".into(),
                stats: EquipStats {
                    armor: 50.0,
                    piercing: 60.0,
                    hardness: 0.9,
                    ..Default::default()
                },
                resources: Vec::new(),
            },
        );
        data.sub_units.insert(
            "medium_armor".into(),
            SubUnitDef {
                name: "medium_armor".into(),
                group: "armor".into(),
                categories: vec![],
                combat_width: 2.0,
                max_strength: 2.0,
                max_organisation: 10.0,
                default_morale: 0.3,
                manpower: 500.0,
                need: HashMap::from([("med_tank".into(), 50.0)]),
                battalion_mults: vec![],
                max_speed: 12.0,
            },
        );
        // 1步(armor0) + 1甲(armor50): avg=25, max=50 → 0.6×25+0.4×50 = 15+20 = 35
        let tmpl = DivisionTemplate {
            name: "mixed".into(),
            regiments: vec![
                RegimentEntry { sub_unit: "infantry".into(), x: 0, y: 0 },
                RegimentEntry { sub_unit: "medium_armor".into(), x: 0, y: 0 },
            ],
            support: vec![],
        };
        let (s, _warnings) = tmpl.to_division_stats(&data);
        assert!(
            (s.armor - 35.0).abs() < 1e-9,
            "装甲加权混合应 35, 实际 {}",
            s.armor
        );
    }

    #[test]
    fn t_parse_template_from_block() {
        let src = "division_template = {
            name = \"Test Div\"
            regiments = {
                infantry = { x = 0 y = 0 }
                infantry = { x = 1 y = 0 }
            }
        }";
        let b = crate::parser::parse(src).unwrap();
        let entry = &b.fields[0];
        let inner = if let Value::Block(ib) = &entry.value { ib } else { panic!() };
        let t = parse_template(inner);
        assert_eq!(t.name, "Test Div");
        assert_eq!(t.regiments.len(), 2);
        assert_eq!(t.regiments[0].sub_unit, "infantry");
    }

    #[test]
    fn t_support_zero_width() {
        // 支援连 combat_width=0, 不增加师总宽度
        let mut data = test_data();
        data.sub_units.insert(
            "engineer".into(),
            SubUnitDef {
                name: "engineer".into(),
                group: "support".into(),
                categories: vec![],
                combat_width: 0.0,
                max_strength: 2.0,
                max_organisation: 20.0,
                default_morale: 0.3,
                manpower: 300.0,
                need: HashMap::new(),
                battalion_mults: vec![],
                max_speed: 4.0,
            },
        );
        let tmpl = DivisionTemplate {
            name: "inf_eng".into(),
            regiments: vec![RegimentEntry { sub_unit: "infantry".into(), x: 0, y: 0 }; 7],
            support: vec![RegimentEntry { sub_unit: "engineer".into(), x: 0, y: 0 }],
        };
        let (s, _warnings) = tmpl.to_division_stats(&data);
        // 7步宽度 14, 加工兵(0)仍是 14
        assert!((s.combat_width - 14.0).abs() < 1e-9);
        // HP 增加: 7×25 + 2 = 177
        assert!((s.max_strength - 177.0).abs() < 1e-9);
    }
}
