//! 数据加载器: 原版文件 → GameData
//!
//! 加载顺序(依赖链, spec §5.1):
//!   模块(modules) → 底盘(chassis) → 装备(equipment)
//!   营(sub_units) → 模板(template)
//! 两遍扫描解决继承(spec §5.3): 第一遍注册名字+原始Block, 第二遍解析 parent/archetype 链。

use crate::data::equipment::{
    compute_equipment_stats, extract_stats, ChassisDef, EquipmentDef, ModuleDef, SlotDef,
};
use crate::data::subunit::{parse_sub_unit};
use crate::data::template::{parse_template};
use crate::data::GameData;
use crate::parser::{Block, Value};
use std::collections::HashMap;

/// 解析模块文件(00_tank_modules.txt 等)
/// 文件顶层是 equipment_modules = { 模块名 = {...} ... }
pub fn load_modules(data: &mut GameData, src: &str) {
    let block = match crate::parser::parse(src) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[data] 警告: 模块文件解析失败: {:?}", e);
            return;
        }
    };
    // 找 equipment_modules 块, 遍历其中的命名条目
    if let Some(modules_block) = find_block(&block, "equipment_modules") {
        for (name, entry_block) in named_entries(modules_block) {
            let module = parse_module(&name, &entry_block);
            data.modules.insert(name, module);
        }
    }
}

/// 解析单个模块: category + add_stats + multiply_stats
fn parse_module(name: &str, block: &Block) -> ModuleDef {
    let category = block
        .fields
        .iter()
        .find(|f| f.key == "category")
        .and_then(|f| f.value.as_scalar_str())
        .unwrap_or("")
        .to_string();
    let add_stats = find_block(block, "add_stats").map(extract_stats).unwrap_or_default();
    let multiply_stats = find_block(block, "multiply_stats").map(extract_stats).unwrap_or_default();
    ModuleDef {
        name: name.into(),
        category,
        add_stats,
        multiply_stats,
    }
}

/// 解析底盘文件(tank_chassis.txt / infantry.txt 等)
/// 文件顶层是 equipments = { 底盘名 = {...} ... }
/// 两遍扫描: 先存所有原始 Block, 再解析继承算最终属性
pub fn load_chassis(data: &mut GameData, src: &str) {
    let block = match crate::parser::parse(src) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[data] 警告: 底盘文件解析失败: {:?}", e);
            return;
        }
    };
    let Some(equip_block) = find_block(&block, "equipments") else {
        return;
    };

    // 第一遍: 收集所有底盘的原始 Block
    let raw: HashMap<String, Block> = named_entries(equip_block).into_iter().collect();

    // 第二遍: 解析每个底盘(先全部 parse_chassis 进 chassis 表)
    for (name, entry) in &raw {
        let chassis = parse_chassis(name, entry);
        data.chassis.insert(name.clone(), chassis);
    }

    // 第三遍: 具体型号(非 archetype)产出 EquipmentDef
    for (name, entry) in &raw {
        let Some(chassis) = data.chassis.get(name) else {
            continue;
        };
        if chassis.is_archetype {
            continue;
        }
        if let Some(equip) = build_equipment(chassis, entry, &raw, data) {
            data.equipment.insert(equip.name.clone(), equip);
        }
    }

    // 第四遍: 为每个 archetype 注册"最新型号"别名
    // 营的 need 引用 archetype 名(如 infantry_equipment), 但可生产型号是
    // infantry_equipment_1/_2/_3。别名指向 year 最大的型号, 让 need 能查到。
    let archetypes: Vec<String> = raw
        .keys()
        .filter(|n| data.chassis.get(*n).map(|c| c.is_archetype).unwrap_or(false))
        .cloned()
        .collect();
    for arch in archetypes {
        // 找该 archetype 下 year 最大的型号
        // 先快照(避免借用冲突: 迭代 equipment 时不能同时可变插入)
        let latest: Option<EquipmentDef> = data
            .equipment
            .values()
            .filter(|e| e.chassis == arch)
            .max_by_key(|e| e.year)
            .cloned();
        if let Some(latest_eq) = latest {
            // 仅当 archetype 名尚未直接存在于 equipment 表时才注册别名
            // (避免覆盖真实型号条目)
            data.equipment.entry(arch).or_insert(latest_eq);
        }
    }
}

/// 解析单个底盘定义
fn parse_chassis(name: &str, block: &Block) -> ChassisDef {
    let equip_type = block
        .fields
        .iter()
        .find(|f| f.key == "type")
        .and_then(|f| f.value.as_scalar_str())
        .unwrap_or("")
        .to_string();
    let year = block
        .fields
        .iter()
        .find(|f| f.key == "year")
        .and_then(|f| f.value.as_scalar_num())
        .unwrap_or(0.0) as u32;
    let is_archetype = block.fields.iter().any(|f| {
        f.key == "is_archetype"
            && matches!(f.value.as_scalar_str(), Some("yes") | Some("true"))
    });
    let base_stats = extract_stats(block);
    let slots = parse_slots(block);
    let default_modules = parse_default_modules(block);
    ChassisDef {
        name: name.into(),
        equip_type,
        year,
        is_archetype,
        base_stats,
        slots,
        default_modules,
    }
}

/// 解析 module_slots 块成 SlotDef 列表(仅 archetype 有; 型号是 inherit)
fn parse_slots(block: &Block) -> Vec<SlotDef> {
    let Some(slots_block) = find_block(block, "module_slots") else {
        return vec![];
    };
    // module_slots 可能是 inherit(标量, 无 fields) 或块
    if slots_block.fields.is_empty() {
        return vec![];
    }
    slots_block
        .fields
        .iter()
        .filter_map(|f| {
            let Value::Block(slot_inner) = &f.value else {
                return None;
            };
            let required = slot_inner.fields.iter().any(|sf| {
                sf.key == "required"
                    && matches!(sf.value.as_scalar_str(), Some("yes") | Some("true"))
            });
            let allowed = find_block(slot_inner, "allowed_module_categories")
                .map(|b| {
                    b.fields
                        .iter()
                        .filter_map(|f| f.value.as_scalar_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            Some(SlotDef {
                name: f.key.clone(),
                required,
                allowed_categories: allowed,
            })
        })
        .collect()
}

/// 解析 default_modules 块(slot → module)
fn parse_default_modules(block: &Block) -> HashMap<String, String> {
    let Some(dm_block) = find_block(block, "default_modules") else {
        return HashMap::new();
    };
    dm_block
        .fields
        .iter()
        .filter_map(|f| f.value.as_scalar_str().map(|m| (f.key.clone(), m.to_string())))
        .collect()
}

/// 给具体型号底盘算最终装备属性
/// 用 archetype 的 default_modules 找模块, 套汇总公式
/// 若无模块(整件装备/步兵), 直接用型号或 archetype 的 base_stats
fn build_equipment(
    chassis: &ChassisDef,
    entry: &Block,
    _raw: &HashMap<String, Block>,
    data: &GameData,
) -> Option<EquipmentDef> {
    // 找 archetype 名(具体型号通过 archetype = xxx 指向)
    let archetype_name = entry
        .fields
        .iter()
        .find(|f| f.key == "archetype")
        .and_then(|f| f.value.as_scalar_str())?;
    let archetype = data.chassis.get(archetype_name)?;

    // 收集模块: archetype 的 default_modules + 型号自身覆盖
    let mut chosen: HashMap<String, String> = archetype.default_modules.clone();
    for (k, v) in &chassis.default_modules {
        chosen.insert(k.clone(), v.clone());
    }

    // 查模块定义, 套汇总公式
    let modules: Vec<ModuleDef> = chosen
        .values()
        .filter_map(|mname| data.modules.get(mname).cloned())
        .collect();
    // base: 型号若直接写了战斗数值, 用型号的; 否则用 archetype 的
    let base = if has_own_stats(entry) {
        chassis.base_stats.clone()
    } else {
        archetype.base_stats.clone()
    };
    let stats = compute_equipment_stats(&base, &modules);

    Some(EquipmentDef {
        name: chassis.name.clone(),
        chassis: archetype_name.to_string(),
        year: chassis.year,
        equip_type: chassis.equip_type.clone(),
        stats,
    })
}

/// 型号是否直接写了战斗数值(armor_value/soft_attack 等)
fn has_own_stats(block: &Block) -> bool {
    block.fields.iter().any(|f| {
        matches!(
            f.key.as_str(),
            "armor_value" | "soft_attack" | "hard_attack" | "defense" | "breakthrough" | "ap_attack"
                | "hardness" | "build_cost_ic" | "maximum_speed" | "reliability"
        ) && f.value.as_scalar_num().map(|n| n != 0.0).unwrap_or(false)
    })
}

/// 解析营定义文件(units/*.txt)
/// 文件顶层是 sub_units = { 营名 = {...} ... }
pub fn load_sub_units(data: &mut GameData, src: &str) {
    let block = match crate::parser::parse(src) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[data] 警告: 营文件解析失败: {:?}", e);
            return;
        }
    };
    let Some(su_block) = find_block(&block, "sub_units") else {
        return;
    };
    for (name, entry) in named_entries(su_block) {
        let su = parse_sub_unit(&name, &entry);
        data.sub_units.insert(name, su);
    }
}

/// 解析模板文件(history/units/*.txt, 即 OOB 文件)
/// 一个文件可含多个 division_template 块
///
/// 容错: 原版 OOB 文件有时大括号不严格平衡(如 FRA_1936.txt 末尾少一个 }),
/// 原版 Paradox 引擎按块独立提取, 容忍文件级不平衡。这里对齐该行为:
/// 严格 parse 失败(UnexpectedEof)时, 尝试在末尾补 } 重试(最多补 3 个)。
/// 仅 OOB 数据加载走此容错路径; 运行时命令脚本仍用严格 parse。
pub fn load_templates(data: &mut GameData, src: &str) {
    let block = match crate::parser::parse(src) {
        Ok(b) => b,
        Err(crate::parser::ParseError::UnexpectedEof) => {
            // 容错恢复: 逐个补 } 重试, 直到成功或补满 3 个仍失败
            let mut recovered = None;
            for extra in 1..=3 {
                let patched = format!("{src}\n{}", "}".repeat(extra));
                if let Ok(b) = crate::parser::parse(&patched) {
                    eprintln!("[data] OOB 文件大括号不平衡, 补 {extra} 个 }} 后恢复解析");
                    recovered = Some(b);
                    break;
                }
            }
            match recovered {
                Some(b) => b,
                None => {
                    eprintln!("[data] 警告: 模板文件解析失败且容错恢复无效");
                    return;
                }
            }
        }
        Err(e) => {
            eprintln!("[data] 警告: 模板文件解析失败: {:?}", e);
            return;
        }
    };
    // 文件里可能有多个 division_template = {...}, 散布在顶层
    for f in &block.fields {
        if f.key == "division_template" {
            if let Value::Block(tb) = &f.value {
                let t = parse_template(tb);
                if !t.name.is_empty() {
                    data.templates.insert(t.name.clone(), t);
                }
            }
        }
    }
}

/// 统一加载入口: 按依赖链加载所有数据文件, 产出 GameData
pub fn load_all() -> GameData {
    let mut data = GameData::default();
    data.start_year = 1936;

    // 阶段1: 模块(无依赖)
    load_modules(&mut data, include_str!("../data_raw/modules/00_tank_modules.txt"));

    // 阶段2: 底盘(依赖模块) — 各装备文件
    load_chassis(&mut data, include_str!("../data_raw/equipment/infantry.txt"));
    load_chassis(&mut data, include_str!("../data_raw/equipment/artillery.txt"));
    load_chassis(&mut data, include_str!("../data_raw/equipment/tank_chassis.txt"));

    // 阶段3: 营定义(依赖装备)
    load_sub_units(&mut data, include_str!("../data_raw/units/infantry.txt"));
    load_sub_units(&mut data, include_str!("../data_raw/units/artillery.txt"));
    load_sub_units(&mut data, include_str!("../data_raw/units/medium_armor.txt"));
    load_sub_units(&mut data, include_str!("../data_raw/units/light_armor.txt"));

    // 阶段4: 模板(依赖营) — OOB 文件(history/units/*.txt)
    load_templates(&mut data, include_str!("../data_raw/history/GER.txt"));
    load_templates(&mut data, include_str!("../data_raw/history/FRA.txt"));

    data
}

// ===== Block 解读辅助(通用) =====

/// 在 block 的 fields 里找 key 对应的子块
pub fn find_block<'a>(block: &'a Block, key: &str) -> Option<&'a Block> {
    block
        .fields
        .iter()
        .find(|f| f.key == key)
        .and_then(|f| if let Value::Block(b) = &f.value { Some(b) } else { None })
}

/// 提取 block 里所有"命名条目": key 是名字, value 是 Block
/// 如 equipments = { infantry_equipment = {...}, infantry_equipment_1 = {...} }
pub fn named_entries(block: &Block) -> Vec<(String, Block)> {
    block
        .fields
        .iter()
        .filter_map(|f| {
            if let Value::Block(b) = &f.value {
                Some((f.key.clone(), b.clone()))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_load_modules_parses_category_and_stats() {
        let src = "equipment_modules = {
            test_gun = {
                category = tank_small_main_armament
                add_stats = { soft_attack = 8 }
                multiply_stats = { build_cost_ic = 0.1 }
            }
        }";
        let mut data = GameData::default();
        load_modules(&mut data, src);
        let m = data.modules.get("test_gun").expect("应解析出 test_gun");
        assert_eq!(m.category, "tank_small_main_armament");
        assert!((m.add_stats.soft_attack - 8.0).abs() < 1e-9);
        assert!((m.multiply_stats.build_cost_ic - 0.1).abs() < 1e-9);
    }

    #[test]
    fn t_load_chassis_archetype_and_variant() {
        // 简化: archetype + 一个具体型号(直接写数值, 不走模块)
        let src = "equipments = {
            test_weapon = {
                type = infantry
                is_archetype = yes
                soft_attack = 3
                defense = 20
            }
            test_weapon_1 = {
                archetype = test_weapon
                year = 1936
                soft_attack = 3.5
                defense = 22
            }
        }";
        let mut data = GameData::default();
        load_chassis(&mut data, src);
        // archetype 进 chassis 表
        assert!(data.chassis.contains_key("test_weapon"));
        // 具体型号进 equipment 表
        let e = data.equipment.get("test_weapon_1").expect("应产出 test_weapon_1");
        assert_eq!(e.chassis, "test_weapon");
        assert_eq!(e.year, 1936);
        // 型号直接写数值 → 用型号的(has_own_stats)
        assert!((e.stats.soft_attack - 3.5).abs() < 1e-9);
        assert!((e.stats.defense - 22.0).abs() < 1e-9);
    }

    #[test]
    fn t_load_chassis_variant_inherits_archetype_stats() {
        // 型号不写数值 → 继承 archetype 的
        let src = "equipments = {
            base_w = { type = infantry is_archetype = yes soft_attack = 3 }
            base_w_1 = { archetype = base_w year = 1936 }
        }";
        let mut data = GameData::default();
        load_chassis(&mut data, src);
        let e = data.equipment.get("base_w_1").expect("应产出 base_w_1");
        assert!((e.stats.soft_attack - 3.0).abs() < 1e-9, "应继承 archetype soft=3");
    }

    #[test]
    fn t_load_real_infantry_file() {
        // 加载真实原版 infantry.txt
        let src = include_str!("../data_raw/equipment/infantry.txt");
        let mut data = GameData::default();
        load_chassis(&mut data, src);
        // infantry_equipment 是 archetype
        assert!(data
            .chassis
            .get("infantry_equipment")
            .map(|c| c.is_archetype)
            .unwrap_or(false));
        // 应至少解析出一个可生产型号(infantry_equipment_* 系列)
        let has_variant = data
            .equipment
            .keys()
            .any(|k| k.starts_with("infantry_equipment_"));
        assert!(has_variant, "应解析出 infantry_equipment_* 型号, 实际装备: {:?}",
            data.equipment.keys().collect::<Vec<_>>());
    }

    #[test]
    fn t_load_sub_units_infantry() {
        let src = "sub_units = {
            infantry = {
                group = infantry
                combat_width = 2
                max_strength = 25
                manpower = 1000
                need = { infantry_equipment_1 = 100 }
            }
        }";
        let mut data = GameData::default();
        load_sub_units(&mut data, src);
        let su = data.sub_units.get("infantry").expect("应解析出 infantry 营");
        assert_eq!(su.group, "infantry");
        assert!((su.combat_width - 2.0).abs() < 1e-9);
        assert!((su.max_strength - 25.0).abs() < 1e-9);
    }

    #[test]
    fn t_load_real_units_infantry_file() {
        let src = include_str!("../data_raw/units/infantry.txt");
        let mut data = GameData::default();
        load_sub_units(&mut data, src);
        // 原版 units/infantry.txt 含 infantry 营
        let su = data.sub_units.get("infantry");
        assert!(su.is_some(), "应解析出 infantry 营");
        if let Some(su) = su {
            assert!((su.combat_width - 2.0).abs() < 1e-9);
            assert!((su.max_strength - 25.0).abs() < 1e-9);
        }
    }

    #[test]
    fn t_load_templates_from_block() {
        let src = "division_template = {
            name = \"7 Infantry\"
            regiments = {
                infantry = { x = 0 y = 0 }
                infantry = { x = 1 y = 0 }
            }
        }
        division_template = {
            name = \"Armor\"
            regiments = { medium_armor = { x = 0 y = 0 } }
        }";
        let mut data = GameData::default();
        load_templates(&mut data, src);
        assert!(data.templates.contains_key("7 Infantry"));
        assert!(data.templates.contains_key("Armor"));
    }

    #[test]
    fn t_load_all_produces_populated_data() {
        // 端到端: load_all 应产出非空的 GameData
        let data = crate::data::loader::load_all();
        assert!(!data.chassis.is_empty(), "应加载出底盘");
        assert!(!data.equipment.is_empty(), "应加载出装备");
        assert!(!data.sub_units.is_empty(), "应加载出营");
        // infantry_equipment 系列(可生产型号)必须存在(步兵营 need 它)
        let has_inf_eq = data
            .equipment
            .keys()
            .any(|k| k.starts_with("infantry_equipment_"));
        assert!(has_inf_eq, "应存在 infantry_equipment_* 型号, 实际装备: {:?}",
            data.equipment.keys().collect::<Vec<_>>());
    }

    #[test]
    fn t_load_all_has_light_armor_subunit() {
        // light_armor 营应加载(装甲师用)
        let data = crate::data::loader::load_all();
        assert!(data.sub_units.contains_key("light_armor"), "应加载 light_armor 营");
    }

    #[test]
    fn t_load_all_has_fra_templates() {
        // FRA OOB 模板应加载
        let data = crate::data::loader::load_all();
        assert!(data.templates.contains_key("Division d'Infanterie"), "应加载 FRA 步兵模板");
    }
}
