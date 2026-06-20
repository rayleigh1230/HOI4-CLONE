//! 诊断: 验证歼灭 + 人力消耗在真实场景下的行为
use hoi4_clone::ast::lower::lower_effects;
use hoi4_clone::commands::register_all;
use hoi4_clone::parser::parse;
use hoi4_clone::runtime::{GameClock, Interpreter, Registry, World};

fn main() {
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    world.player_tag = "GER".into();

    // 场景: 7个中坦营 vs 7个步兵营。中坦装甲60碾压步兵穿甲1
    let script = r#"
        create_division = { owner = GER location = 1 equipment = medium_tank battalions = 7 }
        create_division = { owner = FRA location = 1 equipment = infantry_equipment battalions = 7 }
        start_battle = { attacker = GER defender = FRA province = 1 }
    "#;
    let block = parse(script).unwrap();
    lower_effects(&block).iter().for_each(|e| {
        interp.run(&[e.clone()], &mut world);
    });

    let fra_id = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap().id;
    println!("=== 开战 ===");
    print(&world, fra_id);

    // 每 24 小时打印一次, 看 HP/人力/装备 变化
    for hour in [24, 48, 72, 96, 120, 144, 168] {
        GameClock::advance(&interp, &mut world, 24);
        println!("\n=== 第 {} 小时 ===", world.hour);
        print(&world, fra_id);
        // FRA 被歼灭就停
        if !world.divisions.contains_key(&fra_id) {
            println!("\n💀 FRA 已被歼灭(HP归零, 从世界删除)");
            break;
        }
    }
}

fn print(world: &World, fra_id: u64) {
    println!("  活跃战斗: {}", world.battles.len());
    for b in &world.battles {
        println!("  battle#{}: atk={:?} def={:?}", b.id, b.attackers, b.defenders);
    }
    for d in world.divisions.values() {
        let eq = d.equipment_ratio_only();
        let mp = d.manpower_ratio();
        println!(
            "{}#{}: HP={:.0}/{:.0} org={:.1}/{:.0} 装备={:.0}% 人力={:.0}% annih={} retreat={}",
            d.owner_tag, d.id, d.strength, d.max_strength, d.org, d.max_org,
            eq * 100.0, mp * 100.0, d.is_annihilated(), d.retreating
        );
    }
    let _ = fra_id;
}
