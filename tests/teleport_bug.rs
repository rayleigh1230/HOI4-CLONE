//! 回归测试: 撤退瞬移 bug(重构 OrderState enum 的验收基线)
//!
//! 复现场景(当前代码下 D 会瞬移回 origin, 此测试 FAIL):
//!   1. D(FRA) origin=省1, 在省1 被 GER 攻击战败
//!   2. D 撤退途中, origin 省1 被 GER 占领(controller 变 GER)
//!   3. D 到达撤退目标省2(己方), 变攻方遇 GER 师, 再战败
//!   4. 战败回 origin(省1) — 但省1 已是 GER 敌占区 → 瞬移进敌群(BUG)
//!
//! 重构后: safe_retreat_dest 检查 origin 是否己方; 省1 已丢 → 找邻省 → 都丢则歼灭。
//! D 不应出现在敌占省1。
use hoi4_clone::commands::register_all;
use hoi4_clone::runtime::{Division, GameClock, Interpreter, Province, Registry, World};
use hoi4_clone::runtime::entities::OrderState;

fn make_world() -> World {
    let mut w = World::new();
    w.player_tag = "GER".into();
    // 省1=FRA(D 的 origin + 初始防守地, 后被占), 省2=FRA(撤退目标), 省3=GER(攻方来源)
    w.provinces.insert(1, Province {
        id: 1, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![2, 3],
    });
    w.provinces.insert(2, Province {
        id: 2, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![1, 3],
    });
    w.provinces.insert(3, Province {
        id: 3, owner: "GER".into(), controller: "GER".into(),
        terrain: "plains".into(), neighbors: vec![1, 2],
    });
    w
}

/// FRA 师: 低 org + 巨量 HP,确保挨打时触发撤退(org 归零)而非歼灭
fn fra_div(loc: u32) -> Division {
    Division {
        id: 0, owner_tag: "FRA".into(), location_province: loc,
        soft_attack: 30.0, hard_attack: 2.0, defense: 40.0, breakthrough: 8.0,
        armor: 0.0, piercing: 5.0, hardness: 0.0, combat_width: 10.0,
        max_org: 10.0, org: 10.0, max_strength: 1000.0, strength: 1000.0,
        order: OrderState::Idle,
        ..Default::default()
    }
}

#[test]
fn t_no_teleport_to_enemy_origin_after_retreat() {
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut w = make_world();

    // D: FRA 守省1, origin=省1
    let d = w.add_division(fra_div(1));
    // C: GER 在省3, 高攻击(打垮 D 的 org)
    let c = Division {
        id: 0, owner_tag: "GER".into(), location_province: 3,
        soft_attack: 200.0, hard_attack: 0.0, defense: 40.0, breakthrough: 8.0,
        armor: 0.0, piercing: 5.0, hardness: 0.0, combat_width: 10.0,
        max_org: 60.0, org: 60.0, max_strength: 20.0, strength: 20.0,
        order: OrderState::Idle,
        ..Default::default()
    };
    let c = w.add_division(c);
    // 省1 战斗: C 攻 D 守
    w.battles.push(hoi4_clone::runtime::Battle {
        id: 0, province: 1,
        attackers: vec![c], defenders: vec![d],
        ..Default::default()
    });

    // 阶段1: 推进让 D 战败撤退(撤向邻省2=FRA 己方)
    for _ in 0..5 {
        GameClock::tick(&interp, &mut w);
    }
    let d_state = w.divisions.get(&d).expect("D 应存活");
    assert!(
        d_state.is_withdrawing() || d_state.is_pending(),
        "阶段1 后 D 应在 Retreating 或 Pending, 实际 order={:?}", d_state.order
    );

    // 阶段2: 模拟 D 的 origin 省1 被占(controller 变 GER)
    // 这是瞬移 bug 的关键: D 战败时若回 origin(省1), 会跳进敌占区
    w.provinces.get_mut(&1).unwrap().controller = "GER".into();

    // 阶段3: 推进让 D 走完撤退/再战败
    for _ in 0..60 {
        if !w.divisions.contains_key(&d) { break; } // D 歼灭也算合法
        GameClock::tick(&interp, &mut w);
    }

    // ===== 核心断言: D 不应出现在敌占省1(瞬移 bug) =====
    // bug: D 战败回 origin=省1, 但省1 已是 GER → 瞬移进敌群
    // 正确: safe_retreat_dest 检查 origin 非己方 → 找邻省省2(FRA) → 回省2; 或歼灭
    if let Some(d_final) = w.divisions.get(&d) {
        let loc = d_final.location_province;
        let loc_controller = w.provinces.get(&loc).map(|p| p.controller.as_str()).unwrap_or("");
        assert_ne!(
            loc_controller, "GER",
            "瞬移 bug: D 战败后停在省{loc}(controller=GER 敌占区)。\
             origin 省1 已丢, 应回邻省省2(FRA) 或歼灭, 不该瞬移进敌占区。\
             实际 order={:?}",
            d_final.order
        );
    }
    // D 被歼灭(w.divisions 无 D)也是合法结局
}
