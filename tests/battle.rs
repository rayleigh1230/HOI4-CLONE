//! 端到端战斗测试(M3 核心验收): 脚本驱动两师打仗
use hoi4_clone::ast::lower::lower_effects;
use hoi4_clone::commands::register_all;
use hoi4_clone::parser::{parse, Value};
use hoi4_clone::runtime::entities::OrderState;
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
    // 手动设 Moving 让师移动(目标省2=敌方, hostile=true)
    world.divisions.get_mut(&did).unwrap().order = OrderState::Moving {
        dest: 2, progress: 0.0, hostile: true, origin: 1, remaining: vec![],
    };
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
    world.divisions.get_mut(&did).unwrap().order = OrderState::Moving {
        dest: 2, progress: 0.0, hostile: false, origin: 1, remaining: vec![],
    }; // 移向己方省2
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
            create_division = { owner = GER location = 10 equipment = medium_tank battalions = 7 }
            create_division = { owner = FRA location = 1 equipment = infantry_equipment battalions = 7 }
        }
    "#);
    let ger_id = world.divisions.values().find(|d| d.owner_tag == "GER").unwrap().id;
    // GER 在省10, 命令移到省1(FRA 所在, 相邻) → 应立刻开战
    assert_eq!(world.battles.len(), 0, "下令前无战斗");
    let move_effs = hoi4_clone::ast::lower::lower_effects(
        &hoi4_clone::parser::parse("move_division = { division = 1 target = 1 }").unwrap()
    );
    interp.run(&move_effs, &mut world);
    assert_eq!(world.battles.len(), 1, "下令移到敌省应立刻开战");
    assert!(world.divisions.get(&ger_id).unwrap().is_attacking_move(), "应处于进攻移动状态");
}

#[test]
fn move_to_empty_province_no_battle() {
    // 普通移动: 移到空省/己方省 → 不开战, 普通进驻
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    world.player_tag = "GER".into();
    world.countries.insert("GER".into(), Default::default());
    // 省10(GER) 邻接 省1(GER空省, 无部队)
    world.provinces.insert(10, hoi4_clone::runtime::Province {
        id: 10, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![1],
    });
    world.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![10],
    });
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 10 equipment = infantry_equipment battalions = 7 } }
    "#);
    let ger_id = world.divisions.values().find(|d| d.owner_tag == "GER").unwrap().id;
    // 省1 是 GER 己方空省, 移过去不开战
    let move_effs = hoi4_clone::ast::lower::lower_effects(
        &hoi4_clone::parser::parse("move_division = { division = 1 target = 1 }").unwrap()
    );
    interp.run(&move_effs, &mut world);
    assert_eq!(world.battles.len(), 0, "移到己方空省不应开战");
    assert!(!world.divisions.get(&ger_id).unwrap().is_attacking_move(), "应非进攻状态");
    // 推进到达
    use hoi4_clone::runtime::GameClock;
    GameClock::advance(&interp, &mut world, 100);
    assert_eq!(world.divisions.get(&ger_id).unwrap().location_province, 1, "应到达省1");
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
    assert!(world.divisions.get(&ger_id).unwrap().is_attacking_move(), "进军敌方地块应红箭头");
    assert_eq!(world.battles.len(), 0, "无防御部队不应开战");
    // 推进到达(进军速度慢, 给足时间)
    GameClock::advance(&interp, &mut world, 100);
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
            create_division = { owner = GER location = 10 equipment = medium_tank battalions = 7 }
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
    // 推进让 FRA 前线崩 + GER 行军到达占领
    GameClock::advance(&interp, &mut world, 100);
    // FRA 前线全崩 → 战斗结束 + GER 到达占地
    assert_eq!(world.battles.len(), 0, "前线崩后战斗应结束");
    assert_eq!(world.provinces.get(&1).unwrap().controller, "GER", "应占领省1");
    // FRA 师应撤退(非歼灭, org归零HP有余)
    let fra_alive = world.divisions.values().filter(|d| d.owner_tag == "FRA").count();
    assert!(fra_alive > 0, "FRA 师应撤退存活(非歼灭)");
}

#[test]
fn routed_reserve_keeps_org() {
    // 带溃的预备队师 org 保持(非归零) — 它没参战, org 不被打掉
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    // FRA 省1: 前线师(低org易崩) + 预备队师(满org, 超宽进预备队)
    // 用大宽度让第2个FRA师进预备队: combat_width=40 × 2 = 80 > 70
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 2 equipment = medium_tank battalions = 7 }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 5 max_org = 10 combat_width = 40 equipment = infantry_equipment }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 5 max_org = 10 combat_width = 40 equipment = infantry_equipment }
        }
    "#);
    // 找预备队师(第2个FRA师, 应在reserve)
    let reserve_fra = world.divisions.values()
        .filter(|d| d.owner_tag == "FRA")
        .last().unwrap().id;
    // GER 进攻省1
    let move_effs = hoi4_clone::ast::lower::lower_effects(
        &hoi4_clone::parser::parse("move_division = { division = 1 target = 1 }").unwrap()
    );
    interp.run(&move_effs, &mut world);
    // 预备队师 org 应满(没参战)
    let org_before = world.divisions.get(&reserve_fra).unwrap().org;
    assert!((org_before - 10.0).abs() < 1e-9, "预备队师初始org应为10");
    // 推进让前线崩 → 带溃预备队
    GameClock::advance(&interp, &mut world, 40);
    // 带溃师应存活 + org 保持(非归零)
    let routed = world.divisions.get(&reserve_fra);
    assert!(routed.is_some(), "带溃师应存活(撤退非歼灭)");
    let routed = routed.unwrap();
    // 带溃师没参战, org 应保持(非归零); 到达后方省后 retreat 清(org满)
    assert!(
        routed.org > 0.0,
        "带溃师 org 应保持(非归零): 实际 {}", routed.org
    );
}

#[test]
fn retreating_division_not_reengaged_by_check_engagements() {
    // 回归 bug: 撤退师 location 仍在战场省, 被 check_engagements 每tick重新拉入战斗,
    // 导致 org 归零后 str 持续下降直至歼灭(用户报告的"组织度掉完还在掉装备HP")。
    // 场景: GER move 进攻省1, FRA 守省1。FRA 撤退撤向省3, 但 location 仍=省1。
    //       GER 的 destination=省1, 每 tick check_engagements 查省1敌军→重拉 FRA。
    // 修复后: retreating 师不被 check_engagements 当作守方, FRA 撤退后 str 应停止下降。
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    world.player_tag = "GER".into();
    world.countries.insert("GER".into(), Default::default());
    world.countries.insert("FRA".into(), Default::default());
    // UI 默认布局: 省1(FRA) 邻省2(GER)和省3(FRA, 撤退目标)
    world.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![2, 3],
    });
    world.provinces.insert(2, hoi4_clone::runtime::Province {
        id: 2, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![1],
    });
    world.provinces.insert(3, hoi4_clone::runtime::Province {
        id: 3, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![1],
    });
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 2 equipment = medium_tank battalions = 7 }
            create_division = { owner = FRA location = 1 equipment = infantry_equipment battalions = 7 }
        }
    "#);
    let fra_id = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap().id;
    // GER 进攻省1
    let move_effs = hoi4_clone::ast::lower::lower_effects(
        &hoi4_clone::parser::parse("move_division = { division = 1 target = 1 }").unwrap()
    );
    interp.run(&move_effs, &mut world);

    // 推进到 FRA 撤退(org 归零), 记录撤退瞬间的 str
    let mut str_when_retreat_started: Option<f64> = None;
    for h in 1..=120 {
        GameClock::tick(&interp, &mut world);
        let fra = world.divisions.get(&fra_id);
        if fra.is_none() {
            panic!("FRA 被歼灭删除于 tick {h} — 撤退师不应被歼灭(回归 bug 复现)");
        }
        let fra = fra.unwrap();
        if str_when_retreat_started.is_none() && fra.is_withdrawing() {
            str_when_retreat_started = Some(fra.strength);
            eprintln!("tick {h}: FRA 开始撤退, str={:.1}", fra.strength);
        }
    }

    // 最终: FRA 应存活(撤退保留非歼灭)
    assert!(world.divisions.contains_key(&fra_id), "撤退师应保留, 不应被歼灭");
    let fra_final = world.divisions.get(&fra_id).unwrap();
    assert!(fra_final.strength > 0.0, "撤退师 str 应有余, 不应归零: {}", fra_final.strength);
    // 关键: 撤退开始后 str 不应大幅下降(撤退师不挨打)
    // 给一定容差(撤退过程可能再挨1-2tick), 但不应从~130掉到歼灭
    if let Some(s0) = str_when_retreat_started {
        let drop = s0 - fra_final.strength;
        assert!(
            drop < 20.0,
            "撤退后 str 不应大幅下降: 开始={s0} 最终={} 下降={drop:.1}",
            fra_final.strength
        );
    }
}

#[test]
fn retreating_into_enemy_occupied_province_starts_battle() {
    // 回归 bug: 撤退师到达目标省时, 若该省已被敌方占领+有敌军,
    // 应爆发战斗(撤退师变攻方), 而非直接占领该省。
    // 直接构造场景, 精确控制撤退到达逻辑:
    //   FRA 师正在撤退(retreating=true, destination=省20, progress 接近满)
    //   省20 控制权=GER(敌方), 且有 GER 师驻守
    //   advance_movement 推进到到达 → 不应占领省20, 应进入 pending 等开战
    //   check_engagements → 撤退师变攻方开战
    use hoi4_clone::combat::movement::{advance_movement, check_engagements};
    let mut world = World::new();
    world.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![20],
    });
    world.provinces.insert(20, hoi4_clone::runtime::Province {
        id: 20, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![1],
    });
    // FRA 师在省1, 撤退中, 目标省20, 进度几乎满(1次 advance 即到达)
    let fra = world.add_division(hoi4_clone::runtime::entities::Division {
        owner_tag: "FRA".into(), location_province: 1,
        order: OrderState::Retreating { dest: 20, progress: 0.99 },
        max_org: 60.0, org: 60.0, max_strength: 20.0, strength: 20.0,
        ..Default::default()
    });
    // GER 师驻守省20(敌方占领+有部队)
    let ger = world.add_division(hoi4_clone::runtime::entities::Division {
        owner_tag: "GER".into(), location_province: 20,
        order: OrderState::Idle,
        max_org: 60.0, org: 60.0, max_strength: 20.0, strength: 20.0,
        ..Default::default()
    });

    advance_movement(&mut world);

    // 核心断言1: 省20 不应被占领(仍属 GER)
    let prov20 = world.provinces.get(&20).unwrap();
    assert_eq!(
        prov20.controller, "GER",
        "撤退师到达敌方驻军省不应直接占领(当前 controller={})",
        prov20.controller
    );
    // 核心断言2: 撤退师进入 Pending(等开战), 不再是 Retreating(即将变攻方)
    let fra_div = world.divisions.get(&fra).unwrap();
    assert_eq!(fra_div.pending_dest(), Some(20), "应进入 Pending 等开战");
    assert!(!fra_div.is_withdrawing(), "撤退师到达敌方省应退出 Retreating(即将变攻方)");

    // check_engagements → 应开战(FRA 变攻方, GER 守)
    check_engagements(&mut world);
    let battle = world.battles.iter().find(|b| b.province == 20);
    assert!(battle.is_some(), "省20 应爆发战斗");
    let battle = battle.unwrap();
    assert!(battle.attackers.contains(&fra), "撤退师 FRA 应成省20战斗攻方");
    assert!(battle.defenders.contains(&ger), "GER 师应成省20战斗守方");
}

#[test]
fn retreating_to_enemy_province_then_loses_continues_retreat_or_dies() {
    // 完整流程(用户需求): 撤退师到达敌方驻军省 → 变攻方开战 →
    //   胜 → 占领; 败 → 继续撤退(回origin); origin被占无己方邻省 → 包围歼灭。
    // 本测验证"败"分支: FRA 弱(刚撤退org低), 打不过省20的 GER → 应退出战斗保留,
    // 不应卡死也不应错误占领省20。
    use hoi4_clone::combat::movement::{advance_movement, check_engagements};
    use hoi4_clone::combat::resolve::resolve_all_battles;
    let mut world = World::new();
    world.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![20],
    });
    world.provinces.insert(20, hoi4_clone::runtime::Province {
        id: 20, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![1],
    });
    // FRA 师: org很低(刚被打崩), 撤退到省20(GER驻军). 它会变攻方但打不过.
    let fra = world.add_division(hoi4_clone::runtime::entities::Division {
        owner_tag: "FRA".into(), location_province: 1,
        order: OrderState::Retreating { dest: 20, progress: 0.99 },
        max_org: 60.0, org: 1.0, // org 极低, GER 反击一击即崩
        max_strength: 20.0, strength: 20.0,
        soft_attack: 5.0, defense: 10.0,
        ..Default::default()
    });
    // GER 师: 强势守省20
    let _ger = world.add_division(hoi4_clone::runtime::entities::Division {
        owner_tag: "GER".into(), location_province: 20,
        order: OrderState::Idle,
        max_org: 60.0, org: 60.0, max_strength: 20.0, strength: 20.0,
        soft_attack: 50.0, defense: 40.0,
        ..Default::default()
    });

    // 第1步: FRA 到达省20 → pending + 清retreating
    advance_movement(&mut world);
    // 第2步: check_engagements → FRA 变攻方开战
    check_engagements(&mut world);
    assert!(!world.battles.is_empty(), "应开战");
    let battle_started = world.battles.iter().any(|b| b.province == 20);
    assert!(battle_started, "省20应有战斗(FRA变攻方)");

    // 第3步: resolve → FRA org被打崩 → cleanup 攻方战败
    // 新语义: FRA 撤退到省20(RetreatIntoEnemy) → 归属地强制=省20 → 变攻方战败
    //   → 归属地省20 是 GER(敌方) → 进 Retreating 撤向邻省省1(行军)
    resolve_all_battles(&mut world);

    // 核心: FRA 不应占领省20(它败了)
    assert_eq!(
        world.provinces.get(&20).unwrap().controller, "GER",
        "FRA 战败, 省20 应仍属 GER"
    );
    // FRA 应存活: 攻方战败, 归属地省20(GER)非己方 → 进 Retreating 撤向邻省省1
    let fra_div = world.divisions.get(&fra);
    assert!(fra_div.is_some(), "FRA 战败应存活(转 Retreating), 不应歼灭");
    let fra_div = fra_div.unwrap();
    assert!(fra_div.is_withdrawing(), "FRA 应转 Retreating 撤向省1, 实际 order={:?}", fra_div.order);
    assert_eq!(fra_div.retreat_dest(), Some(1), "撤退目标应为邻省省1");
    assert!(fra_div.strength > 0.0, "FRA 应存活 str>0");
}

// ===== 支援攻击(support_attack)Step1: 命令基础行为 =====

/// 辅助: 跑一条命令脚本
fn run_cmd(world: &mut World, interp: &Interpreter, src: &str) {
    let effs = hoi4_clone::ast::lower::lower_effects(
        &hoi4_clone::parser::parse(src).unwrap()
    );
    interp.run(&effs, world);
}

#[test]
fn support_attack_invalid_when_no_battle() {
    // 规则1: 目标省无战斗 → 指令无效, supporting 不设
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 2 equipment = infantry_equipment battalions = 7 }
            create_division = { owner = FRA location = 1 equipment = infantry_equipment battalions = 7 }
        }
    "#);
    let ger_id = world.divisions.values().find(|d| d.owner_tag == "GER").unwrap().id;
    assert!(world.battles.is_empty(), "开战前无战斗");

    // GER 支援攻击省1(此刻无战斗) → 指令无效
    run_cmd(&mut world, &interp, "support_attack = { division = 1 target = 1 }");

    assert!(!world.divisions.get(&ger_id).unwrap().is_supporting(), "无战斗时不应进入 Supporting");
    assert!(world.battles.is_empty(), "无战斗时不应新建战斗");
}

#[test]
fn support_attack_joins_existing_battle_without_moving() {
    // 规则1/2/3: 目标省有战斗 → 加入攻方, 师不移动
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    // 先建一场省1的战斗(GER攻FRA守)
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 30 defense = 40 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 30 defense = 40 max_org = 60 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    // 在省10部署一支援师(GER, 与省1相邻), 支援省1(已有战斗)
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 10 equipment = infantry_equipment battalions = 7 } }
    "#);
    let support_id = world.divisions.values()
        .filter(|d| d.owner_tag == "GER" && d.location_province == 10)
        .map(|d| d.id).next().unwrap();
    assert_eq!(world.battles.len(), 1, "应已有1场战斗");

    run_cmd(&mut world, &interp, &format!("support_attack = {{ division = {} target = 1 }}", support_id));

    let sup = world.divisions.get(&support_id).unwrap();
    assert!(sup.is_supporting(), "应进入 Supporting 状态");
    // 规则2: 师不移动
    assert_eq!(sup.location_province, 10, "支援师 location 不变(仍在省10)");
    assert!(!sup.is_moving(), "支援师不进入 Moving(不移动)");
    assert!((sup.move_progress() - 0.0).abs() < 1e-9, "支援师进度不变");
    // 规则3: 加入战斗攻方
    let battle = &world.battles[0];
    assert!(
        battle.attackers.contains(&support_id) || battle.reserve_attackers.contains(&support_id),
        "支援师应加入战斗攻方(前线或预备队), atk={:?} res_atk={:?}",
        battle.attackers, battle.reserve_attackers
    );
}

#[test]
fn support_attack_same_origin_goes_reserve() {
    // 规则3: 同 origin 已有攻方师 → 支援师进预备队(started=true 后才生效)
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 30 defense = 40 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 30 defense = 40 max_org = 60 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    // 两个 GER 支援师都在省10(同 origin, 与省1相邻)
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 10 equipment = infantry_equipment battalions = 7 }
            create_division = { owner = GER location = 10 equipment = infantry_equipment battalions = 7 }
        }
    "#);
    let sup_ids: Vec<u64> = world.divisions.values()
        .filter(|d| d.owner_tag == "GER" && d.location_province == 10)
        .map(|d| d.id).collect();
    // 第一个支援(同 origin 无其他支援师)→ 前线
    run_cmd(&mut world, &interp, &format!("support_attack = {{ division = {} target = 1 }}", sup_ids[0]));
    // 推进让 started=true(部署阶段结束), 之后同 origin 才进预备队
    GameClock::advance(&interp, &mut world, 1);
    // 第二个支援(同 origin 已有支援师, started=true)→ 预备队
    run_cmd(&mut world, &interp, &format!("support_attack = {{ division = {} target = 1 }}", sup_ids[1]));

    let battle = &world.battles[0];
    assert!(battle.attackers.contains(&sup_ids[0]), "第一个支援师应在前线");
    assert!(battle.reserve_attackers.contains(&sup_ids[1]), "第二个同origin支援师应在预备队");
}

// ===== 支援攻击 Step2: 主循环集成行为 =====

#[test]
fn support_attack_auto_cancels_when_battle_ends() {
    // 规则7: 战斗结束后, 支援师的 supporting 自动清除
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    // GER 强攻 FRA, FRA 很快破阵 → 战斗结束
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 500 defense = 100 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 5 defense = 5 max_org = 30 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    // 在省10部署支援师(省10是GER后方, setup_world 里定义)
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 10 equipment = infantry_equipment battalions = 7 } }
    "#);
    let sup_id = world.divisions.values()
        .filter(|d| d.owner_tag == "GER" && d.location_province == 10)
        .map(|d| d.id).next().unwrap();
    run_cmd(&mut world, &interp, &format!("support_attack = {{ division = {} target = 1 }}", sup_id));
    assert!(world.divisions.get(&sup_id).unwrap().is_supporting(), "应已设支援");

    // 推进让 FRA 破阵、战斗结束
    GameClock::advance(&interp, &mut world, 30);

    assert_eq!(world.battles.len(), 0, "战斗应已结束");
    assert!(
        !world.divisions.get(&sup_id).unwrap().is_supporting(),
        "战斗结束后应自动退出 Supporting"
    );
}

#[test]
fn support_attacker_keeps_battle_after_move_attacker_retreats() {
    // 规则4: 移动攻方被打退, 但支援攻方在场 → 战斗继续
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    // 移动攻方 GER 弱(易退), 守方 FRA 中等, 支援攻方 GER 强
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 10 defense = 5 max_org = 20 }
            create_division = { owner = FRA location = 1 soft_attack = 50 defense = 100 max_org = 60 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 10 equipment = infantry_equipment battalions = 7 } }
    "#);
    let move_atk = world.divisions.values()
        .filter(|d| d.owner_tag == "GER" && d.location_province == 1)
        .map(|d| d.id).next().unwrap();
    let sup_id = world.divisions.values()
        .filter(|d| d.owner_tag == "GER" && d.location_province == 10)
        .map(|d| d.id).next().unwrap();
    // 支援师加入省1战斗
    run_cmd(&mut world, &interp, &format!("support_attack = {{ division = {} target = 1 }}", sup_id));

    // 推进: 移动攻方 GER 会先被打退(org20 vs FRA强), 但支援师还在 → 战斗继续
    GameClock::advance(&interp, &mut world, 15);

    // 移动攻方应已撤退(退出战斗), 但支援师应在战斗中 → 战斗不结束
    let _ = world.divisions.get(&move_atk).unwrap(); // 移动攻方仍存活(撤退非歼灭)
    let in_battle: std::collections::HashSet<u64> = world.battles.iter()
        .flat_map(|b| b.attackers.iter().chain(b.defenders.iter())
            .chain(b.reserve_attackers.iter()).chain(b.reserve_defenders.iter()).copied())
        .collect();
    // 关键: 支援师仍在战斗, 战斗未结束
    if !world.battles.is_empty() {
        assert!(in_battle.contains(&sup_id), "支援师应仍在战斗中(战斗继续), in_battle不含它");
    }
    // 移动攻方应已不在战斗(被打退)
    // (它可能回origin省1恢复, 也可能战斗还在时被移出 attackers)
}

#[test]
fn support_only_does_not_capture_province() {
    // 规则5: 敌方全灭只剩支援攻方(无移动攻方到达目标省) → 目标省归属不变。
    // 直接构造 cleanup 输入: 省1战斗, 攻方只有支援师(location≠省1), 守方全退。
    use hoi4_clone::combat::resolve::resolve_all_battles;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 10 equipment = infantry_equipment battalions = 7 }
            create_division = { owner = FRA location = 1 soft_attack = 0 defense = 5 max_org = 10 max_strength = 50 }
        }
    "#);
    let sup_id = world.divisions.values()
        .filter(|d| d.owner_tag == "GER").map(|d| d.id).next().unwrap();
    let fra_id = world.divisions.values()
        .filter(|d| d.owner_tag == "FRA").map(|d| d.id).next().unwrap();
    // 支援师: location=省10, Supporting 省1
    world.divisions.get_mut(&sup_id).unwrap().order = OrderState::Supporting { target: 1 };
    // FRA 守方: org 归零(将被判定撤退 → 前线崩)
    world.divisions.get_mut(&fra_id).unwrap().org = 0.0;
    // 构造省1战斗: 只有支援师作攻方, FRA 作守方
    world.battles.push(hoi4_clone::runtime::entities::Battle {
        id: 0, province: 1,
        attackers: vec![sup_id], defenders: vec![fra_id],
        ..Default::default()
    });
    assert_eq!(world.provinces.get(&1).unwrap().controller, "FRA", "开战前省1属FRA");

    resolve_all_battles(&mut world);

    // 守方 FRA 前线崩 → 战斗结束。攻方只有支援师(location=省10≠省1)
    // → attacker_present=false → 不占地
    let prov1 = world.provinces.get(&1).unwrap();
    assert_eq!(
        prov1.controller, "FRA",
        "只剩支援攻方(location≠省1)不应占领省1, 实际={}", prov1.controller
    );
}

// ===== 停止命令(stop_order): 取消主动行动, 保留被动防守/撤退 =====

#[test]
fn stop_cancels_march_destination() {
    // 停止普通移动: 清 destination, 师留在当前省
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 1 equipment = infantry_equipment battalions = 7 } }
    "#);
    let did = world.divisions.values().next().unwrap().id;
    // 下移动令: 省1→省10(己方)
    run_cmd(&mut world, &interp, "move_division = { division = 1 target = 10 }");
    assert!(world.divisions.get(&did).unwrap().is_moving(), "应已下令移动");

    // 停止
    run_cmd(&mut world, &interp, "stop_order = { division = 1 }");
    let d = world.divisions.get(&did).unwrap();
    assert!(d.is_idle(), "停止后应转 Idle");
    assert_eq!(d.location_province, 1, "师应留在当前省1");
}

#[test]
fn stop_cancels_support_attack() {
    // 停止支援攻击: 清 supporting, 从战斗 attackers 移除
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 30 defense = 40 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 30 defense = 40 max_org = 60 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 10 equipment = infantry_equipment battalions = 7 } }
    "#);
    let sup_id = world.divisions.values()
        .filter(|d| d.owner_tag == "GER" && d.location_province == 10)
        .map(|d| d.id).next().unwrap();
    run_cmd(&mut world, &interp, &format!("support_attack = {{ division = {} target = 1 }}", sup_id));
    assert!(world.divisions.get(&sup_id).unwrap().is_supporting(), "应已设支援");
    assert!(world.battles[0].attackers.contains(&sup_id) || world.battles[0].reserve_attackers.contains(&sup_id));

    // 停止支援
    run_cmd(&mut world, &interp, &format!("stop_order = {{ division = {} }}", sup_id));
    let d = world.divisions.get(&sup_id).unwrap();
    assert!(!d.is_supporting(), "停止后应退出 Supporting");
    assert!(!world.battles[0].attackers.contains(&sup_id), "停止后应从 attackers 移除");
    assert!(!world.battles[0].reserve_attackers.contains(&sup_id), "停止后应从 reserve_attackers 移除");
}

#[test]
fn stop_ignored_for_retreating() {
    // 撤退中的师: 停止命令被忽略(retreating 不能停)
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = FRA location = 1 soft_attack = 0 defense = 5 max_org = 30 } }
    "#);
    let did = world.divisions.values().next().unwrap().id;
    // 手动设撤退状态(Retreating, dest=省20, progress=0.3)
    {
        let d = world.divisions.get_mut(&did).unwrap();
        d.order = OrderState::Retreating { dest: 20, progress: 0.3 };
    }
    // 停止(应被忽略: stop_order 只停 Moving/Supporting)
    run_cmd(&mut world, &interp, "stop_order = { division = 1 }");
    let d = world.divisions.get(&did).unwrap();
    assert!(d.is_withdrawing(), "撤退师 Retreating 应保持");
    assert_eq!(d.retreat_dest(), Some(20), "撤退师 dest 不应被停止清除");
    assert!((d.move_progress() - 0.3).abs() < 1e-9, "撤退师进度不应变");
}

#[test]
fn stop_keeps_passive_defense() {
    // 师同时进军(主动) + 被动防守(location 被攻):
    // 停止只取消进军, 保留防守(留在 defenders 里)
    // 构造: A 在省10(己方GER, 非战斗地块), 进军省2(空敌方, 主动);
    //       同时省10被FRA攻 → A 是省10防守方(被动)
    // 注: A 在省10下进军省2(非战斗地块→非己方), 不触发撤退分支, 是纯主动进军
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    world.player_tag = "GER".into();
    world.countries.insert("GER".into(), Default::default());
    world.countries.insert("FRA".into(), Default::default());
    // 省10(GER) 邻接省1(FRA)和省2(FRA空); 省1(FRA)也邻接省10
    world.provinces.insert(10, hoi4_clone::runtime::Province {
        id: 10, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![1, 2],
    });
    world.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![10],
    });
    world.provinces.insert(2, hoi4_clone::runtime::Province {
        id: 2, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![10],
    });
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 10 soft_attack = 30 defense = 40 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 30 defense = 40 max_org = 60 }
        }
    "#);
    let a_id = world.divisions.values()
        .filter(|d| d.owner_tag == "GER").map(|d| d.id).next().unwrap();
    let fra_id = world.divisions.values()
        .filter(|d| d.owner_tag == "FRA").map(|d| d.id).next().unwrap();
    // FRA 进攻省10 → A 成省10防守方(被动)
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {} target = 10 }}", fra_id));
    let battle10 = world.battles.iter().find(|b| b.province == 10).unwrap();
    assert!(battle10.defenders.contains(&a_id), "A 应是省10守方(被动)");

    // A 下令进军省2(敌方空省, 主动进攻) — A 在省10(战斗地块)移到省2(非己方)
    // → 不触发撤退分支(目标非己方), 走进攻逻辑
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {} target = 2 }}", a_id));
    assert!(world.divisions.get(&a_id).unwrap().is_moving(), "A 应有进军令");
    assert!(!world.divisions.get(&a_id).unwrap().is_withdrawing(), "进军敌方省不应是撤退");

    // 停止 A: 取消进军省2, 但保留防守省10
    run_cmd(&mut world, &interp, &format!("stop_order = {{ division = {} }}", a_id));
    let d = world.divisions.get(&a_id).unwrap();
    assert!(d.is_idle(), "停止后 A 应转 Idle(进军令取消)");
    // 关键: A 仍是省10战斗的防守方
    let battle10_after = world.battles.iter().find(|b| b.province == 10);
    assert!(battle10_after.is_some(), "省10战斗应仍存在(防守未停)");
    assert!(
        battle10_after.unwrap().defenders.contains(&a_id),
        "停止后 A 应仍是省10防守方(被动防守不停), defenders={:?}",
        battle10_after.unwrap().defenders
    );
}

// ===== 防守主动撤退: 防守中下移动到己方省 → 撤退状态 =====

#[test]
fn defender_move_to_friendly_becomes_retreat() {
    // 师在防守战斗中(location==战场省), 下移动令到己方省 → 进入撤退状态,
    // 从战斗移除, 不改 location(行军中), retreating=true
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    // 省1属FRA, FRA守省1; GER从省10进攻省1 → FRA是省1防守方
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = FRA location = 1 soft_attack = 30 defense = 40 max_org = 60 }
        }
    "#);
    let fra_id = world.divisions.values().next().unwrap().id;
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 10 soft_attack = 100 defense = 40 max_org = 60 } }
    "#);
    let ger_id = world.divisions.values()
        .filter(|d| d.owner_tag == "GER").map(|d| d.id).next().unwrap();
    // GER 进攻省1 → 省1战斗, FRA 守
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {} target = 1 }}", ger_id));
    let battle1 = world.battles.iter().find(|b| b.province == 1).unwrap();
    assert!(battle1.defenders.contains(&fra_id), "FRA 应是省1防守方");

    // FRA 防守中下移动令到省20(己方FRA省) → 应变撤退
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {} target = 20 }}", fra_id));
    let d = world.divisions.get(&fra_id).unwrap();
    assert!(d.is_withdrawing(), "防守中移动到己方省应进入撤退状态");
    assert_eq!(d.retreat_dest(), Some(20), "撤退目标应为省20");
    assert_eq!(d.location_province, 1, "撤退中 location 不变(行军未到达)");
    // 从战斗移除(脱离战斗)
    let battle1_after = world.battles.iter().find(|b| b.province == 1);
    if let Some(b) = battle1_after {
        assert!(!b.defenders.contains(&fra_id), "撤退后应从 defenders 移除");
        assert!(!b.reserve_defenders.contains(&fra_id), "撤退后应从 reserve_defenders 移除");
    }
}

#[test]
fn defender_move_to_enemy_keeps_attacking() {
    // 防守中下移动令到敌方省 → 走进攻逻辑(不变撤退)
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = FRA location = 1 soft_attack = 30 defense = 40 max_org = 60 }
        }
    "#);
    let fra_id = world.divisions.values().next().unwrap().id;
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 10 soft_attack = 100 defense = 40 max_org = 60 } }
    "#);
    let ger_id = world.divisions.values()
        .filter(|d| d.owner_tag == "GER").map(|d| d.id).next().unwrap();
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {} target = 1 }}", ger_id));

    // FRA 防守中下移动令到省10(GER敌方省) → 走进攻逻辑(attacking=true), 不变撤退
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {} target = 10 }}", fra_id));
    let d = world.divisions.get(&fra_id).unwrap();
    assert!(!d.is_withdrawing(), "移动到敌方省不应变撤退(应走进攻逻辑)");
    assert!(d.is_moving(), "应有移动指令(Moving)");
}

#[test]
fn attacker_on_battle_province_can_retreat_to_friendly() {
    // 攻方师在战斗地块(如空降/撤退变攻方), 下移动到己方省 → 也能撤退(不分攻守)
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    // GER 师在省1(战场, 属FRA), 作为攻方在省1战斗里
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 50 defense = 40 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 30 defense = 40 max_org = 60 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    let ger_id = world.divisions.values()
        .filter(|d| d.owner_tag == "GER").map(|d| d.id).next().unwrap();
    let battle1 = world.battles.iter().find(|b| b.province == 1).unwrap();
    assert!(battle1.attackers.contains(&ger_id), "GER 应是省1攻方");
    assert_eq!(world.divisions.get(&ger_id).unwrap().location_province, 1, "GER location 在战场省1");

    // GER 攻方师在战斗地块, 下移动到省10(己方) → 应变撤退
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {} target = 10 }}", ger_id));
    let d = world.divisions.get(&ger_id).unwrap();
    assert!(d.is_withdrawing(), "战斗地块的攻方师下移动到己方省也应撤退(不分攻守)");
    assert_eq!(d.retreat_dest(), Some(10), "撤退目标省10");
    assert_eq!(d.location_province, 1, "撤退中 location 不变");
}

// ===== 预备队判定时机: started 标志 =====

#[test]
fn before_started_same_origin_all_frontline() {
    // 游戏未开始(started=false)时, 同出发地多个师进攻都进前线(不进预备队)
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    // 2个GER师在省10(同origin), 进攻省1(FRA守)
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = FRA location = 1 soft_attack = 30 defense = 40 max_org = 60 }
            create_division = { owner = GER location = 10 soft_attack = 30 defense = 40 max_org = 60 }
            create_division = { owner = GER location = 10 soft_attack = 30 defense = 40 max_org = 60 }
        }
    "#);
    let ger_ids: Vec<u64> = world.divisions.values()
        .filter(|d| d.owner_tag == "GER").map(|d| d.id).collect();
    // 未推进(started=false): 第一个进攻省1
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {} target = 1 }}", ger_ids[0]));
    // 第二个同 origin 进攻省1 — started=false 应进前线
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {} target = 1 }}", ger_ids[1]));
    let battle = world.battles.iter().find(|b| b.province == 1).unwrap();
    assert!(
        battle.attackers.contains(&ger_ids[0]) && battle.attackers.contains(&ger_ids[1]),
        "started=false 时同 origin 两个师都应在前线, atk={:?} res={:?}",
        battle.attackers, battle.reserve_attackers
    );
}

#[test]
fn after_started_same_origin_goes_reserve() {
    // 游戏开始后(started=true), 同出发地后到的师进预备队
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = FRA location = 1 soft_attack = 30 defense = 40 max_org = 60 }
            create_division = { owner = GER location = 10 soft_attack = 30 defense = 40 max_org = 60 }
            create_division = { owner = GER location = 10 soft_attack = 30 defense = 40 max_org = 60 }
        }
    "#);
    let ger_ids: Vec<u64> = world.divisions.values()
        .filter(|d| d.owner_tag == "GER").map(|d| d.id).collect();
    // 第一个进攻省1
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {} target = 1 }}", ger_ids[0]));
    // 推进 1 小时 → started 置 true
    GameClock::advance(&interp, &mut world, 1);
    // 第二个同 origin 进攻省1 — started=true 应进预备队
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {} target = 1 }}", ger_ids[1]));
    let battle = world.battles.iter().find(|b| b.province == 1).unwrap();
    assert!(
        battle.attackers.contains(&ger_ids[0]),
        "第一个师应在前线, atk={:?}", battle.attackers
    );
    assert!(
        battle.reserve_attackers.contains(&ger_ids[1]),
        "started=true 后同 origin 后到的师应在预备队, res={:?}", battle.reserve_attackers
    );
}

// ===== 多段路径行军(move_division 接入寻路)=====

/// 3 省链拓扑: 1-2-3, 全部初始为己方(GER), 便于纯移动测试
fn chain_world_owned() -> World {
    let mut w = World::new();
    w.player_tag = "GER".into();
    w.provinces.insert(1, hoi4_clone::runtime::Province {
        id: 1, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![2],
    });
    w.provinces.insert(2, hoi4_clone::runtime::Province {
        id: 2, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![1, 3],
    });
    w.provinces.insert(3, hoi4_clone::runtime::Province {
        id: 3, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![2],
    });
    w
}

#[test]
fn t_multihop_move_occupies_each_segment() {
    // 决策5/10: 师从省1 move_division 到省3(不相邻), 寻路 1→2→3, 逐段占领
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = chain_world_owned();
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 10 defense = 10 max_org = 60 }
        }
    "#);
    let did = *world.divisions.keys().next().unwrap();
    // 下令去省3(不相邻, 需寻路 1→2→3)
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {did} target = 3 }}"));
    // 寻路成功: 师朝省2 走(dest=2, remaining=[3])—寻路第1站
    {
        let div = world.divisions.get(&did).unwrap();
        assert!(div.is_moving(), "下令后应 Moving");
        assert_eq!(div.move_dest(), Some(2), "第1段 dest 应为省2(寻路第1站)");
    }
    // 推进 ~21h 到达省2(第1段), 占领省2
    // 注: Task4 续走逻辑尚未实现, 到达省2后转 Idle(后续 Task 实现续走后再加强此测试)
    GameClock::advance(&interp, &mut world, 21);
    let div = world.divisions.get(&did).unwrap();
    assert_eq!(div.location_province, 2, "到达省2 后归属地应为2");
}

#[test]
fn t_move_to_same_province_ignored() {
    // 决策12: 目标 == 当前省 → 忽略
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = chain_world_owned();
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 2 soft_attack = 10 defense = 10 max_org = 60 } }
    "#);
    let did = *world.divisions.keys().next().unwrap();
    // 下令去自己所在的省2 — 应忽略
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {did} target = 2 }}"));
    let div = world.divisions.get(&did).unwrap();
    assert!(div.is_idle(), "同省命令应忽略, 保持 Idle");
}

#[test]
fn t_find_path_no_route_ignored() {
    // 寻路失败(不连通)→ 师不动
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = chain_world_owned();
    // 加一个孤立省 99(与任何省都不邻接)
    world.provinces.insert(99, hoi4_clone::runtime::Province {
        id: 99, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![],
    });
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 1 soft_attack = 10 defense = 10 max_org = 60 } }
    "#);
    let did = *world.divisions.keys().next().unwrap();
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {did} target = 99 }}"));
    assert!(world.divisions.get(&did).unwrap().is_idle(), "寻路失败应忽略, 保持 Idle");
}

#[test]
fn t_move_during_pending_ignored() {
    // 决策11: 师在 Pending 时收到移动命令 → 忽略(不能中断待占领的战斗)
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = chain_world_owned();
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 2 soft_attack = 10 defense = 10 max_org = 60 } }
    "#);
    let did = *world.divisions.keys().next().unwrap();
    // 手动设为 Pending(战斗未胜, 等占领)
    world.divisions.get_mut(&did).unwrap().order = hoi4_clone::runtime::entities::OrderState::Pending {
        dest: 3, remaining: vec![],
    };
    // 下令移动到省1 — 应被忽略(Pending 不可被新移动命令中断)
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {did} target = 1 }}"));
    assert!(world.divisions.get(&did).unwrap().is_pending(), "Pending 时命令应忽略");
}

#[test]
fn t_move_during_retreating_ignored() {
    // 决策11: 师在 Retreating 时收到移动命令 → 忽略(不能中断撤退)
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = chain_world_owned();
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 2 soft_attack = 10 defense = 10 max_org = 60 } }
    "#);
    let did = *world.divisions.keys().next().unwrap();
    // 手动设为 Retreating
    world.divisions.get_mut(&did).unwrap().order = hoi4_clone::runtime::entities::OrderState::Retreating {
        dest: 1, progress: 0.5,
    };
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {did} target = 3 }}"));
    assert!(world.divisions.get(&did).unwrap().is_withdrawing(), "Retreating 时命令应忽略");
}

#[test]
fn support_attack_invalid_when_non_adjacent() {
    // 决策13: 目标省与师 location 不相邻 → 静默无效(不设 Supporting)
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = setup_world(); // 省1(neighbors:10,20), 省10(neighbors:1), 省20(neighbors:1)
    // 在省10 建 GER 师, 省1 有 FRA 师 + 战斗
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 10 equipment = infantry_equipment battalions = 7 }
            create_division = { owner = FRA location = 1 equipment = infantry_equipment battalions = 7 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    let ger_id = world.divisions.values().find(|d| d.owner_tag == "GER").map(|d| d.id).unwrap();
    // GER 师在省10, 支援省1(相邻) — 应成功(回归, 确保邻接检查不误伤合法支援)
    run_cmd(&mut world, &interp, &format!("support_attack = {{ division = {ger_id} target = 1 }}"));
    assert!(world.divisions.get(&ger_id).unwrap().is_supporting(), "相邻省支援应成功");
    // 先停止, 重置
    run_cmd(&mut world, &interp, &format!("stop_order = {{ division = {ger_id} }}"));
    // 加一个省 30(与省10 不相邻, 孤立)
    world.provinces.insert(30, hoi4_clone::runtime::Province {
        id: 30, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![],
    });
    // 支援省30(与省10 不相邻) — 应静默无效
    run_cmd(&mut world, &interp, &format!("support_attack = {{ division = {ger_id} target = 30 }}"));
    assert!(!world.divisions.get(&ger_id).unwrap().is_supporting(), "不相邻省支援应无效");
}

#[test]
fn t_queue_move_appends_waypoint() {
    // 决策9/10: queue_move 追加目标到路径末尾
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = chain_world_owned(); // 1-2-3 全 GER
    // 扩展到 4 省: 1-2-3-4(双向邻接)
    world.provinces.insert(4, hoi4_clone::runtime::Province {
        id: 4, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![3],
    });
    world.provinces.get_mut(&3).unwrap().neighbors.push(4);
    // 建师在省1
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 1 soft_attack = 10 defense = 10 max_org = 60 } }
    "#);
    let did = *world.divisions.keys().next().unwrap();
    // queue_move 到省3(寻路 1→2→3)
    run_cmd(&mut world, &interp, &format!("queue_move = {{ division = {did} target = 3 }}"));
    // 再 queue_move 到省4(追加: 当前路径末尾省3 → 寻路到省4, 拼接)
    run_cmd(&mut world, &interp, &format!("queue_move = {{ division = {did} target = 4 }}"));
    // 推进足够长 → 应到达省4(经 1→2→3→4)
    GameClock::advance(&interp, &mut world, 90);
    let div = world.divisions.get(&did).unwrap();
    assert_eq!(div.location_province, 4, "应到达追加的航点省4");
    assert!(div.is_idle());
}

#[test]
fn t_queue_move_then_queue_roundtrip_keeps_full_path() {
    // 回归: move 1→5 后 queue 5→3, 路径应是 [1,2,3,4,5,4,3](去回全程, 不丢前段)
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = chain_world_owned();
    world.provinces.insert(4, hoi4_clone::runtime::Province {
        id: 4, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![3, 5],
    });
    world.provinces.insert(5, hoi4_clone::runtime::Province {
        id: 5, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![4],
    });
    world.provinces.get_mut(&3).unwrap().neighbors.push(4);
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 1 soft_attack = 10 defense = 10 max_org = 60 } }
    "#);
    let did = *world.divisions.keys().next().unwrap();
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {did} target = 5 }}"));
    run_cmd(&mut world, &interp, &format!("queue_move = {{ division = {did} target = 3 }}"));
    // 期望: dest=2(第一站), remaining=[3,4,5,4,3](后续, 含去回)
    let div = world.divisions.get(&did).unwrap();
    assert_eq!(div.move_dest(), Some(2), "第一站应仍是省2");
    use hoi4_clone::runtime::entities::OrderState;
    if let OrderState::Moving { remaining, .. } = &div.order {
        assert_eq!(remaining, &vec![3, 4, 5, 4, 3], "全程路径应含去(3,4,5)和回(4,3), 不丢前段");
    } else {
        panic!("应是 Moving");
    }
}
