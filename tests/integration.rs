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
    world.provinces.insert(
        1,
        hoi4_clone::runtime::Province {
            id: 1,
            owner: "AFG".into(),
            controller: "AFG".into(),
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
