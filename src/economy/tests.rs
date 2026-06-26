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
