//! 端到端战斗测试(M3 核心验收): 脚本驱动两师打仗
use hoi4_clone::ast::lower::lower_effects;
use hoi4_clone::commands::register_all;
use hoi4_clone::parser::{parse, Value};
use hoi4_clone::runtime::{GameClock, Interpreter, Registry, World};

/// 从脚本块中取出名为 key 的子块
fn block_named<'a>(b: &'a hoi4_clone::parser::Block, key: &str) -> &'a hoi4_clone::parser::Block {
    let f = b.fields.iter().find(|f| f.key == key).unwrap_or_else(|| panic!("缺 {key}"));
    match &f.value {
        Value::Block(b) => b,
        _ => panic!("{key} 应为块"),
    }
}

fn setup_world() -> World {
    let mut w = World::new();
    w.player_tag = "GER".into();
    w.countries.insert("GER".into(), Default::default());
    w.countries.insert("FRA".into(), Default::default());
    w
}

fn run_setup(world: &mut World, interp: &Interpreter, src: &str) {
    let b = parse(src).unwrap();
    // 顶层用 _setup 包裹, 取其内层
    let setup = block_named(&b, "_setup");
    let effs = lower_effects(setup);
    interp.run(&effs, world);
}

#[test]
fn two_divisions_battle_deals_damage() {
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();

    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 100 defense = 40 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 80 defense = 40 max_org = 60 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    assert_eq!(world.divisions.len(), 2, "应创建 2 个师");
    assert_eq!(world.battles.len(), 1, "应有 1 场战斗");

    // 记录守方初始 org
    let fra_id = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap().id;
    let org_before = world.divisions.get(&fra_id).unwrap().org;
    assert!((org_before - 60.0).abs() < 1e-9, "初始 org 应为 60");

    // 推进 24 小时(战斗每小时结算)
    GameClock::advance(&interp, &mut world, 24);

    let org_after = world.divisions.get(&fra_id).unwrap().org;
    assert!(
        org_after < org_before,
        "24h 战斗后守方 org 应下降: before={org_before} after={org_after}"
    );
}

#[test]
fn broken_division_detected() {
    // 战斗到 org 归零, is_broken trigger 应为 true
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();

    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 500 defense = 40 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 10 defense = 5 max_org = 60 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);

    let fra_id = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap().id;
    // 推进足够长时间让守方破阵
    GameClock::advance(&interp, &mut world, 200);

    let fra = world.divisions.get(&fra_id).unwrap();
    assert!(fra.org <= 0.0, "高强度攻击下守方应破阵, org={}", fra.org);
    assert!(fra.is_broken(), "is_broken 应为 true");
}

#[test]
fn stalemate_no_damage_when_no_battle() {
    // 无战斗时, 师不应掉 org
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();

    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 100 max_org = 60 }
        }
    "#);
    let ger_id = world.divisions.values().next().unwrap().id;
    let org_before = world.divisions.get(&ger_id).unwrap().org;

    GameClock::advance(&interp, &mut world, 24);
    let org_after = world.divisions.get(&ger_id).unwrap().org;
    assert!(
        (org_after - org_before).abs() < 1e-9,
        "无战斗时 org 不应变化"
    );
}
