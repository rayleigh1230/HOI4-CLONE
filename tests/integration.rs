//! 端到端集成测试: 解析真实 HOI4 国策脚本 → 降级 → 执行 → 验证 World 变化
//!
//! 这是 M1 的核心验收:证明"用脚本运行时承载 HOI4 内容"方案可行。
use hoi4_clone::ast::lower::lower_effects;
use hoi4_clone::commands::register_all;
use hoi4_clone::parser::{parse, Value};
use hoi4_clone::runtime::{Interpreter, Registry, World};

/// 用注册好的解释器执行一段脚本(辅助)
fn run_script(src: &str, w: &mut World) {
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let b = parse(src).expect("脚本解析失败");
    let effs = lower_effects(&b);
    interp.run(&effs, w);
}

/// 从解析后的 Block 中按 key 取出子块
fn reward_block(b: &hoi4_clone::parser::Block) -> &hoi4_clone::parser::Block {
    let f = b.fields.iter().find(|f| f.key == "completion_reward").expect("应有 completion_reward");
    match &f.value {
        Value::Block(b) => b,
        _ => panic!("completion_reward 应为块"),
    }
}

#[test]
fn focus_add_pp_then_stability() {
    // 模拟一个国策 completion_reward: 加 150 政治点, 若 pp>=150 则加稳定度
    // 注意: limit 里的 political_power >= 150 是裸比较,会被降级成真正的 Trigger::Compare
    //       在 interp.eval 中读 world.get_var("political_power")。由于 add_political_power 先执行
    //       把 pp 设为 150, 满足 >=150, 所以 stability 被加 0.05。
    let src = r#"
        completion_reward = {
            add_political_power = 150
            if = {
                limit = { political_power >= 150 }
                add_stability = 0.05
            }
        }
    "#;
    let b = parse(src).unwrap();
    let inner = reward_block(&b);
    let effs = lower_effects(inner);

    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();

    interp.run(&effs, &mut world);

    assert!((world.get_var("political_power") - 150.0).abs() < 1e-9);
    assert!((world.get_var("stability") - 0.05).abs() < 1e-9);
}

#[test]
fn compare_trigger_false_branch() {
    // 反向用例:阈值 200,但 pp 只加到 150,比较应失败,stability 保持 0
    // 这证明 Trigger::Compare 真的被求值(而非恒 true)
    let src = r#"
        completion_reward = {
            add_political_power = 150
            if = {
                limit = { political_power >= 200 }
                add_stability = 0.05
            }
        }
    "#;
    let b = parse(src).unwrap();
    let inner = reward_block(&b);
    let effs = lower_effects(inner);

    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    interp.run(&effs, &mut world);

    assert!((world.get_var("political_power") - 150.0).abs() < 1e-9);
    assert!(
        world.get_var("stability").abs() < 1e-9,
        "pp=150 < 200, 比较应失败, stability 应保持 0"
    );
}

#[test]
fn focus_afghanistan_real_fragment() {
    // 来自 afghanistan.txt AFG_expand_telegraph_network 的真实 completion_reward
    // M2: 未注册的 trigger 默认 false, 故需注册 stub 让 limit 通过
    let src = r#"
        completion_reward = {
            every_owned_state = {
                limit = { is_owned_and_controlled_by = AFG }
                add_to_variable = { AFG_state_development_production_speed = 0.05 }
                add_to_variable = { AFG_state_development_state_resources_factor = 0.05 }
            }
        }
    "#;
    let b = parse(src).unwrap();
    let inner = reward_block(&b);
    let effs = lower_effects(inner);

    let mut reg = Registry::new();
    register_all(&mut reg);
    // 注册测试用 stub trigger: is_owned_and_controlled_by 恒 true
    reg.register_trigger("is_owned_and_controlled_by", |_, _| Ok(true));
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    // M3: every_owned_state 真实枚举, 需设置 AFG 国家 + 省让它能遍历
    world.player_tag = "AFG".into();
    world.countries.insert(
        "AFG".into(),
        hoi4_clone::runtime::Country {
            tag: "AFG".into(),
            owned_states: vec![1],
            capital_state: 1,
            ..Default::default()
        },
    );
    let sid = 1 * 1000;
    world.states.insert(sid, hoi4_clone::runtime::State {
        id: sid, owner: "AFG".into(), controller: "AFG".into(),
        ..Default::default()
    });
    world.provinces.insert(
        1,
        hoi4_clone::runtime::Province {
            id: 1, state_id: sid,
            terrain: "mountain".into(),
            ..Default::default()
        },
    );
    interp.run(&effs, &mut world);

    assert!(
        (world.get_var("AFG_state_development_production_speed") - 0.05).abs() < 1e-9,
        "AFG 变量应被加 0.05"
    );
    assert!(
        (world.get_var("AFG_state_development_state_resources_factor") - 0.05).abs() < 1e-9
    );
}

#[test]
fn full_focus_tree_parse_and_execute() {
    // 完整的 focus_tree 结构:外层 focus_tree → focus → completion_reward
    let src = r#"
        focus_tree = {
            id = demo_tree
            focus = {
                id = DEMO_test
                x = 0
                y = 0
                cost = 5
                completion_reward = {
                    add_political_power = 100
                    set_flag = demo_done
                }
            }
        }
    "#;
    let b = parse(src).unwrap();
    // 找到 focus_tree → focus → completion_reward
    let tree = b.fields.iter().find(|f| f.key == "focus_tree").expect("应有 focus_tree");
    let tree_b = match &tree.value {
        Value::Block(b) => b,
        _ => panic!(),
    };
    let focus = tree_b.fields.iter().find(|f| f.key == "focus").expect("应有 focus");
    let focus_b = match &focus.value {
        Value::Block(b) => b,
        _ => panic!(),
    };
    let inner = reward_block(focus_b);
    let effs = lower_effects(inner);

    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    interp.run(&effs, &mut world);

    assert!((world.get_var("political_power") - 100.0).abs() < 1e-9);
    assert!(world.has_flag("demo_done"));
}

#[test]
fn t_create_division_from_template() {
    // 端到端: create_division 用 template 参数走数据驱动汇总
    let mut w = World::new();
    // 取 GameData 里第一个可用模板(德国 OOB 加载的真实模板)
    let tmpl_name = w.data.templates.keys().next()
        .expect("GameData 应至少有一个模板").clone();
    let script = format!(
        "create_division = {{ owner = GER template = \"{}\" location = 1 }}",
        tmpl_name
    );
    run_script(&script, &mut w);
    assert_eq!(w.divisions_of("GER").len(), 1, "应建出 1 个师(模板={})", tmpl_name);
    let did = *w.divisions_of("GER").first().unwrap();
    let d = w.divisions.get(&did).unwrap();
    // 数据驱动师应有非零属性(由真实模板汇总算出)
    assert!(d.combat_width > 0.0, "应有战斗宽度, 实际 {}", d.combat_width);
    assert!(d.max_strength > 0.0, "应有 HP, 实际 {}", d.max_strength);
}

#[test]
fn t_create_division_unknown_template_errors() {
    // 未知模板应返回错误(而非静默建空师)
    let mut w = World::new();
    let script = "create_division = { owner = GER template = \"nonexistent_xyz\" location = 1 }";
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let b = parse(script).unwrap();
    let effs = lower_effects(&b);
    interp.run(&effs, &mut w);
    // 未知模板 → error_log 应记录错误, 且不建师
    assert!(!w.error_log.is_empty(), "未知模板应产生错误");
    assert_eq!(w.divisions_of("GER").len(), 0, "未知模板不应建师");
}

#[test]
fn t_country_modifier_affects_combat() {
    // GER 加 +100% soft_attack(Add), 战斗伤害应显著提升
    use hoi4_clone::runtime::{World, Interpreter, Registry, GameClock};
    use hoi4_clone::commands::register_all;
    use hoi4_clone::ast::lower::lower_effects;
    use hoi4_clone::parser::parse;

    let mut w = World::new();
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);

    // 先加 modifier
    let setup = r#"
        add_country_modifier = { tag = GER stat = soft_attack value = 1.0 }
    "#;
    interp.run(&lower_effects(&parse(setup).unwrap()), &mut w);

    // 建 GER 师和 FRA 师, 开战
    let battle_setup = r#"
        create_division = { owner = GER location = 1 soft_attack = 30 hard_attack = 0 defense = 10 max_strength = 100 }
        create_division = { owner = FRA location = 2 soft_attack = 0 hard_attack = 0 defense = 10 max_strength = 100 }
        create_state = { id = 1000 owner = GER }
        create_state = { id = 2000 owner = FRA }
        create_province = { id = 1 state = 1000 neighbors = { 2 } }
        create_province = { id = 2 state = 2000 neighbors = { 1 } }
        start_battle = { attacker = GER defender = FRA province = 2 }
    "#;
    interp.run(&lower_effects(&parse(battle_setup).unwrap()), &mut w);

    // 记录 FRA 师 HP, 结算 1 小时
    let fra_id = w.divisions_of("FRA")[0];
    let hp_before = w.divisions.get(&fra_id).unwrap().strength;
    GameClock::advance(&interp, &mut w, 1);
    let hp_after = w.divisions.get(&fra_id).unwrap().strength;
    let loss = hp_before - hp_after;
    // +100% soft_attack → 攻击翻倍 → 伤害应明显(>0)
    assert!(loss > 0.0, "modifier 生效后应有伤害, 实际 loss={loss}");

    // 对照: 无 modifier 时同样配置的伤害(应小于有 modifier)
    let mut w2 = World::new();
    let mut reg2 = Registry::new();
    register_all(&mut reg2);
    let interp2 = Interpreter::new(reg2);
    interp2.run(&lower_effects(&parse(battle_setup).unwrap()), &mut w2);
    let fra_id2 = w2.divisions_of("FRA")[0];
    let hp_before2 = w2.divisions.get(&fra_id2).unwrap().strength;
    GameClock::advance(&interp2, &mut w2, 1);
    let loss2 = hp_before2 - w2.divisions.get(&fra_id2).unwrap().strength;
    assert!(loss > loss2, "有 +100% modifier 的伤害应大于无 modifier: {loss} > {loss2}");
}

#[test]
fn t_factor_suffix_parses_as_multiply() {
    // soft_attack_factor 应解析为 Multiply(独立乘), soft_attack 应解析为 Add
    use hoi4_clone::combat::modifier::{parse_modifier_token, ModifierOp};
    let (_, op1) = parse_modifier_token("soft_attack").unwrap();
    let (_, op2) = parse_modifier_token("soft_attack_factor").unwrap();
    assert_eq!(op1, ModifierOp::Add);
    assert_eq!(op2, ModifierOp::Multiply);
}

#[test]
fn t_empty_modifiers_exact_same_as_before() {
    // 空 ModifierStack 时 effective_soft_attack 应等于 面板×补给(无 modifier)
    use hoi4_clone::runtime::Division;
    use hoi4_clone::combat::modifier::ModifierStack;

    let mut d = Division::default();
    d.soft_attack = 30.0;
    d.equipment_held.insert("x".into(), 100.0);
    d.equipment_need.insert("x".into(), 100.0);
    d.manpower_held = 1000.0;
    d.manpower_need = 1000.0;

    let empty = ModifierStack::new();
    let with_mods = d.effective_soft_attack(&empty);
    // 满编时 supply_ratio=1.0, 空 modifier multiplier=1.0 → 30.0
    assert!((with_mods - 30.0).abs() < 1e-9, "空栈应精确还原: 30×1.0×1.0=30, 实际 {}", with_mods);
}

#[test]
fn t_occupation_changes_state_controller() {
    use hoi4_clone::runtime::World;
    use hoi4_clone::runtime::Interpreter;
    use hoi4_clone::runtime::Registry;
    use hoi4_clone::commands::register_all;
    use hoi4_clone::ast::lower::lower_effects;
    use hoi4_clone::parser::parse;

    let mut w = World::new();
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let src = r#"
        create_state = { id = 1 owner = GER manpower = 500000 state_category = large_city cores = { GER } }
        create_province = { id = 10 state = 1 terrain = plains neighbors = { 11 } }
    "#;
    interp.run(&lower_effects(&parse(src).unwrap()), &mut w);

    // State 存在且含省 10
    let s = w.states.get(&1).expect("State 1 应存在");
    assert_eq!(s.owner, "GER");
    assert!((s.manpower - 500000.0).abs() < 1e-9);
    assert!(s.provinces.contains(&10), "反向注册: State 应含省 10");

    // 省份归属从 State 派生
    assert_eq!(w.province_controller(10).unwrap_or(""), "GER");
    assert_eq!(w.province_owner(10).unwrap_or(""), "GER");
}

#[test]
fn t_war_system_basic() {
    // 宣战后两国互为敌人; 无战争时中立
    use hoi4_clone::runtime::World;
    let mut w = World::new();
    // 无战争: 中立
    assert!(!w.are_at_war("GER", "FRA"));
    // 宣战
    w.declare_war("GER", "FRA");
    assert!(w.are_at_war("GER", "FRA"));
    // 第三国中立
    assert!(!w.are_at_war("GER", "SOV"));
    assert!(!w.are_at_war("FRA", "SOV"));
    // 白和后停战
    w.wars.retain(|war| {
        !(war.attackers.contains("GER") && war.defenders.contains("FRA"))
    });
    assert!(!w.are_at_war("GER", "FRA"));
}

#[test]
fn t_faction_auto_join_war() {
    // 阵营成员宣战时自动加入
    use hoi4_clone::runtime::World;
    let mut w = World::new();
    // GER 和 ITA 同阵营 "Axis"
    w.countries.entry("GER".into()).or_default().faction = Some("Axis".into());
    w.countries.entry("ITA".into()).or_default().faction = Some("Axis".into());
    w.countries.entry("FRA".into()).or_default();
    // GER 宣战 FRA → ITA 自动在攻方
    w.declare_war("GER", "FRA");
    assert!(w.are_at_war("GER", "FRA"));
    assert!(w.are_at_war("ITA", "FRA"), "阵营成员 ITA 应自动与 FRA 交战");
}

#[test]
fn t_neutral_countries_dont_fight() {
    // 未宣战的两军在同省不开打
    use hoi4_clone::runtime::{World, GameClock, Interpreter, Registry};
    use hoi4_clone::commands::register_all;
    let mut w = World::new();
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let setup = r#"
        create_state = { id = 1000 owner = GER }
        create_state = { id = 2000 owner = FRA }
        create_province = { id = 1 state = 1000 neighbors = { 2 } }
        create_province = { id = 2 state = 2000 neighbors = { 1 } }
        create_division = { owner = GER location = 1 soft_attack = 30 max_strength = 100 }
        create_division = { owner = FRA location = 2 soft_attack = 30 max_strength = 100 }
    "#;
    interp.run(&hoi4_clone::ast::lower::lower_effects(&hoi4_clone::parser::parse(setup).unwrap()), &mut w);
    // 未宣战: 推进也不开打
    assert!(!w.are_at_war("GER", "FRA"), "未宣战应为中立");
    GameClock::advance(&interp, &mut w, 10);
    assert!(w.battles.is_empty(), "中立国不应开战");
    // 宣战后才开打
    w.declare_war("GER", "FRA");
    assert!(w.are_at_war("GER", "FRA"));
}
