//! 作用域枚举测试(M3-2)
use hoi4_clone::ast::{Arg, Effect};
use hoi4_clone::commands::register_all;
use hoi4_clone::runtime::entities::{Country, Division, Province};
use hoi4_clone::runtime::{Interpreter, Registry, World};

fn two_states_world() -> World {
    let mut w = World::new();
    w.player_tag = "GER".into();
    w.countries.insert(
        "GER".into(),
        Country {
            tag: "GER".into(),
            owned_states: vec![1, 2],
            capital_state: 1,
            ..Default::default()
        },
    );
    w.provinces.insert(
        1,
        Province {
            id: 1,
            owner: "GER".into(),
            controller: "GER".into(),
            terrain: "plains".into(),
        },
    );
    w.provinces.insert(
        2,
        Province {
            id: 2,
            owner: "GER".into(),
            controller: "GER".into(),
            terrain: "forest".into(),
        },
    );
    w
}

fn inf_div(tag: &str, loc: u32) -> Division {
    Division {
        id: 0,
        owner_tag: tag.into(),
        location_province: loc,
        soft_attack: 10.0,
        hard_attack: 2.0,
        defense: 20.0,
        breakthrough: 5.0,
        armor: 0.0,
        piercing: 5.0,
        hardness: 0.0,
        combat_width: 10.0,
        max_org: 60.0,
        org: 60.0,
        max_strength: 20.0,
        strength: 20.0,
        ..Default::default()
    }
}

#[test]
fn t_every_owned_state_enumerates_both() {
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = two_states_world();
    let effs = vec![Effect::ForEach {
        scope: "every_owned_state".into(),
        filter: None,
        body: vec![Effect::Command {
            name: "add_to_variable".into(),
            params: vec![("state_count".into(), Arg::Num(1.0))],
        }],
    }];
    interp.run(&effs, &mut world);
    assert!(
        (world.get_var("state_count") - 2.0).abs() < 1e-9,
        "应遍历 2 个省"
    );
}

#[test]
fn t_all_army_enumerates_divisions() {
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = two_states_world();
    world.add_division(inf_div("GER", 1));
    world.add_division(inf_div("GER", 2));
    let effs = vec![Effect::ForEach {
        scope: "all_army".into(),
        filter: None,
        body: vec![Effect::Command {
            name: "add_to_variable".into(),
            params: vec![("div_count".into(), Arg::Num(1.0))],
        }],
    }];
    interp.run(&effs, &mut world);
    assert!(
        (world.get_var("div_count") - 2.0).abs() < 1e-9,
        "应遍历 2 个师"
    );
}

#[test]
fn t_every_country_enumerates_all() {
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = two_states_world();
    world.countries.insert(
        "FRA".into(),
        Country {
            tag: "FRA".into(),
            owned_states: vec![10],
            capital_state: 10,
            ..Default::default()
        },
    );
    let effs = vec![Effect::ForEach {
        scope: "every_country".into(),
        filter: None,
        body: vec![Effect::Command {
            name: "add_to_variable".into(),
            params: vec![("country_count".into(), Arg::Num(1.0))],
        }],
    }];
    interp.run(&effs, &mut world);
    assert!(
        (world.get_var("country_count") - 2.0).abs() < 1e-9,
        "应遍历 2 个国家"
    );
}
