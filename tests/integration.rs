//! 端到端集成测试: 解析真实 HOI4 国策脚本 → 降级 → 执行 → 验证 World 变化
//!
//! 这是 M1 的核心验收:证明"用脚本运行时承载 HOI4 内容"方案可行。
use hoi4_clone::ast::lower::lower_effects;
use hoi4_clone::commands::register_all;
use hoi4_clone::parser::{parse, Value};
use hoi4_clone::runtime::{Interpreter, Registry, World};

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
