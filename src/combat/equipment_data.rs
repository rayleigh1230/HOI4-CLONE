//! 原版装备数据(1936, 取自 common/units/equipment/*.txt)
//!
//! 单件装备属性。师的属性 = 装备属性 × 件数/100(need=100 为满编营)。

#[derive(Debug, Clone, Copy)]
pub struct EquipmentDef {
    pub name: &'static str,
    pub soft_attack: f64,
    pub hard_attack: f64,
    pub defense: f64,
    pub breakthrough: f64,
    pub armor: f64,
    pub piercing: f64,
    pub hardness: f64,
    pub build_cost_ic: f64,
    /// 生产所需资源(原版 `resources = { steel = 2 }`), 如 &[("steel", 2.0)]
    pub resources: &'static [(&'static str, f64)],
}

/// 1936 年装备(实证自原版文件)
/// infantry_equipment: soft=3 hard=0.5 defense=20 break=2 armor=0 pierce=1 hardness=0 ic=0.43
/// artillery: soft=25 hard=4 defense=8 break=12 armor=0 pierce=8 hardness=0 ic=3.5
/// light_tank_equipment_1: soft=8 hard=6 defense=4 break=26 armor=14 pierce=14 hardness=0.9 ic=2.7
/// medium_tank_equipment_1: soft=19 hard=14 defense=5 break=36 armor=60 pierce=61 hardness=0.9 ic=11
/// heavy_tank_equipment_1: soft=20 hard=22 defense=8 break=48 armor=100 pierce=100 hardness=0.95 ic=15
pub static EQUIPMENT: &[EquipmentDef] = &[
    EquipmentDef {
        name: "infantry_equipment",
        soft_attack: 3.0,
        hard_attack: 0.5,
        defense: 20.0,
        breakthrough: 2.0,
        armor: 0.0,
        piercing: 1.0,
        hardness: 0.0,
        build_cost_ic: 0.43,
        resources: &[("steel", 2.0)],
    },
    EquipmentDef {
        name: "artillery",
        soft_attack: 25.0,
        hard_attack: 4.0,
        defense: 8.0,
        breakthrough: 12.0,
        armor: 0.0,
        piercing: 8.0,
        hardness: 0.0,
        build_cost_ic: 3.5,
        resources: &[("tungsten", 1.0), ("steel", 2.0)],
    },
    EquipmentDef {
        name: "light_tank",
        soft_attack: 8.0,
        hard_attack: 6.0,
        defense: 4.0,
        breakthrough: 26.0,
        armor: 14.0,
        piercing: 14.0,
        hardness: 0.9,
        build_cost_ic: 2.7,
        resources: &[("steel", 2.0), ("rubber", 1.0)],
    },
    EquipmentDef {
        name: "medium_tank",
        soft_attack: 19.0,
        hard_attack: 14.0,
        defense: 5.0,
        breakthrough: 36.0,
        armor: 60.0,
        piercing: 61.0,
        hardness: 0.9,
        build_cost_ic: 11.0,
        resources: &[("steel", 3.0), ("rubber", 1.0)],
    },
    EquipmentDef {
        name: "heavy_tank",
        soft_attack: 20.0,
        hard_attack: 22.0,
        defense: 8.0,
        breakthrough: 48.0,
        armor: 100.0,
        piercing: 100.0,
        hardness: 0.95,
        build_cost_ic: 15.0,
        resources: &[("steel", 4.0), ("chromium", 1.0)],
    },
];

/// 营级基础属性(取自 common/units/infantry.txt 等)
/// 每营: max_strength=25, max_org=60, combat_width=2, manpower=1000, need=100件
pub const BATTALION_HP: f64 = 25.0;
pub const BATTALION_ORG: f64 = 60.0;
pub const BATTALION_WIDTH: f64 = 2.0;
pub const BATTALION_MANPOWER: f64 = 1000.0;
pub const BATTALION_EQUIPMENT_NEED: f64 = 100.0;

/// 按装备名查装备定义
pub fn find_equipment(name: &str) -> Option<&'static EquipmentDef> {
    EQUIPMENT.iter().find(|e| e.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_find_infantry_equipment() {
        let e = find_equipment("infantry_equipment").unwrap();
        assert_eq!(e.soft_attack, 3.0);
        assert_eq!(e.defense, 20.0);
    }

    #[test]
    fn t_seven_inf_battalions_stats() {
        // 7个步兵营 = 标准7步师
        // 每营 100 件, 单件 soft=3 → 营 soft = 3×100/100 = 3; 7营 = 21
        let e = find_equipment("infantry_equipment").unwrap();
        let n = 7.0;
        let soft = n * e.soft_attack; // 7×3=21
        let defense = n * e.defense; // 7×20=140
        let hp = n * BATTALION_HP; // 7×25=175
        assert!((soft - 21.0).abs() < 1e-9);
        assert!((defense - 140.0).abs() < 1e-9);
        assert!((hp - 175.0).abs() < 1e-9);
    }
}
