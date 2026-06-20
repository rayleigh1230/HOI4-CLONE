//! demo: 加载 HOI4 风格国策脚本并执行,展示脚本运行时工作
use hoi4_clone::ast::lower::lower_effects;
use hoi4_clone::commands::register_all;
use hoi4_clone::parser::{parse, Value};
use hoi4_clone::runtime::{Interpreter, Registry, World};
use std::fs;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "examples/demo_focus.txt".into());
    let src = fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("无法读取脚本文件 {path}: {e}");
        std::process::exit(1);
    });
    println!("=== 加载脚本: {path} ===");

    let block = parse(&src).unwrap_or_else(|e| {
        eprintln!("解析失败: {e}");
        std::process::exit(2);
    });
    println!("✓ 解析成功,顶层字段数: {}", block.fields.len());

    // 定位 focus_tree → focus → completion_reward
    let tree = block
        .fields
        .iter()
        .find(|f| f.key == "focus_tree")
        .expect("未找到 focus_tree");
    let tree_b = match &tree.value {
        Value::Block(b) => b,
        _ => panic!("focus_tree 应为块"),
    };
    let focus = tree_b
        .fields
        .iter()
        .find(|f| f.key == "focus")
        .expect("focus_tree 内未找到 focus");
    let focus_b = match &focus.value {
        Value::Block(b) => b,
        _ => panic!(),
    };
    let focus_id = focus_b
        .fields
        .iter()
        .find(|f| f.key == "id")
        .and_then(|f| match &f.value {
            Value::Scalar(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default();
    println!("✓ 找到国策: {focus_id}");

    let reward = focus_b
        .fields
        .iter()
        .find(|f| f.key == "completion_reward")
        .expect("无 completion_reward");
    let reward_b = match &reward.value {
        Value::Block(b) => b,
        _ => panic!(),
    };
    let effs = lower_effects(reward_b);
    println!("✓ 降级为 {} 条 Effect", effs.len());

    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();

    println!("\n=== 执行 completion_reward ===");
    interp.run(&effs, &mut world);

    println!("\n=== 执行后 World 状态 ===");
    println!("  political_power = {}", world.get_var("political_power"));
    println!("  stability        = {}", world.get_var("stability"));
    println!("  industry_level   = {}", world.get_var("industry_level"));
    println!("\n✓ M1 验收通过:HOI4 脚本可被解析并执行");
}
