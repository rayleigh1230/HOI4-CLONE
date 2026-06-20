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

#[test]
fn counter_attack_damages_attacker() {
    // P0-2 验证: 战斗对称, 攻方也掉 org(反击)
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 100 defense = 40 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 100 defense = 40 max_org = 60 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    let ger_id = world.divisions.values().find(|d| d.owner_tag == "GER").unwrap().id;
    let ger_org_before = world.divisions.get(&ger_id).unwrap().org;

    GameClock::advance(&interp, &mut world, 24);
    let ger_org_after = world.divisions.get(&ger_id).unwrap().org;
    assert!(
        ger_org_after < ger_org_before,
        "P0-2: 攻方应受反击掉 org, before={ger_org_before} after={ger_org_after}"
    );
}

#[test]
fn exact_org_after_one_hour() {
    // P1-7 验证: 1 小时后守方 org = 精确预期值(锁定公式)
    // 配置: 攻方 soft_attack=200 hard=0, 守方 hardness=0 defense=0 max_org=60
    //   攻击点 = 200×(1-0) + 0 = 200, 单目标 share=100% → 200 攻击
    //   防御池 0 → 全 undefended: 命中 = 200×0.40 = 80
    //   无装甲碾压: org骰=4, 期望=(4+1)/2=2.5
    //   org伤害 = 80 × 2.5 × 0.053 = 10.6
    //   守方 org = 60 - 10.6 = 49.4
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 200 hard_attack = 0 armor = 0 piercing = 0 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 0 hardness = 0 armor = 0 piercing = 0 max_org = 60 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    let fra_id = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap().id;
    GameClock::advance(&interp, &mut world, 1);
    let fra_org = world.divisions.get(&fra_id).unwrap().org;
    // 容忍小数误差(反击守方 soft_attack=0 不造成伤害, 故纯正向)
    assert!(
        (fra_org - 49.4).abs() < 0.01,
        "1h 后守方 org 应为 49.4, 实际 {fra_org}"
    );
}

#[test]
fn equipment_degrades_in_combat_and_reinforces() {
    // M4a 端到端: 战斗扣装备 → 装备充足度下降 → 增援从仓库补回
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            add_equipment = { owner = GER type = inf amount = 50 }
            create_division = { owner = GER location = 1 soft_attack = 200 defense = 5 max_org = 60 max_strength = 20 equipment = inf equipment_amount = 100 }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 0 max_org = 60 max_strength = 20 equipment = inf equipment_amount = 100 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    let ger_id = world.divisions.values().find(|d| d.owner_tag == "GER").unwrap().id;
    let fra_id = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap().id;

    // 战斗前: FRA 装备满(100/100)
    let fra_eq_before = world.divisions.get(&fra_id).unwrap().equipment_ratio();
    assert!((fra_eq_before - 1.0).abs() < 1e-9, "战斗前 FRA 装备应满");

    // 打 12 小时(不到一天, 不触发增援)
    GameClock::advance(&interp, &mut world, 12);
    let fra_eq_mid = world.divisions.get(&fra_id).unwrap().equipment_ratio();
    assert!(
        fra_eq_mid < fra_eq_before,
        "战斗应消耗 FRA 装备: before={fra_eq_before} mid={fra_eq_mid}"
    );

    // 推进到 24h+ 触发每日增援(GER 仓库有 50 件 inf, 但 GER 是攻方不补; FRA 仓库空)
    // 给 FRA 也加库存以便验证增援
    world.countries.get_mut("FRA").unwrap().equipment_stockpile.insert("inf".into(), 30.0);
    let fra_eq_before_reinforce = world.divisions.get(&fra_id).unwrap().equipment_ratio();
    GameClock::advance(&interp, &mut world, 24); // 触发一次 daily reinforce
    let fra_eq_after_reinforce = world.divisions.get(&fra_id).unwrap().equipment_ratio();
    assert!(
        fra_eq_after_reinforce >= fra_eq_before_reinforce,
        "增援应补充装备: before={fra_eq_before_reinforce} after={fra_eq_after_reinforce}"
    );

    let _ = ger_id;
}

#[test]
fn broken_division_removed_from_battle() {
    // P2-14: 破阵师从战斗移除, 一方全破则战斗结束
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 500 defense = 10 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 5 defense = 5 max_org = 30 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    assert_eq!(world.battles.len(), 1, "开战应有1场战斗");
    let fra_id = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap().id;

    // 高强度攻击, FRA 很快破阵
    GameClock::advance(&interp, &mut world, 50);

    // FRA 应已破阵
    assert!(world.divisions.get(&fra_id).unwrap().is_broken(), "FRA 应破阵");
    // 战斗应已结束(FRA 全破)
    assert_eq!(world.battles.len(), 0, "守方全破后战斗应结束");
}

#[test]
fn battle_continues_while_both_sides_alive() {
    // 双方都活着时战斗不结束
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 30 defense = 100 max_org = 100 }
            create_division = { owner = FRA location = 1 soft_attack = 30 defense = 100 max_org = 100 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    GameClock::advance(&interp, &mut world, 24);
    // 低强度, 双方都应存活, 战斗继续
    assert_eq!(world.battles.len(), 1, "双方存活战斗应继续");
    let any_broken = world.divisions.values().any(|d| d.is_broken());
    assert!(!any_broken, "低强度战斗24h内不应有师破阵");
}
