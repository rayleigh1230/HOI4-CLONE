use super::*;

#[test]
fn t_variant_chassis_strips_numeric_suffix() {
    assert_eq!(variant_chassis("infantry_equipment_1"), "infantry_equipment");
    assert_eq!(variant_chassis("light_tank_chassis_1"), "light_tank_chassis");
    assert_eq!(variant_chassis("artillery_equipment_2"), "artillery_equipment");
}

#[test]
fn t_variant_chassis_handles_no_suffix() {
    assert_eq!(variant_chassis("infantry_equipment"), "infantry_equipment");
    assert_eq!(variant_chassis("light_tank_chassis"), "light_tank_chassis");
}

#[test]
fn t_variant_chassis_handles_underscore_non_numeric() {
    // 末段是字母(如 "chassis")不应被剥
    assert_eq!(variant_chassis("foo_bar"), "foo_bar");
}

use super::{ProductionLine, EFFICIENCY_START, SLOTS_PER_LINE};

#[test]
fn t_set_active_fills_slots_from_front() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(3);
    assert_eq!(line.active_count, 3);
    assert!(line.slots[0].active);
    assert!(line.slots[1].active);
    assert!(line.slots[2].active);
    assert!(!line.slots[3].active);
    assert!((line.slots[0].efficiency - EFFICIENCY_START).abs() < 1e-9);
    assert!((line.slots[2].efficiency - EFFICIENCY_START).abs() < 1e-9);
    assert!(line.slots[3].efficiency == 0.0);
}

#[test]
fn t_active_count_clamped_at_15() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(99);
    assert_eq!(line.active_count, SLOTS_PER_LINE as u32);
    assert!(line.slots.iter().all(|s| s.active));
}

#[test]
fn t_reduce_factories_keeps_efficiency() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(5);
    line.slots[4].efficiency = 0.40;
    line.set_active(3);  // 关闭 slot 3,4
    assert!(!line.slots[4].active);
    assert!((line.slots[4].efficiency - 0.40).abs() < 1e-9, "关闭的槽应保留 efficiency");
    assert_eq!(line.active_count, 3);
}

#[test]
fn t_reactivate_preserves_efficiency() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(5);
    line.slots[4].efficiency = 0.40;
    line.set_active(3);  // 关闭 slot 4(保留 eff 0.40)
    line.set_active(5);  // 重新激活 slot 4
    assert!(line.slots[4].active);
    assert!((line.slots[4].efficiency - 0.40).abs() < 1e-9, "重激活应保留 0.40 不重置");
}

#[test]
fn t_chassis_derived_from_variant() {
    let line = ProductionLine::new(0, "light_tank_chassis_1".into());
    assert_eq!(line.chassis, "light_tank_chassis");
    assert_eq!(line.variant, "light_tank_chassis_1");
}

// === Phase 3 测试: 资源惩罚 + change_line_variant ===
use super::production::{change_line_variant, resource_penalty};
use crate::data::equipment::EquipmentDef as GameEq;
use crate::data::EquipStats;
use std::collections::HashMap;

fn mock_equip(name: &str, bc: f64, resources: Vec<(&str, f64)>) -> GameEq {
    GameEq {
        name: name.into(),
        chassis: crate::economy::variant_chassis(name).to_string(),
        year: 1936,
        equip_type: "infantry".into(),
        stats: EquipStats {
            build_cost_ic: bc,
            ..Default::default()
        },
        resources: resources.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
    }
}

#[test]
fn t_no_penalty_when_resources_sufficient() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(5);
    let eq = mock_equip("infantry_equipment_1", 0.43, vec![("steel", 2.0)]);
    let mut res = HashMap::new();
    res.insert("steel".into(), 100.0);
    let mult = resource_penalty(&line, &eq, &res);
    assert!(
        (mult - 1.0).abs() < 1e-9,
        "资源充足应无惩罚, mult={}",
        mult
    );
}

#[test]
fn t_steel_shortage_5pct_per_unit() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(10); // 10 工厂 × 钢 2 = 需 20 钢
    let eq = mock_equip("infantry_equipment_1", 0.43, vec![("steel", 2.0)]);
    let mut res = HashMap::new();
    res.insert("steel".into(), 18.0); // 缺 2 → -10%
    let mult = resource_penalty(&line, &eq, &res);
    assert!(
        (mult - 0.90).abs() < 1e-9,
        "缺 2 钢应 -10%, mult={}",
        mult
    );
}

#[test]
fn t_multiple_resource_penalties_stack() {
    let mut line = ProductionLine::new(0, "artillery_1".into());
    line.set_active(3); // 钨 3×1=3, 钢 3×2=6
    let eq = mock_equip(
        "artillery_1",
        3.5,
        vec![("tungsten", 1.0), ("steel", 2.0)],
    );
    let mut res = HashMap::new();
    res.insert("tungsten".into(), 2.0); // 缺 1 → -5%
    res.insert("steel".into(), 5.0); // 缺 1 → -5%
    let mult = resource_penalty(&line, &eq, &res);
    assert!((mult - 0.90).abs() < 1e-9, "总 -10%, mult={}", mult);
}

#[test]
fn t_half_penalty_when_one_resource_fully_short() {
    // 单一资源全部短缺(缺 10 单位)→ -50%
    let mut line = ProductionLine::new(0, "artillery_1".into());
    line.set_active(10);
    let eq = mock_equip("artillery_1", 3.5, vec![("tungsten", 1.0)]);
    let res = HashMap::new(); // 0 钨
    let mult = resource_penalty(&line, &eq, &res);
    assert!(
        (mult - 0.50).abs() < 1e-9,
        "缺 10 钨应 -50%, mult={}",
        mult
    );
}

#[test]
fn t_multiplier_clamps_to_zero_on_severe_shortage() {
    // 15 工厂(上限)各需 2 钨, 总缺 30 → penalty=1.5(>1) → mult 被 .max(0.0) 钳为 0
    let mut line = ProductionLine::new(0, "artillery_1".into());
    line.set_active(20); // 实际激活上限 15
    let eq = mock_equip("artillery_1", 3.5, vec![("tungsten", 2.0)]);
    let res = HashMap::new(); // 0 钨
    let mult = resource_penalty(&line, &eq, &res);
    assert!(
        mult.abs() < 1e-9,
        "短缺 penalty≥1.0 时应钳为 0, mult={}",
        mult
    );
}

#[test]
fn t_variant_change_keeps_90pct() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(3);
    line.slots[0].efficiency = 0.30;
    line.slots[1].efficiency = 0.30;
    line.slots[2].efficiency = 0.30;
    change_line_variant(&mut line, "infantry_equipment_2");
    assert!(
        (line.slots[0].efficiency - 0.27).abs() < 1e-9,
        "应保留 90%: 0.30×0.9=0.27"
    );
    assert_eq!(line.variant, "infantry_equipment_2");
    assert_eq!(line.chassis, "infantry_equipment");
}

#[test]
fn t_chassis_change_resets_to_start() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(3);
    line.slots[0].efficiency = 0.40;
    change_line_variant(&mut line, "artillery_1");
    // 不同 chassis → 重置 active 槽到 EFFICIENCY_START
    assert!(
        (line.slots[0].efficiency - EFFICIENCY_START).abs() < 1e-9,
        "active 槽应重置到 START({}), 实际 {}",
        EFFICIENCY_START,
        line.slots[0].efficiency
    );
    assert_eq!(line.chassis, "artillery");
}

#[test]
fn t_change_to_same_variant_noop() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(2);
    line.slots[0].efficiency = 0.40;
    change_line_variant(&mut line, "infantry_equipment_1");
    assert!(
        (line.slots[0].efficiency - 0.40).abs() < 1e-9,
        "同 variant 不应变"
    );
}
