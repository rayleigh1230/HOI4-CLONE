//! demo: 战斗+装备联动全流程文本战报
use hoi4_clone::ast::lower::lower_effects;
use hoi4_clone::commands::register_all;
use hoi4_clone::parser::parse;
use hoi4_clone::runtime::{GameClock, Interpreter, Registry, World};

fn print_state(world: &World, label: &str) {
    println!("\n--- {label} (第 {} 小时) ---", world.hour);
    println!(
        "{:<8} {:>6} {:>8} {:>8} {:>8}",
        "师", "HP", "组织度", "装备%", "状态"
    );
    let mut divs: Vec<&hoi4_clone::runtime::Division> = world.divisions.values().collect();
    divs.sort_by_key(|d| (d.owner_tag.clone(), d.id));
    for d in divs {
        let eq_pct = d.equipment_ratio() * 100.0;
        let status = if d.is_broken() { "💀破阵" } else { "战斗中" };
        println!(
            "{:<8} {:>6.1} {:>8.1} {:>7.1}% {:>8}",
            format!("{}#{}", d.owner_tag, d.id),
            d.strength,
            d.org,
            eq_pct,
            status
        );
    }
    // 打印仓库
    for (tag, c) in &world.countries {
        if !c.equipment_stockpile.is_empty() {
            let stock: Vec<String> = c
                .equipment_stockpile
                .iter()
                .map(|(k, v)| format!("{k}={v:.0}"))
                .collect();
            println!("  仓库 {tag}: {}", stock.join(", "));
        }
    }
}

fn main() {
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    world.player_tag = "GER".into();

    // 场景: GER 重装师 vs FRA 步兵, GER 仓库有补给
    let script = r#"
        add_equipment = { owner = GER type = rifles amount = 40 }
        add_equipment = { owner = FRA type = rifles amount = 10 }
        create_division = { owner = GER location = 1 soft_attack = 120 hard_attack = 5 defense = 30 breakthrough = 15 armor = 0 piercing = 10 max_org = 60 max_strength = 20 equipment = rifles equipment_amount = 100 }
        create_division = { owner = FRA location = 1 soft_attack = 60 hard_attack = 3 defense = 30 breakthrough = 10 armor = 0 piercing = 10 max_org = 60 max_strength = 20 equipment = rifles equipment_amount = 100 }
        start_battle = { attacker = GER defender = FRA province = 1 }
    "#;
    let block = parse(script).unwrap();
    lower_effects(&block).iter().for_each(|e| {
        let mut tmp = Vec::new();
        tmp.push(e.clone());
        interp.run(&tmp, &mut world);
    });

    println!("=== 装备系统联动战报 ===");
    println!("场景: GER 重装师(120软攻) vs FRA 步兵(60软攻)");
    println!("说明: 战斗每小时扣装备, 每24小时增援从仓库补\n");

    print_state(&world, "开战前");

    for h in [12, 12, 24, 24, 48] {
        GameClock::advance(&interp, &mut world, h);
        print_state(&world, &format!("+{}h", h));
    }

    println!("\n=== 结论 ===");
    let ger = world.divisions.values().find(|d| d.owner_tag == "GER").unwrap();
    let fra = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap();
    println!("GER 装备充足度: {:.0}% (仓库补给中)", ger.equipment_ratio() * 100.0);
    println!("FRA 装备充足度: {:.0}% (仓库耗尽, 难补充)", fra.equipment_ratio() * 100.0);
    if fra.equipment_ratio() < ger.equipment_ratio() {
        println!("✓ FRA 仓库少, 装备补充慢, 战力衰减更严重 — 装备战联动生效");
    } else {
        println!("⚠ 装备差异不明显, 需检查");
    }
}
