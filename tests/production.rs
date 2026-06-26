//! 生产系统端到端 integration
//! 验证: 建国家+State(含 steel 资源) → 建生产线 → 跑 N 日 → 库存积累 + 效率达到 ~50%
//!       → 删 State 的 steel → 产出降为 0(资源耗尽, 15 工厂×钢 2 = 缺 30 → penalty=1.5 → mult 钳为 0)
//!       → variant 切换端到端

use hoi4_clone::ast::lower::lower_effects;
use hoi4_clone::commands::register_all;
use hoi4_clone::combat::commands::register;
use hoi4_clone::economy::EFFICIENCY_MAX;
use hoi4_clone::parser::parse;
use hoi4_clone::runtime::clock::GameClock;
use hoi4_clone::runtime::entities::{Country, State};
use hoi4_clone::runtime::{Interpreter, Registry, World};

fn setup_world_with_ger_production() -> (Interpreter, World) {
    let mut reg = Registry::new();
    register_all(&mut reg);
    register(&mut reg);
    let interp = Interpreter::new(reg);
    let mut w = World::new();
    w.player_tag = "GER".into();
    // GER 国家
    let mut ger = Country::default();
    ger.tag = "GER".into();
    ger.owned_states = vec![1];
    w.countries.insert("GER".into(), ger);
    // State 1 含 steel=100
    let mut state = State::default();
    state.id = 1;
    state.owner = "GER".into();
    state.controller = "GER".into();
    state.resources.insert("steel".into(), 100.0);
    w.states.insert(1, state);
    (interp, w)
}

fn run_script(interp: &Interpreter, world: &mut World, src: &str) {
    let block = parse(src).expect("parse failed");
    let effs = lower_effects(&block);
    interp.run(&effs, world);
    // 失败时打印错误日志, 让测试可观察
    if !world.error_log.is_empty() {
        eprintln!("error_log: {:?}", world.error_log);
    }
}

#[test]
fn t_production_accumulates_stockpile_over_days() {
    let (interp, mut world) = setup_world_with_ger_production();
    run_script(
        &interp,
        &mut world,
        "create_production_line = { owner = GER variant = infantry_equipment_1 factories = 5 }",
    );
    assert!(
        !world.countries["GER"].production_lines.is_empty(),
        "应已创建生产线, error_log={:?}",
        world.error_log
    );

    let stock_before: f64 = world.countries["GER"].equipment_stockpile.values().sum();
    GameClock::advance(&interp, &mut world, 24 * 10); // 10 天
    let stock_after: f64 = world.countries["GER"].equipment_stockpile.values().sum();
    assert!(
        stock_after > stock_before,
        "10 天后库存应增长: before={}, after={}",
        stock_before,
        stock_after
    );
}

#[test]
fn t_efficiency_reaches_near_cap_after_long_run() {
    let (interp, mut world) = setup_world_with_ger_production();
    run_script(
        &interp,
        &mut world,
        "create_production_line = { owner = GER variant = infantry_equipment_1 factories = 3 }",
    );
    GameClock::advance(&interp, &mut world, 24 * 60); // 60 天

    let line = &world.countries["GER"].production_lines[0];
    let max_slot_eff = line
        .slots
        .iter()
        .filter(|s| s.active)
        .map(|s| s.efficiency)
        .fold(0.0_f64, f64::max);
    assert!(
        max_slot_eff > 0.40,
        "60 天后效率应接近 cap 0.50, 实际 {}",
        max_slot_eff
    );
    assert!(
        max_slot_eff <= EFFICIENCY_MAX + 1e-9,
        "效率不应超 cap, 实际 {}",
        max_slot_eff
    );
}

#[test]
fn t_no_output_when_no_steel() {
    // 原版 PRODUCTION_RESOURCE_LACK_PENALTY: 每缺 1 单位资源 → -5% 产出
    // 15 工厂 × (steel=2/工厂) = 需 30 steel; steel=0 → 缺 30 → penalty=1.5 → mult=max(0,1-1.5)=0
    // (用 15 工厂而非 5: 5 工厂仅缺 10 → penalty=0.5 → mult=0.5 仍有一半产出)
    let (interp, mut world) = setup_world_with_ger_production();
    run_script(
        &interp,
        &mut world,
        "create_production_line = { owner = GER variant = infantry_equipment_1 factories = 15 }",
    );
    // 清空 steel
    world.states.get_mut(&1).unwrap().resources.clear();

    let stock_before: f64 = world.countries["GER"].equipment_stockpile.values().sum();
    GameClock::advance(&interp, &mut world, 24 * 5);
    let stock_after: f64 = world.countries["GER"].equipment_stockpile.values().sum();
    assert!(
        (stock_after - stock_before).abs() < 1e-9,
        "15 工厂缺 30 steel 应产出 0, before={}, after={}",
        stock_before,
        stock_after
    );
}

#[test]
fn t_variant_change_keeps_90pct_end_to_end() {
    let (interp, mut world) = setup_world_with_ger_production();
    run_script(
        &interp,
        &mut world,
        "create_production_line = { owner = GER variant = infantry_equipment_1 factories = 3 }",
    );
    GameClock::advance(&interp, &mut world, 24 * 30); // 跑 30 天到中效率

    let eff_before = world.countries["GER"].production_lines[0].slots[0].efficiency;
    // 切到 infantry_equipment_2(同 chassis)
    run_script(
        &interp,
        &mut world,
        "change_line_variant = { owner = GER line_id = 1 variant = infantry_equipment_2 }",
    );
    assert!(
        world.error_log.is_empty(),
        "change_line_variant 不应报错: {:?}",
        world.error_log
    );

    let eff_after = world.countries["GER"].production_lines[0].slots[0].efficiency;
    assert!(
        (eff_after - eff_before * 0.9).abs() < 0.01,
        "应保留 90%: before={}, after={}",
        eff_before,
        eff_after
    );
}
