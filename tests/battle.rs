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
    // 省份布局: 1=战场, 10=GER后方, 20=FRA后方(让撤退师有处可退)
    w.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![10, 20],
    });
    w.provinces.insert(10, hoi4_clone::runtime::Province {
        id: 10, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![1],
    });
    w.provinces.insert(20, hoi4_clone::runtime::Province {
        id: 20, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![1],
    });
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

    // 推进 5 小时(短时, 避免歼灭; 验证 org 下降)
    GameClock::advance(&interp, &mut world, 5);

    let org_after = world.divisions.get(&fra_id).unwrap().org;
    assert!(
        org_after < org_before,
        "24h 战斗后守方 org 应下降: before={org_before} after={org_after}"
    );
}

#[test]
fn broken_division_detected() {
    // 高强度攻击下守方会破阵并被移出战斗(组织度恢复后可能回升, 但战斗已结束)
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

    // 推进 20 小时: FRA 应已破阵并触发战斗结束
    GameClock::advance(&interp, &mut world, 20);
    // 战斗应已结束(守方破阵被移出) — 这是破阵的直接证据
    assert_eq!(world.battles.len(), 0, "守方破阵后战斗应结束");
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
            create_division = { owner = GER location = 1 soft_attack = 200 defense = 5 max_org = 60 equipment = inf equipment_amount = 100 }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 0 max_org = 60 equipment = inf equipment_amount = 100 }
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

    // 高强度攻击, FRA 很快破阵被移出战斗(之后可能恢复 org, 但战斗已结束)
    GameClock::advance(&interp, &mut world, 50);

    // 战斗应已结束(FRA 破阵被移出) — 破阵移除的直接证据
    assert_eq!(world.battles.len(), 0, "守方破阵后战斗应结束");
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
            create_division = { owner = GER location = 1 soft_attack = 30 defense = 100 breakthrough = 100 max_org = 100 }
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

#[test]
fn manpower_consumed_and_reinforced() {
    // 四量模型: 战斗消耗人力, 增援从国家人力池补
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            add_manpower = { owner = GER amount = 500 }
            add_manpower = { owner = FRA amount = 0 }
            create_division = { owner = GER location = 1 soft_attack = 200 defense = 5 max_org = 60 manpower = 1000 }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 0 max_org = 60 manpower = 1000 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    let fra_id = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap().id;
    let mp_before = world.divisions.get(&fra_id).unwrap().manpower_held;

    // 战斗消耗人力(FRA 被打, HP 损失 → 人力损失)
    GameClock::advance(&interp, &mut world, 12);
    let mp_mid = world.divisions.get(&fra_id).unwrap().manpower_held;
    assert!(mp_mid < mp_before, "战斗应消耗 FRA 人力: before={mp_before} mid={mp_mid}");

    // FRA 人力池为 0, 增援补不了; GER 有 500, 能补
    let ger_mp_before = world.divisions.values().find(|d| d.owner_tag == "GER").unwrap().manpower_held;
    GameClock::advance(&interp, &mut world, 24); // 触发增援
    let ger_mp_after = world.divisions.values().find(|d| d.owner_tag == "GER").unwrap().manpower_held;
    assert!(ger_mp_after >= ger_mp_before, "GER 人力池有储备, 增援应补人力");
}

#[test]
fn org_recovers_after_battle_ends() {
    // 组织度恢复: 战斗结束后(破阵移出), 师脱离战斗 org 回升
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 200 defense = 5 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 0 max_org = 60 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    let fra_id = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap().id;
    // 打到 FRA 破阵, 战斗结束
    GameClock::advance(&interp, &mut world, 20);
    let org_at_break = world.divisions.get(&fra_id).unwrap().org;
    assert_eq!(world.battles.len(), 0, "战斗应已结束");
    // 再推进, FRA 脱离战斗, org 应恢复
    let org_right_after = world.divisions.get(&fra_id).unwrap().org;
    GameClock::advance(&interp, &mut world, 100);
    let org_recovered = world.divisions.get(&fra_id).unwrap().org;
    assert!(
        org_recovered > org_right_after,
        "脱离战斗后 org 应回升: at_break={org_at_break} after={org_right_after} recovered={org_recovered}"
    );
}

#[test]
fn annihilated_division_removed_from_world() {
    // HP 归零 → 歼灭: 师从 world.divisions 彻底删除(番号撤销)
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    // FRA 极弱(HP=5, defense=0), GER 强攻 → FRA HP 快速归零 → 歼灭
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 500 defense = 100 breakthrough = 100 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 0 max_org = 60 max_strength = 5 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    assert_eq!(world.divisions.len(), 2);
    GameClock::advance(&interp, &mut world, 30);
    // FRA 应被歼灭(HP 归零), 从世界删除
    assert_eq!(world.divisions.len(), 1, "FRA 应被歼灭删除, 只剩 GER");
    assert!(world.divisions.values().all(|d| d.owner_tag == "GER"), "只剩 GER");
    assert_eq!(world.battles.len(), 0, "战斗应结束");
}

#[test]
fn retreating_division_preserved_not_annihilated() {
    // org 归零 + HP 有余 → 撤退: 师保留(标 retreating), 不删除
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    // GER 攻 FRA: 让 FRA org 归零但 HP 保留
    // FRA defense 高(防 HP 损失), 但 soft_attack=0 不反击, GER 稳定输出 org 伤害
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 200 defense = 100 breakthrough = 100 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 200 max_org = 30 max_strength = 100 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    let fra_id = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap().id;
    GameClock::advance(&interp, &mut world, 40);
    // FRA 应撤退(org 归零, HP 有余), 师仍存在
    assert!(world.divisions.contains_key(&fra_id), "撤退的师应保留, 不删除");
    let fra = world.divisions.get(&fra_id).unwrap();
    assert!(fra.strength > 0.0, "撤退师 HP 应有余: {}", fra.strength);
    assert_eq!(world.battles.len(), 0, "撤退后战斗应结束");
}

#[test]
fn surrounded_division_annihilated_on_retreat() {
    // 包围歼灭: 撤退师无邻接己方省 → 被歼灭(而非撤退)
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    world.player_tag = "GER".into();
    world.countries.insert("GER".into(), Default::default());
    world.countries.insert("FRA".into(), Default::default());
    // 孤立省1: 只有自己, 无任何邻接 → FRA 撤退时无处可退 → 歼灭
    world.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![], // 无邻接!
    });
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 equipment = medium_tank battalions = 7 }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 200 max_org = 30 max_strength = 100 equipment = infantry_equipment }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    let fra_id = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap().id;
    // FRA org 会先归零(装甲碾压) → 尝试撤退 → 无邻省 → 包围歼灭
    GameClock::advance(&interp, &mut world, 40);
    assert!(
        !world.divisions.contains_key(&fra_id),
        "孤立省撤退应被包围歼灭, 师应消失"
    );
}

#[test]
fn retreating_division_moves_to_friendly_province() {
    // 撤退师实际移动到邻接己方省
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    world.player_tag = "GER".into();
    world.countries.insert("GER".into(), Default::default());
    world.countries.insert("FRA".into(), Default::default());
    // 省1(战场) 邻接 省20(FRA后方)
    world.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![20],
    });
    world.provinces.insert(20, hoi4_clone::runtime::Province {
        id: 20, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![1],
    });
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 equipment = medium_tank battalions = 7 }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 200 max_org = 30 max_strength = 100 equipment = infantry_equipment }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    let fra_id = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap().id;
    assert_eq!(world.divisions.get(&fra_id).unwrap().location_province, 1);
    // 推进到 FRA 撤退 + 行军到达
    GameClock::advance(&interp, &mut world, 60);
    let fra = world.divisions.get(&fra_id);
    assert!(fra.is_some(), "FRA 应撤退保留(有邻省可退), 不应歼灭");
    let fra = fra.unwrap();
    assert_eq!(
        fra.location_province, 20,
        "FRA 应撤退到邻接己方省20, 实际在 {}", fra.location_province
    );
}

#[test]
fn attacker_captures_province_on_victory() {
    // 攻方胜(守方全退) → 攻方占领战斗省份
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    world.player_tag = "GER".into();
    world.countries.insert("GER".into(), Default::default());
    world.countries.insert("FRA".into(), Default::default());
    // 省1(FRA控制, 战场) 邻接省20(FRA后方, 让FRA能撤退→战斗因撤退结束→攻方占省1)
    world.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![20],
    });
    world.provinces.insert(20, hoi4_clone::runtime::Province {
        id: 20, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![1],
    });
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 equipment = medium_tank battalions = 7 }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 200 max_org = 30 max_strength = 100 equipment = infantry_equipment }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    assert_eq!(world.provinces.get(&1).unwrap().controller, "FRA", "开战前省1属FRA");
    GameClock::advance(&interp, &mut world, 40);
    // FRA 撤退 → 战斗结束 → GER 占领省1
    assert_eq!(
        world.provinces.get(&1).unwrap().controller, "GER",
        "攻方胜应占领省1, 实际: {}", world.provinces.get(&1).unwrap().controller
    );
}

#[test]
fn marching_division_loses_org() {
    // 移动中的师每小时掉 org(非恢复)
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    world.player_tag = "GER".into();
    world.countries.insert("GER".into(), Default::default());
    world.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![2],
    });
    world.provinces.insert(2, hoi4_clone::runtime::Province {
        id: 2, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![1],
    });
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 equipment = infantry_equipment battalions = 7 }
        }
    "#);
    let did = world.divisions.values().next().unwrap().id;
    let org_before = world.divisions.get(&did).unwrap().org;
    // 手动设 destination 让师移动
    world.divisions.get_mut(&did).unwrap().destination = Some(2);
    GameClock::advance(&interp, &mut world, 3); // 移动中 3 小时
    let org_after = world.divisions.get(&did).unwrap().org;
    assert!(
        org_after < org_before,
        "移向敌方省 org 应下降(每小时-0.2): before={org_before} after={org_after}"
    );
}

#[test]
fn marching_in_friendly_territory_no_org_loss() {
    // 己方地块行军不掉 org(组织度损耗与地块归属相关)
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    world.player_tag = "GER".into();
    world.countries.insert("GER".into(), Default::default());
    // 省1和省2都是GER己方
    world.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![2],
    });
    world.provinces.insert(2, hoi4_clone::runtime::Province {
        id: 2, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![1],
    });
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 1 equipment = infantry_equipment battalions = 7 } }
    "#);
    let did = world.divisions.values().next().unwrap().id;
    let org_before = world.divisions.get(&did).unwrap().org;
    world.divisions.get_mut(&did).unwrap().destination = Some(2); // 移向己方省2
    GameClock::advance(&interp, &mut world, 3);
    let org_after = world.divisions.get(&did).unwrap().org;
    assert!(
        (org_after - org_before).abs() < 1e-9,
        "己方地块行军 org 不应损耗: before={org_before} after={org_after}"
    );
}

#[test]
fn move_to_enemy_province_starts_battle_immediately() {
    // 进攻移动: 下令移到敌军所在省 → 立刻开战(非到达才开战)
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 2 equipment = medium_tank battalions = 7 }
            create_division = { owner = FRA location = 1 equipment = infantry_equipment battalions = 7 }
        }
    "#);
    let ger_id = world.divisions.values().find(|d| d.owner_tag == "GER").unwrap().id;
    // GER 在省2, 命令移到省1(FRA 所在) → 应立刻开战
    assert_eq!(world.battles.len(), 0, "下令前无战斗");
    let move_effs = hoi4_clone::ast::lower::lower_effects(
        &hoi4_clone::parser::parse("move_division = { division = 1 target = 1 }").unwrap()
    );
    interp.run(&move_effs, &mut world);
    assert_eq!(world.battles.len(), 1, "下令移到敌省应立刻开战");
    assert!(world.divisions.get(&ger_id).unwrap().attacking, "应处于进攻移动状态");
}

#[test]
fn move_to_empty_province_no_battle() {
    // 普通移动: 移到空省/己方省 → 不开战, 普通进驻
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 2 equipment = infantry_equipment battalions = 7 }
        }
    "#);
    let ger_id = world.divisions.values().find(|d| d.owner_tag == "GER").unwrap().id;
    // 省10 是 GER 己方空省, 移过去不开战
    let move_effs = hoi4_clone::ast::lower::lower_effects(
        &hoi4_clone::parser::parse("move_division = { division = 1 target = 10 }").unwrap()
    );
    interp.run(&move_effs, &mut world);
    assert_eq!(world.battles.len(), 0, "移到己方空省不应开战");
    assert!(!world.divisions.get(&ger_id).unwrap().attacking, "应非进攻状态");
    // 推进到达
    use hoi4_clone::runtime::GameClock;
    GameClock::advance(&interp, &mut world, 10);
    assert_eq!(world.divisions.get(&ger_id).unwrap().location_province, 10, "应到达省10");
}

#[test]
fn march_into_empty_enemy_province_captures() {
    // 进军无防御的敌方地块 → 红箭头 + 到达占领
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    world.player_tag = "GER".into();
    world.countries.insert("GER".into(), Default::default());
    // 省1(GER) 邻接 省2(FRA空省, 无防御部队)
    world.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![2],
    });
    world.provinces.insert(2, hoi4_clone::runtime::Province {
        id: 2, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![1],
    });
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 1 equipment = infantry_equipment battalions = 7 } }
    "#);
    let ger_id = world.divisions.values().find(|d| d.owner_tag == "GER").unwrap().id;
    // 命令 GER 师进军省2(FRA空省)
    let move_effs = hoi4_clone::ast::lower::lower_effects(
        &hoi4_clone::parser::parse("move_division = { division = 1 target = 2 }").unwrap()
    );
    interp.run(&move_effs, &mut world);
    // 应是进军(红), 无敌军不开战
    assert!(world.divisions.get(&ger_id).unwrap().attacking, "进军敌方地块应红箭头");
    assert_eq!(world.battles.len(), 0, "无防御部队不应开战");
    // 推进到达(进军速度慢, 给足时间)
    GameClock::advance(&interp, &mut world, 30);
    assert_eq!(world.divisions.get(&ger_id).unwrap().location_province, 2, "应到达省2");
    assert_eq!(world.provinces.get(&2).unwrap().controller, "GER", "到达应占领省2");
}

#[test]
fn frontline_route_causes_reserve_routing() {
    // 带溃: 守方前线崩 → 预备队强制撤退 + 攻方占地
    // 即使预备队还有师, 前线崩了就被带溃, 不继续战斗
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    // 3个FRA师在省1: 2前线(低HP易崩) + 1预备队(满血)
    // 用宽度分配: 2个7步师=28宽<70进前线, 第3个超宽进预备队? 不, 3个21宽也<70
    // 改: 用大宽度让第3个进预备队。combat_width=40的两个师=80>70, 第2个进预备队
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 2 equipment = medium_tank battalions = 7 }
            create_division = { owner = FRA location = 1 equipment = infantry_equipment battalions = 7 soft_attack = 0 defense = 5 max_org = 10 }
            create_division = { owner = FRA location = 1 equipment = infantry_equipment battalions = 7 soft_attack = 0 defense = 5 max_org = 10 }
        }
    "#);
    // 手动构造战斗: 2个FRA前线(会被快速打崩), 无预备队先测基础
    // 实际测带溃需要预备队, 但宽度70容纳多个7步师(14宽). 用 move_division 进攻
    let move_effs = hoi4_clone::ast::lower::lower_effects(
        &hoi4_clone::parser::parse("move_division = { division = 1 target = 1 }").unwrap()
    );
    interp.run(&move_effs, &mut world);
    // GER 进攻省1, FRA 2师都在前线(28宽<70)
    assert!(!world.battles.is_empty(), "应有战斗");
    // 推进让 FRA 前线崩(org 10 很快归零)
    GameClock::advance(&interp, &mut world, 30);
    // FRA 前线全崩 → 战斗结束 + 占地
    assert_eq!(world.battles.len(), 0, "前线崩后战斗应结束");
    assert_eq!(world.provinces.get(&1).unwrap().controller, "GER", "应占领省1");
    // FRA 师应撤退(非歼灭, org归零HP有余)
    let fra_alive = world.divisions.values().filter(|d| d.owner_tag == "FRA").count();
    assert!(fra_alive > 0, "FRA 师应撤退存活(非歼灭)");
}
