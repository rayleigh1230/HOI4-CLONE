# 多段路径行军 + 航点规划 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现多段路径行军(自动寻路)+ 航点规划(queue_move)+ 支援攻击邻接收敛 + 路径失效应对,让师能沿多省路线逐段推进。

**Architecture:** 给 `OrderState::Moving` 加 `remaining: Vec<u32>`(剩余中转省)字段;新建 `pathfinding.rs`(BFS 寻路 + `is_passable`/`edge_weight` 双插槽);`move_division` 下单时寻路填 remaining,新增 `queue_move` 追加航点,`advance_movement` 到达后续走。战败/停止复用现有规则(状态机一变 remaining 自动消失)。决策依据见 `docs/superpowers/specs/2026-06-23-multi-hop-marching-design.md`(14 条决策)。

**Tech Stack:** Rust 2021,stable-x86_64-pc-windows-gnu,无外部依赖,cargo test 验证。

---

## 文件结构

| 文件 | 责任 | 改动类型 |
|---|---|---|
| `src/runtime/entities.rs` | `OrderState::Moving` 加 `remaining` 字段 | 修改(改 enum + 1 处运行时构造) |
| `src/combat/pathfinding.rs` | BFS 寻路 + 双插槽 | **新建** |
| `src/combat/mod.rs` | 模块声明 | 修改(加 1 行) |
| `src/combat/commands.rs` | move_division 寻路 + queue_move + support_attack 邻接 | 修改 |
| `src/combat/movement.rs` | 到达续走 + dest 可进入性检查 + invalidate 函数 | 修改 |
| `src/wasm_api.rs` | engine_queue_move FFI | 修改 |
| `tests/battle.rs` | 现有 Moving 构造补字段 + 新增集成测试 | 修改 |

**关键约定**:`remaining` 存"dest 走完之后还要去的省",**不含当前 dest**。单段移动 = `remaining: vec![]`。

**任务依赖**:Task 1(改 enum)是地基,所有后续任务依赖它。Task 1 完成后项目必须能编译(给所有 Moving 构造点补 `vec![]`)。Task 2(寻路)独立。Task 3(move_division)依赖 1+2。Task 4(续走)依赖 1。Task 5(queue_move)依赖 1+2+3。Task 6(support_attack)依赖 1。Task 7(失效应对)依赖 1+2。Task 8(WASM)依赖 5。

---

## Task 1: OrderState::Moving 加 remaining 字段(地基)

**Files:**
- Modify: `src/runtime/entities.rs:36`
- Modify: `src/combat/commands.rs:287`(运行时唯一构造点)
- Modify: `src/combat/movement.rs`(测试构造点:289,309,330,335,355,388,394,419,430)
- Modify: `tests/battle.rs`(测试构造点:516,551)
- Modify: `src/combat/resolve.rs:646`(测试构造点)

- [ ] **Step 1: 改 enum 定义**

修改 `src/runtime/entities.rs:36`,把 Moving 变体改为:
```rust
    /// 主动行军: dest=当前段终点, progress=0..1, hostile=是否进军敌方地块(红箭头),
    /// origin=当前段出发地, remaining=dest 之后还要去的省(多段路径, 不含 dest)
    Moving { dest: u32, progress: f64, hostile: bool, origin: u32, remaining: Vec<u32> },
```

- [ ] **Step 2: 改运行时构造点**

修改 `src/combat/commands.rs:286-291`,move_division 设 Moving 处加 remaining:
```rust
        // 设移动状态: 进入 Moving, 记录 origin=当前省
        // remaining 暂为空(Task 3 会用寻路结果填充); 单段移动 = remaining 空
        if let Some(d) = w.divisions.get_mut(&div_id) {
            d.order = OrderState::Moving {
                dest: target, progress: 0.0,
                hostile: is_hostile, origin: cur_loc,
                remaining: vec![],
            };
        }
```

- [ ] **Step 3: 补所有测试构造点的 remaining 字段**

对所有形如 `OrderState::Moving { dest: X, progress: Y, hostile: Z, origin: W }` 的构造,补 `remaining: vec![]`。涉及文件:
- `src/combat/movement.rs`: 行 289, 309, 330, 335, 355, 388, 394, 419, 430
- `tests/battle.rs`: 行 516, 551
- `src/combat/resolve.rs`: 行 646

每处改成(以 movement.rs:289 为例):
```rust
            order: OrderState::Moving { dest: 2, progress: 0.0, hostile: false, origin: 1, remaining: vec![] },
```

- [ ] **Step 4: 编译验证**

Run: `cargo build --tests`
Expected: 编译通过,无错误(若有遗漏的构造点,编译器会逐个报错指明位置,补 `remaining: vec![]` 即可)

- [ ] **Step 5: 跑全部测试确认无回归**

Run: `cargo test`
Expected: 全部通过(101 个现有测试)。此任务只是加字段+补默认值,不改变任何行为。

- [ ] **Step 6: Commit**

```bash
git add src/runtime/entities.rs src/combat/commands.rs src/combat/movement.rs src/combat/resolve.rs tests/battle.rs
git commit -m "refactor(land): OrderState::Moving 加 remaining 字段(多段路径地基, 单段行为不变)"
```

---

## Task 2: 寻路模块 pathfinding.rs(决策1+2)

**Files:**
- Create: `src/combat/pathfinding.rs`
- Modify: `src/combat/mod.rs:1-8`

- [ ] **Step 1: 模块声明**

修改 `src/combat/mod.rs`,加一行(放在 movement 之前,字母序):
```rust
//! 战斗模块
pub mod commands;
pub mod equipment_data;
pub mod movement;
pub mod pathfinding;
pub mod recovery;
pub mod reinforce;
pub mod resolve;
pub mod width;
```

- [ ] **Step 2: 写失败测试(寻路基础)**

创建 `src/combat/pathfinding.rs`,先只写测试和函数签名(返回 unimplemented):
```rust
//! 寻路: BFS 找两省间最短路径(跳数最少)。
//! 设计: is_passable/edge_weight 双插槽, 当前权重全 1(=BFS);
//!       将来加距离数据改 edge_weight 即变 Dijkstra, 改 is_passable 即加避让规则。
use crate::runtime::World;

/// 从 from 寻路到 to, 返回路径(含 to, 不含 from)。
/// - 路径第一个 = 下一站, 最后一个 = 最终目标
/// - from == to 或不连通返回 None
pub fn find_path(_world: &World, _from: u32, _to: u32) -> Option<Vec<u32>> {
    unimplemented!()
}

/// 判断一个省能否作为寻路中转(穿过)。当前恒 true。
/// 未来扩展: 未开战不得入境 / 绕开驻军省 — 只改此函数。
fn is_passable(_world: &World, _prov: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Province, World};

    /// 链式拓扑 1-2-3-4(双向邻接)
    fn chain_world() -> World {
        let mut w = World::new();
        w.provinces.insert(1, Province { id: 1, neighbors: vec![2], ..Default::default() });
        w.provinces.insert(2, Province { id: 2, neighbors: vec![1, 3], ..Default::default() });
        w.provinces.insert(3, Province { id: 3, neighbors: vec![2, 4], ..Default::default() });
        w.provinces.insert(4, Province { id: 4, neighbors: vec![3], ..Default::default() });
        w
    }

    #[test]
    fn t_find_path_adjacent() {
        let w = chain_world();
        // 1→2 相邻, 返回单元素 [2]
        assert_eq!(find_path(&w, 1, 2), Some(vec![2]));
    }

    #[test]
    fn t_find_path_multi_hop() {
        let w = chain_world();
        // 1→4 经 2,3, 返回 [2,3,4]
        assert_eq!(find_path(&w, 1, 4), Some(vec![2, 3, 4]));
    }

    #[test]
    fn t_find_path_same_province() {
        let w = chain_world();
        assert_eq!(find_path(&w, 2, 2), None, "同省应返回 None");
    }

    #[test]
    fn t_find_path_disconnected() {
        let mut w = chain_world();
        // 加一个孤立省 9
        w.provinces.insert(9, Province { id: 9, neighbors: vec![], ..Default::default() });
        assert_eq!(find_path(&w, 1, 9), None, "不连通应返回 None");
    }
}
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test --lib pathfinding`
Expected: 4 个测试 panic(unimplemented!())

- [ ] **Step 4: 实现 find_path(BFS)**

替换 `find_path` 函数体为:
```rust
pub fn find_path(world: &World, from: u32, to: u32) -> Option<Vec<u32>> {
    use std::collections::{HashSet, VecDeque};
    if from == to {
        return None;
    }
    // 起点或终点不在地图里 → 无法寻路
    if !world.provinces.contains_key(&from) || !world.provinces.contains_key(&to) {
        return None;
    }
    let mut queue: VecDeque<u32> = VecDeque::new();
    let mut visited: HashSet<u32> = HashSet::new();
    let mut came_from: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    queue.push_back(from);
    visited.insert(from);
    while let Some(cur) = queue.pop_front() {
        if cur == to {
            break;
        }
        let neighbors = world.provinces.get(&cur).map(|p| p.neighbors.clone()).unwrap_or_default();
        for n in neighbors {
            if !visited.contains(&n) && is_passable(world, n) {
                visited.insert(n);
                came_from.insert(n, cur);
                queue.push_back(n);
            }
        }
    }
    // to 未被访问到 → 不连通
    if !visited.contains(&to) {
        return None;
    }
    // 从 to 回溯到 from
    let mut path = Vec::new();
    let mut cur = to;
    while cur != from {
        path.push(cur);
        cur = *came_from.get(&cur).expect("came_from 应完整");
    }
    path.reverse(); // 现在 path = [下一站, ..., to],不含 from
    Some(path)
}
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test --lib pathfinding`
Expected: 4 个测试 PASS

- [ ] **Step 6: Commit**

```bash
git add src/combat/pathfinding.rs src/combat/mod.rs
git commit -m "feat(pathfinding): BFS 寻路模块 + is_passable 插槽(权重全1=跳数最少)"
```

---

## Task 3: move_division 接入寻路 + 边界处理(决策10/11/12)

**Files:**
- Modify: `src/combat/commands.rs`(move_division 函数,约 247-297)
- Test: `tests/battle.rs`(新增)

- [ ] **Step 1: 写失败测试(多段占领)**

在 `tests/battle.rs` 末尾追加:
```rust

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
    // 决策5: 师从省1 move_division 到省3, 应逐段占领省2、省3(己方省不损 org)
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = chain_world_owned();
    // 在省1 建师
    run_setup(&mut world, &interp, r#"
        _setup = {
            create_division = { owner = GER location = 1 soft_attack = 10 defense = 10 max_org = 60 }
        }
    "#);
    let did = *world.divisions.keys().next().unwrap();
    // 下令去省3(不相邻, 需寻路 1→2→3)
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {did} target = 3 }}"));
    // 推进足够多小时应到达省3(MOVE_RATE=0.05, 每段约 20h, 两段约 40h)
    GameClock::advance(&interp, &mut world, 45);
    let div = world.divisions.get(&did).unwrap();
    assert_eq!(div.location_province, 3, "师应到达省3");
    assert!(div.is_idle(), "到达后应转 Idle");
}
```

> 注:`run_cmd` 辅助函数已存在于 battle.rs(约 921-926 行)。若不存在,用 run_setup 同款逻辑跑顶层命令。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test t_multihop_move_occupies_each_segment`
Expected: FAIL(现在 move_division 不寻路,dest=3 但无邻接推进,师卡在省1)

- [ ] **Step 3: 改 move_division 接入寻路**

修改 `src/combat/commands.rs` 的 move_division 函数(约 247-297)。在"非防守撤退"分支(现有 `let enemies...` 之前)插入寻路 + 边界检查,并替换 Moving 构造。完整新逻辑:

```rust
    // 查目标省有无敌军(非己方的师; 排除撤退师)
    let enemies: Vec<u64> = w.divisions.values()
        .filter(|d| d.location_province == target && d.owner_tag != owner && !d.is_withdrawing())
        .map(|d| d.id)
        .collect();
    // 【边界B】师在 Pending/Retreating/Supporting → 忽略移动命令(不能中断战斗/撤退/支援)
    let blocked_state = match w.divisions.get(&div_id) {
        Some(d) => d.is_pending() || d.is_withdrawing() || d.is_supporting(),
        None => false,
    };
    if blocked_state {
        return Ok(());
    }
    // 【边界C】目标 == 当前省 → 忽略(无意义)
    if target == cur_loc {
        return Ok(());
    }
    // 【寻路】find_path 返回 [下一站, ..., 最终目标]; None 则师不动
    let path = match crate::combat::pathfinding::find_path(w, cur_loc, target) {
        Some(p) => p,
        None => return Ok(()), // 不连通或同省, 静默忽略
    };
    // path 非空: 拆成 dest(第一站) + remaining(后续)
    let first = path[0];
    let remaining: Vec<u32> = path[1..].to_vec();
    // 进军判定: 第一站非己方控制 → 进军红箭头
    let first_controller = w.provinces.get(&first).map(|p| p.controller.as_str()).unwrap_or("");
    let is_hostile = first_controller != owner;
    // 设移动状态: dest=第一站, remaining=后续站
    if let Some(d) = w.divisions.get_mut(&div_id) {
        d.order = OrderState::Moving {
            dest: first, progress: 0.0,
            hostile: is_hostile, origin: cur_loc,
            remaining,
        };
    }
    // 第一站有敌军防守 → 开战
    let first_enemies: Vec<u64> = w.divisions.values()
        .filter(|d| d.location_province == first && d.owner_tag != owner && !d.is_withdrawing())
        .map(|d| d.id)
        .collect();
    if !first_enemies.is_empty() {
        join_as_attacker(w, div_id, first, &first_enemies);
    }
    Ok(())
```

替换原 move_division 中从 `let enemies...` 到函数结尾的整段代码。注意:原 `enemies` 变量若未再被使用则删除(上面已用 `first_enemies` 替代开战判定)。

- [ ] **Step 4: 编译并跑新测试**

Run: `cargo test t_multihop_move_occupies_each_segment`
Expected: PASS

- [ ] **Step 5: 跑全部测试确认无回归**

Run: `cargo test`
Expected: 全部通过(现有单段移动行为:相邻省寻路返回单元素 path,remaining=[],行为不变)

- [ ] **Step 6: Commit**

```bash
git add src/combat/commands.rs tests/battle.rs
git commit -m "feat(land): move_division 接入 BFS 寻路 + 边界B/C 处理(单段行为不变)"
```

---

## Task 4: advance_movement 到达后续走(决策5+7)

**Files:**
- Modify: `src/combat/movement.rs`(advance_movement 函数 + Arrival 结构)
- Test: `src/combat/movement.rs`(新增测试模块内的测试)

- [ ] **Step 1: 写失败测试(占领中途省后续走)**

在 `src/combat/movement.rs` 的 tests 模块内追加:
```rust

    // ===== 多段路径: 占领中途省后续走 =====

    /// 3 省链 1-2-3, 省2 是敌方空省(待占领), 省3 己方
    fn chain_1_2_3_world() -> World {
        let mut w = World::new();
        w.provinces.insert(1, crate::runtime::Province {
            id: 1, owner: "GER".into(), controller: "GER".into(),
            terrain: "plains".into(), neighbors: vec![2],
        });
        w.provinces.insert(2, crate::runtime::Province {
            id: 2, owner: "FRA".into(), controller: "FRA".into(),
            terrain: "plains".into(), neighbors: vec![1, 3],
        });
        w.provinces.insert(3, crate::runtime::Province {
            id: 3, owner: "GER".into(), controller: "GER".into(),
            terrain: "plains".into(), neighbors: vec![2],
        });
        w
    }

    #[test]
    fn t_multihop_continues_after_mid_capture() {
        // 师从省1 Moving 到省2(remaining=[3]), 占领省2 后应续走到省3
        let mut w = chain_1_2_3_world();
        let d = Division {
            id: 0, owner_tag: "GER".into(), location_province: 1,
            order: OrderState::Moving {
                dest: 2, progress: 0.99, hostile: true, origin: 1, remaining: vec![3],
            },
            max_org: 60.0, org: 60.0, max_strength: 20.0, strength: 20.0,
            ..Default::default()
        };
        let did = w.add_division(d);
        // 第 1 tick: 到达省2(progress 满)→ 占领 → 续走设 dest=3
        advance_movement(&mut w);
        let div = w.divisions.get(&did).unwrap();
        assert_eq!(div.location_province, 2, "占领省2 后 location 应更新为 2");
        assert!(div.is_moving(), "应续走(仍是 Moving)");
        assert_eq!(div.move_dest(), Some(3), "dest 应切到省3");
        assert_eq!(div.move_progress(), 0.0, "续走进度归零");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --lib movement::tests::t_multihop_continues_after_mid_capture`
Expected: FAIL(占领省2 后转 Idle,不会续走)

- [ ] **Step 3: Arrival 结构加 remaining 字段**

修改 `src/combat/movement.rs` 的 `Arrival` 结构(约 101 行):
```rust
    struct Arrival { id: u64, dest: u32, owner: String, remaining: Vec<u32> }
```

- [ ] **Step 4: 第一阶段收集时携带 remaining**

修改 advance_movement 第一阶段(约 150-156 行),从 Moving 取出 remaining:
```rust
                // 取出 dest + owner + 是否撤退, 把状态置为 Idle(后续第二阶段根据判定改写)
                let (dest, owner, was_retreat, remaining) = match d.order {
                    OrderState::Moving { dest, ref remaining, .. } =>
                        (dest, d.owner_tag.clone(), false, remaining.clone()),
                    OrderState::Retreating { dest, .. } =>
                        (dest, d.owner_tag.clone(), true, Vec::new()),
                    _ => continue,
                };
                d.order = OrderState::Idle; // 临时置 Idle, 第二阶段再决定
                arrived.push((id, dest, owner, was_retreat, remaining));
```

同步修改 `arrived` 的类型声明(约 121 行):
```rust
        let mut arrived: Vec<(u64, u32, String, bool, Vec<u32>)> = Vec::new();
```

- [ ] **Step 5: 第一阶段b 解构加 remaining**

修改第一阶段b 的循环(约 160 行),解构加 remaining 并传入 Capture:
```rust
        for (id, dest, owner, was_retreat, remaining) in arrived {
            let dest_has_battle = world.battles.iter().any(|b| b.province == dest);
            let has_enemies = world.divisions.values()
                .any(|od| od.location_province == dest && od.owner_tag != owner
                    && !od.is_annihilated());
            if was_retreat {
                if has_enemies || dest_has_battle {
                    decisions.push(ArrivalDecision::RetreatIntoEnemy { id, dest });
                } else {
                    decisions.push(ArrivalDecision::Capture(Arrival { id, dest, owner, remaining }));
                }
            } else {
                if dest_has_battle || has_enemies {
                    decisions.push(ArrivalDecision::Pending { id, dest });
                } else {
                    decisions.push(ArrivalDecision::Capture(Arrival { id, dest, owner, remaining }));
                }
            }
        }
```

> 注:Retreating 分支的 Capture 传 `remaining`(此处为空 Vec),不影响撤退逻辑(撤退后续走逻辑见下)。

- [ ] **Step 6: 第二阶段 Capture 分支加续走逻辑**

修改第二阶段 Capture 应用(约 191-197),占领后检查 remaining 续走:
```rust
            ArrivalDecision::Capture(a) => {
                if let Some(d) = world.divisions.get_mut(&a.id) {
                    d.location_province = a.dest;
                    d.order = OrderState::Idle; // 临时, 下面可能覆盖
                }
                // 【新增决策5】检查路径剩余: remaining 非空 → 续走下一段
                if !a.remaining.is_empty() {
                    let next = a.remaining[0];
                    let new_remaining = a.remaining[1..].to_vec();
                    if let Some(d) = world.divisions.get_mut(&a.id) {
                        let hostile = world.provinces.get(&next)
                            .map(|p| p.controller != a.owner)
                            .unwrap_or(false);
                        d.order = OrderState::Moving {
                            dest: next, progress: 0.0,
                            hostile, origin: a.dest, // 出发地 = 刚占领的省
                            remaining: new_remaining,
                        };
                        // 续走时不在此开战, 交给下一 tick 的 check_engagements
                    }
                }
                arrivals.push(a);
            }
```

> 注意:`arrivals.push(a)` 仍要执行(第三阶段占领结算用 a.dest/a.owner)。但若续走了,第三阶段的 org 损只对 a.dest(中途省)生效,符合预期。

- [ ] **Step 7: 跑测试确认通过**

Run: `cargo test --lib movement::tests::t_multihop_continues_after_mid_capture`
Expected: PASS

- [ ] **Step 8: 跑全部测试确认无回归**

Run: `cargo test`
Expected: 全部通过。重点验证 t_division_moves_to_destination(单段,remaining=[],不续走)、t_conquering_loses_org。

- [ ] **Step 9: Commit**

```bash
git add src/combat/movement.rs
git commit -m "feat(land): 到达中途省后续走(决策5) — Capture 后检查 remaining 设下一段"
```

---

## Task 5: Pending 战斗胜利后续走(决策5补全)

**Files:**
- Modify: `src/combat/movement.rs`(advance_movement 第四阶段)

> Pending 的师战斗胜利占领 dest 后,也要检查 remaining 续走。这是续走的第二处点。

- [ ] **Step 1: 写失败测试(Pending 续走)**

在 `src/combat/movement.rs` tests 模块追加:
```rust
    #[test]
    fn t_multihop_pending_resolves_then_continues() {
        // 师在省1, Moving dest=2(remaining=[3]), 省2 有 FRA 敌军 → 到达变 Pending
        // 战斗结束后(敌人消失)→ 占领省2 → 续走省3
        let mut w = chain_1_2_3_world();
        // 把省3 也设成 FRA(让 dest=2 hostile), 省2 放一个 FRA 师
        w.provinces.get_mut(&3).unwrap().controller = "FRA".into();
        w.provinces.get_mut(&3).unwrap().owner = "FRA".into();
        // FRA 守军在省2
        let enemy = Division {
            id: 0, owner_tag: "FRA".into(), location_province: 2,
            max_strength: 20.0, strength: 20.0, max_org: 60.0, org: 0.0, // org 归零会被清出战斗
            ..Default::default()
        };
        w.add_division(enemy);
        // GER 师 Moving dest=2 remaining=[3]
        let d = Division {
            id: 0, owner_tag: "GER".into(), location_province: 1,
            order: OrderState::Moving {
                dest: 2, progress: 0.99, hostile: true, origin: 1, remaining: vec![3],
            },
            max_org: 60.0, org: 60.0, max_strength: 20.0, strength: 20.0,
            ..Default::default()
        };
        let did = w.add_division(d);
        // 到达省2 → Pending(省2 有敌人)
        advance_movement(&mut w);
        let div = w.divisions.get(&did).unwrap();
        assert!(div.is_pending(), "省2 有敌军应进 Pending, order={:?}", div.order);
        // 模拟战斗结束: 移除 FRA 师, 省2 无战斗无敌人
        let enemy_id = w.divisions.values().find(|d| d.owner_tag == "FRA").map(|d| d.id).unwrap();
        w.divisions.remove(&enemy_id);
        // 再 tick → Pending 结算(占领省2)→ 续走省3
        advance_movement(&mut w);
        let div = w.divisions.get(&did).unwrap();
        assert!(div.is_moving(), "占领省2 后应续走省3");
        assert_eq!(div.move_dest(), Some(3));
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --lib movement::tests::t_multihop_pending_resolves_then_continues`
Expected: FAIL(Pending 结算后转 Idle,不续走)

- [ ] **Step 3: Pending 结算时携带 remaining 并续走**

Pending 的师没有 remaining 字段(Pending 变体只有 dest)。需要让 Pending 能记住 remaining。**方案**:Pending 变体加 remaining 字段。

先改 `src/runtime/entities.rs` 的 Pending 变体:
```rust
    /// 到达目标但战斗未胜, 等战斗胜利才结算归属。remaining=战斗胜后续走的剩余路径
    Pending { dest: u32, remaining: Vec<u32> },
```

- [ ] **Step 4: 补所有 Pending 构造点**

修改 `src/combat/movement.rs` 两处 Pending 构造(约 203 行 ArrivalDecision::Pending 应用,约 211 行 RetreatIntoEnemy):
```rust
            ArrivalDecision::Pending { id, dest } => {
                // 从 arrived 的 remaining 传入 — 但此处 decisions 已丢失 remaining。
                // 解决: ArrivalDecision::Pending 也带 remaining
            }
```

**改 ArrivalDecision::Pending 变体携带 remaining**(约 102-108 行):
```rust
    enum ArrivalDecision {
        Capture(Arrival),
        // Moving 组: 进入 Pending 等战斗, remaining 保留供战斗胜后续走
        Pending { id: u64, dest: u32, remaining: Vec<u32> },
        RetreatIntoEnemy { id: u64, dest: u32 },
    }
```

第一阶段b 构造 Pending 时传 remaining(约 180 行):
```rust
                if dest_has_battle || has_enemies {
                    decisions.push(ArrivalDecision::Pending { id, dest, remaining });
                } else {
                    decisions.push(ArrivalDecision::Capture(Arrival { id, dest, owner, remaining }));
                }
```

第二阶段应用 Pending(约 198-204):
```rust
            ArrivalDecision::Pending { id, dest, remaining } => {
                if let Some(d) = world.divisions.get_mut(&id) {
                    d.order = OrderState::Pending { dest, remaining };
                }
            }
```

RetreatIntoEnemy(约 206-214)保持用空 remaining(撤退师无多段):
```rust
            ArrivalDecision::RetreatIntoEnemy { id, dest } => {
                if let Some(d) = world.divisions.get_mut(&id) {
                    d.location_province = dest;
                    d.order = OrderState::Pending { dest, remaining: vec![] };
                }
            }
```

- [ ] **Step 5: 第四阶段 Pending 结算后续走**

修改 advance_movement 第四阶段(约 263-275),Pending 占领后检查 remaining:
```rust
        if !is_own {
            if let Some(p) = world.provinces.get_mut(&dest) {
                p.controller = owner.clone();
                p.owner = owner;
            }
            if let Some(d) = world.divisions.get_mut(&id) {
                d.org = (d.org - d.max_org * ORG_LOSS_ON_CONQUER).max(0.0);
            }
        }
        // 【新增决策5】Pending 结算后, 检查 remaining 续走
        // 重新取师(上面 get_mut 借用已释放),读 remaining
        let remaining_left: Vec<u32> = match world.divisions.get(&id) {
            Some(d) => match d.order {
                OrderState::Pending { ref remaining, .. } => remaining.clone(),
                _ => Vec::new(),
            },
            None => Vec::new(),
        };
        if !remaining_left.is_empty() {
            let next = remaining_left[0];
            let new_remaining = remaining_left[1..].to_vec();
            if let Some(d) = world.divisions.get_mut(&id) {
                let hostile = world.provinces.get(&next)
                    .map(|p| p.controller != owner)
                    .unwrap_or(false);
                d.order = OrderState::Moving {
                    dest: next, progress: 0.0, hostile,
                    origin: dest, remaining: new_remaining,
                };
            }
        } else {
            // 无剩余路径, 上面已设 Idle + location=dest
        }
```

> 注意:第四阶段开头(约 263)已 `d.order = OrderState::Idle; d.location_province = dest;`,所以无 remaining 时师已正确 Idle 在 dest。有 remaining 时覆盖为 Moving。

- [ ] **Step 6: 补 pending_dest() 等访问方法 + wasm_api 序列化**

`src/runtime/entities.rs` 的 `pending_dest()`(约 129)仍只返回 dest,不受影响。

但 `src/wasm_api.rs:285` 序列化时用了模式匹配 `OrderState::Pending { dest }`,加 remaining 字段后此 match 会编译失败。必须同步改:
```rust
            OrderState::Pending { dest, .. } => (0, *dest, 0.0, 0, false, false),
```
(加 `.. ` 忽略 remaining,前端只读 dest,零改动 ✅)

- [ ] **Step 7: 编译并跑测试**

Run: `cargo test --lib movement`
Expected: 所有 movement 测试 PASS(含新的 Pending 续走测试 + 原 4 个基础测试)

- [ ] **Step 8: 跑全部测试**

Run: `cargo test`
Expected: 全部通过

- [ ] **Step 9: Commit**

```bash
git add src/runtime/entities.rs src/combat/movement.rs
git commit -m "feat(land): Pending 战斗胜利后续走(决策5补全) — Pending 加 remaining 字段"
```

---

## Task 6: support_attack 邻接收敛(决策13)

**Files:**
- Modify: `src/combat/commands.rs`(support_attack 函数,约 302-321)
- Test: `tests/battle.rs`

- [ ] **Step 1: 写失败测试(不相邻被忽略)**

在 `tests/battle.rs` 支援攻击测试区(约 919 行附近)追加:
```rust

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
            create_division = { owner = GER location = 10 soft_attack = 10 defense = 10 max_org = 60 }
            create_division = { owner = FRA location = 1 soft_attack = 10 defense = 10 max_org = 60 }
            start_battle = { attacker = GER defender = FRA province = 1 }
        }
    "#);
    let ger_id = world.divisions.values().find(|d| d.owner_tag == "GER").map(|d| d.id).unwrap();
    // GER 师在省10, 支援省1(相邻) — 应成功(回归, 确保邻接检查不误伤)
    run_cmd(&mut world, &interp, &format!("support_attack = {{ division = {ger_id} target = 1 }}"));
    assert!(world.divisions.get(&ger_id).unwrap().is_supporting(), "相邻省支援应成功");

    // 先停止, 重置
    run_cmd(&mut world, &interp, &format!("stop_order = {{ division = {ger_id} }}"));

    // 再加一个省 30(neighbors: 空, 与省10 不相邻)
    world.provinces.insert(30, hoi4_clone::runtime::Province {
        id: 30, owner: "FRA".into(), controller: "FRA".into(),
        terrain: "plains".into(), neighbors: vec![],
    });
    // 支援省30(与省10 不相邻) — 应静默无效
    run_cmd(&mut world, &interp, &format!("support_attack = {{ division = {ger_id} target = 30 }}"));
    assert!(!world.divisions.get(&ger_id).unwrap().is_supporting(), "不相邻省支援应无效");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test support_attack_invalid_when_non_adjacent`
Expected: FAIL(现在不检查邻接,不相邻也设 Supporting)

- [ ] **Step 3: support_attack 加邻接检查**

修改 `src/combat/commands.rs` 的 support_attack(约 302-321),在"无战斗检查"前加邻接检查:
```rust
    reg.register("support_attack", |w, p| {
        let div_id = num_of(np(p, "support_attack", "division")?)? as u64;
        let target = num_of(np(p, "support_attack", "target")?)? as u32;
        // 【决策13】邻接检查: 目标省须与师 location 相邻, 否则静默无效
        let cur_loc = match w.divisions.get(&div_id) {
            Some(d) => d.location_province,
            None => return Ok(()),
        };
        let adjacent = w.provinces.get(&cur_loc)
            .map(|p| p.neighbors.contains(&target))
            .unwrap_or(false);
        if !adjacent {
            return Ok(()); // 不相邻, 静默无效
        }
        // 检查目标省是否已有战斗
        let has_battle = w.battles.iter().any(|b| b.province == target);
        if !has_battle {
            return Ok(());
        }
        let enemies: Vec<u64> = Vec::new();
        if let Some(d) = w.divisions.get_mut(&div_id) {
            d.order = OrderState::Supporting { target };
        }
        join_as_attacker(w, div_id, target, &enemies);
        Ok(())
    });
```

- [ ] **Step 4: 跑测试确认通过 + 回归**

Run: `cargo test support_attack`
Expected: 全部支援攻击测试 PASS(含新的不相邻测试 + 原有 `support_attack_joins_existing_battle_without_moving` 回归)

- [ ] **Step 5: Commit**

```bash
git add src/combat/commands.rs tests/battle.rs
git commit -m "feat(land): support_attack 邻接收敛(决策13) — 只能相邻省发起支援攻击"
```

---

## Task 7: queue_move 航点追加命令(决策8+9+10)

**Files:**
- Modify: `src/combat/commands.rs`(新增 queue_move 注册)
- Test: `tests/battle.rs`

- [ ] **Step 1: 写失败测试(追加航点)**

在 `tests/battle.rs` 多段测试区追加:
```rust

#[test]
fn t_queue_move_appends_waypoint() {
    // 决策9/10: queue_move 追加目标到路径末尾
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = chain_world_owned(); // 1-2-3 全 GER
    // 扩展到 4 省: 1-2-3-4
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
    // 再 queue_move 到省4(追加: 3→4)
    run_cmd(&mut world, &interp, &format!("queue_move = {{ division = {did} target = 4 }}"));
    // 推进足够长 → 应到达省4(经 1→2→3→4)
    GameClock::advance(&interp, &mut world, 90);
    let div = world.divisions.get(&did).unwrap();
    assert_eq!(div.location_province, 4, "应到达追加的航点省4");
    assert!(div.is_idle());
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test t_queue_move_appends_waypoint`
Expected: FAIL(queue_move 命令未注册,报错或无效果)

- [ ] **Step 3: 实现 queue_move 命令**

在 `src/combat/commands.rs` 的 register 函数内,stop_order 之后追加 queue_move 注册:
```rust
    // 航点追加: 把目标追加到当前路径末尾(多段长程规划, 手机端友好无需 shift)。
    // - 当前 Moving: 从路径末尾寻路到 target, 拼接到 remaining
    // - 当前 Idle: 等同 move_division(从头寻路)
    // - Pending/Retreating/Supporting: 忽略(决策11/4.4)
    reg.register("queue_move", |w, p| {
        let div_id = num_of(np(p, "queue_move", "division")?)? as u64;
        let target = num_of(np(p, "queue_move", "target")?)? as u32;
        // 读当前状态(释放借用)
        let (cur_loc, owner) = match w.divisions.get(&div_id) {
            Some(d) => (d.location_province, d.owner_tag.clone()),
            None => return Ok(()),
        };
        // 边界C: 同省忽略
        if target == cur_loc {
            return Ok(());
        }
        // 读当前 order 决定追加还是新建
        let order_snapshot = w.divisions.get(&div_id).map(|d| d.order.clone());
        match order_snapshot {
            Some(OrderState::Moving { dest, ref remaining, .. }) => {
                // 路径末尾 = remaining 最后一个, 或 dest(remaining 空时)
                let end_prov = remaining.last().copied().unwrap_or(dest);
                if end_prov == target {
                    return Ok(()); // 追加的就是当前末尾, 无意义
                }
                // 从末尾寻路到 target
                let seg = match crate::combat::pathfinding::find_path(w, end_prov, target) {
                    Some(s) => s,
                    None => return Ok(()), // 不连通, 忽略
                };
                // 拼接到 remaining
                if let Some(d) = w.divisions.get_mut(&div_id) {
                    if let OrderState::Moving { ref mut remaining, .. } = d.order {
                        remaining.extend(seg);
                    }
                }
                Ok(())
            }
            Some(OrderState::Idle) => {
                // 等同 move_division: 从 cur_loc 寻路
                let path = match crate::combat::pathfinding::find_path(w, cur_loc, target) {
                    Some(p) => p,
                    None => return Ok(()),
                };
                let first = path[0];
                let remaining: Vec<u32> = path[1..].to_vec();
                let hostile = w.provinces.get(&first)
                    .map(|p| p.controller != owner).unwrap_or(false);
                if let Some(d) = w.divisions.get_mut(&div_id) {
                    d.order = OrderState::Moving {
                        dest: first, progress: 0.0, hostile, origin: cur_loc, remaining,
                    };
                }
                // 第一站有敌军 → 开战
                let first_enemies: Vec<u64> = w.divisions.values()
                    .filter(|d| d.location_province == first && d.owner_tag != owner && !d.is_withdrawing())
                    .map(|d| d.id).collect();
                if !first_enemies.is_empty() {
                    join_as_attacker(w, div_id, first, &first_enemies);
                }
                Ok(())
            }
            // Pending/Retreating/Supporting → 忽略
            _ => Ok(()),
        }
    });
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test t_queue_move_appends_waypoint`
Expected: PASS

- [ ] **Step 5: 跑全部测试**

Run: `cargo test`
Expected: 全部通过

- [ ] **Step 6: Commit**

```bash
git add src/combat/commands.rs tests/battle.rs
git commit -m "feat(land): queue_move 航点追加命令(决策9) — 多段长程规划, 手机端友好"
```

---

## Task 8: 路径失效应对(决策14)

**Files:**
- Modify: `src/combat/movement.rs`(advance_movement 开头加 dest 检查 + 新增 invalidate_paths_to_inaccessible)
- Test: `src/combat/pathfinding.rs`(mock is_passable 的测试)

> 决策14:dest 突然不可进入 → 师停止。因 is_passable 现恒 true,需用测试钩子模拟。方案:给 is_passable 加 cfg(test) 的可注入判定。

- [ ] **Step 1: 给 is_passable 加测试钩子**

修改 `src/combat/pathfinding.rs`,让 is_passable 在测试时可被全局开关控制:
```rust
use crate::runtime::World;

#[cfg(test)]
thread_local! {
    /// 测试用: 设为 Some(HashSet) 时, 集合中的省视为不可进入。
    pub(crate) static TEST_BLOCKED: std::cell::RefCell<Option<std::collections::HashSet<u32>>> =
        std::cell::RefCell::new(None);
}

#[cfg(test)]
pub fn set_test_blocked(provs: &[u32]) {
    TEST_BLOCKED.with(|b| *b.borrow_mut() = Some(provs.iter().copied().collect()));
}
#[cfg(test)]
pub fn clear_test_blocked() {
    TEST_BLOCKED.with(|b| *b.borrow_mut() = None);
}

/// 判断一个省能否作为寻路中转。当前恒 true(无投降/停战系统)。
/// 测试时可通过 set_test_blocked 模拟省份不可进入。
pub fn is_passable(_world: &World, prov: u32) -> bool {
    #[cfg(test)]
    {
        if let Some(blocked) = TEST_BLOCKED.with(|b| *b.borrow()) {
            if blocked.contains(&prov) {
                return false;
            }
        }
    }
    let _ = prov;
    true
}
```

> 注:is_passable 改为 pub(供 movement.rs 调用)。find_path 内部调用 is_passable 不变。

- [ ] **Step 2: 写失败测试(dest 不可进入则停止)**

在 `src/combat/pathfinding.rs` tests 模块追加:
```rust
    use crate::combat::movement::advance_movement;
    use crate::runtime::entities::{Division, OrderState};

    #[test]
    fn t_path_stops_when_dest_inaccessible() {
        // 决策14机制1: 多段行军途中, dest 突然不可进入 → 师转 Idle, remaining 清空
        let mut w = chain_world(); // 1-2-3-4
        let d = Division {
            id: 0, owner_tag: "GER".into(), location_province: 1,
            order: OrderState::Moving {
                dest: 2, progress: 0.5, hostile: false, origin: 1, remaining: vec![3],
            },
            ..Default::default()
        };
        let did = w.add_division(d);
        // 设置省2 不可进入(模拟投降/停战)
        super::set_test_blocked(&[2]);
        advance_movement(&mut w);
        super::clear_test_blocked();
        let div = w.divisions.get(&did).unwrap();
        assert!(div.is_idle(), "dest 不可进入应停止, order={:?}", div.order);
        assert_eq!(div.move_dest(), None, "应无 dest");
    }
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test --lib pathfinding::tests::t_path_stops_when_dest_inaccessible`
Expected: FAIL(advance_movement 开头还没加 dest 检查,师照常推进)

- [ ] **Step 4: advance_movement 开头加 dest 可进入性检查**

修改 `src/combat/movement.rs` 的 advance_movement 开头(函数体第一行,约 90 行 `pub fn advance_movement` 之后),加入第 0 步:
```rust
pub fn advance_movement(world: &mut World) {
    // 【决策14机制1】第 0 步: 检查每个 Moving 师的 dest 是否仍可进入。
    // 不可进入(如对方领土投降后) → 师停止(转 Idle, 清 remaining)。
    let moving_now: Vec<u64> = world.divisions.iter()
        .filter_map(|(id, d)| d.is_moving().then_some(*id))
        .collect();
    for id in moving_now {
        let blocked = match world.divisions.get(&id) {
            Some(d) => match d.order {
                OrderState::Moving { dest, .. } =>
                    !crate::combat::pathfinding::is_passable(world, dest),
                _ => false,
            },
            None => false,
        };
        if blocked {
            if let Some(d) = world.divisions.get_mut(&id) {
                d.order = OrderState::Idle; // 清 remaining(Moving→Idle)
            }
        }
    }

    // 收集所有 Moving/Retreating 的师(需要推进进度)
    let moving_ids: Vec<u64> = world
```

> 注意:原 `let moving_ids: Vec<u64> = world.divisions.iter()...` 这一行保持不变,接在上面之后。

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test --lib pathfinding::tests::t_path_stops_when_dest_inaccessible`
Expected: PASS

- [ ] **Step 6: 写 invalidate_paths_to_inaccessible 函数测试**

在 `src/combat/pathfinding.rs` tests 模块追加:
```rust
    #[test]
    fn t_invalidate_paths_clears_blocked() {
        // 决策14机制2: invalidate 扫描 dest+remaining, 任一不可进入则整条路径停止
        let mut w = chain_world();
        let d = Division {
            id: 0, owner_tag: "GER".into(), location_province: 1,
            order: OrderState::Moving {
                dest: 2, progress: 0.3, hostile: false, origin: 1, remaining: vec![3, 4],
            },
            ..Default::default()
        };
        let did = w.add_division(d);
        // 省4 不可进入(remaining 里的中转省)
        super::set_test_blocked(&[4]);
        crate::combat::movement::invalidate_paths_to_inaccessible(&mut w);
        super::clear_test_blocked();
        assert!(w.divisions.get(&did).unwrap().is_idle(), "remaining 含不可进入省应整条停止");
    }
```

- [ ] **Step 7: 实现 invalidate_paths_to_inaccessible**

在 `src/combat/movement.rs` 追加函数(放在 advance_movement 之后):
```rust
/// 强制中止所有路径涉及不可进入省的师(转 Idle, 清 remaining)。
/// 供未来投降/停战/领土移交事件批量调用 —— 原版"强制中止敌对行为"的等价。
/// 与 advance_movement 第 0 步的区别: 本函数扫整条路径(dest+remaining), 事件触发时一次性清场。
pub fn invalidate_paths_to_inaccessible(world: &mut World) {
    let moving_ids: Vec<u64> = world.divisions.iter()
        .filter_map(|(id, d)| d.is_moving().then_some(*id))
        .collect();
    for id in moving_ids {
        let blocked = match world.divisions.get(&id) {
            Some(d) => match &d.order {
                OrderState::Moving { dest, remaining, .. } =>
                    !crate::combat::pathfinding::is_passable(world, *dest)
                    || remaining.iter().any(|&p| !crate::combat::pathfinding::is_passable(world, p)),
                _ => false,
            },
            None => false,
        };
        if blocked {
            if let Some(d) = world.divisions.get_mut(&id) {
                d.order = OrderState::Idle;
            }
        }
    }
}
```

- [ ] **Step 8: 跑测试 + 全部测试**

Run: `cargo test`
Expected: 全部通过

- [ ] **Step 9: Commit**

```bash
git add src/combat/pathfinding.rs src/combat/movement.rs
git commit -m "feat(land): 路径失效应对(决策14) — dest不可进入则停止 + invalidate强制中止函数"
```

---

## Task 9: WASM 桥接 engine_queue_move(决策9)

**Files:**
- Modify: `src/wasm_api.rs`(新增 engine_queue_move)

- [ ] **Step 1: 新增 engine_queue_move FFI**

在 `src/wasm_api.rs` 的 engine_support_attack 之后(约 114 行后)追加:
```rust
/// 追加航点到师的行军路径(前端航点规划用, 手机端友好无需 shift)
#[no_mangle]
pub extern "C" fn engine_queue_move(division_id: u32, target: u32) {
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Engine { interp, world } = &mut *e;
        let script = format!("queue_move = {{ division = {division_id} target = {target} }}");
        if let Ok(b) = crate::parser::parse(&script) {
            let effs = crate::ast::lower::lower_effects(&b);
            interp.run(&effs, world);
        }
    });
}
```

- [ ] **Step 2: 编译验证(WASM target)**

Run: `cargo build --target wasm32-unknown-unknown --lib --release`
Expected: 编译通过(WASM 产物生成在 target/wasm32-unknown-unknown/release/)

> 注:若该 target 未安装,跳过此步,改用 `cargo build --lib` 确认至少原生编译通过。WASM 编译在最终交付前补做(Task 11)。

- [ ] **Step 3: Commit**

```bash
git add src/wasm_api.rs
git commit -m "feat(wasm): engine_queue_move FFI(航点规划前端接口)"
```

---

## Task 10: 边界测试补全 + 全量回归(决策11/12)

**Files:**
- Modify: `tests/battle.rs`

- [ ] **Step 1: 写边界测试**

在 `tests/battle.rs` 多段测试区追加:
```rust

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
    // 下令去自己所在的省2
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {did} target = 2 }}"));
    let div = world.divisions.get(&did).unwrap();
    assert!(div.is_idle(), "同省命令应忽略, 保持 Idle");
}

#[test]
fn t_move_during_pending_ignored() {
    // 决策11: Pending 时移动命令被忽略
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = chain_world_owned();
    run_setup(&mut world, &interp, r#"
        _setup = { create_division = { owner = GER location = 2 soft_attack = 10 defense = 10 max_org = 60 } }
    "#);
    let did = *world.divisions.keys().next().unwrap();
    // 手动设为 Pending
    world.divisions.get_mut(&did).unwrap().order = hoi4_clone::runtime::entities::OrderState::Pending {
        dest: 3, remaining: vec![],
    };
    // 下令移动到省1 — 应被忽略(Pending 不可中断)
    run_cmd(&mut world, &interp, &format!("move_division = {{ division = {did} target = 1 }}"));
    assert!(world.divisions.get(&did).unwrap().is_pending(), "Pending 时命令应忽略");
}

#[test]
fn t_find_path_no_route_ignored() {
    // 寻路失败(不连通)→ 师不动
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = chain_world_owned();
    // 加一个孤立省 99
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
```

- [ ] **Step 2: 跑全部测试**

Run: `cargo test`
Expected: 全部通过(新增 3 个边界测试 + 原有全部)

- [ ] **Step 3: Commit**

```bash
git add tests/battle.rs
git commit -m "test(land): 多段行军边界测试补全(决策11/12 — 同省/Pending/不连通忽略)"
```

---

## Task 11: 最终验证 + WASM 重编译 + 文档更新

**Files:**
- Modify: `docs/HANDOFF.md`(更新里程碑表 + 本阶段提交)

- [ ] **Step 1: 全量测试**

Run: `cargo test`
Expected: 全部通过(原 101 + 新增约 15 个 = ~116 个测试)

- [ ] **Step 2: WASM 重编译**

Run:
```bash
cargo build --target wasm32-unknown-unknown --lib --release
cp target/wasm32-unknown-unknown/release/hoi4_clone.wasm web/
```
Expected: 编译成功,web/hoi4_clone.wasm 更新

- [ ] **Step 3: 更新 HANDOFF.md**

修改 `docs/HANDOFF.md`:
- §1 里程碑表加一行(多段路径行军 + 航点 + 失效应对)
- §5 未实现规则表移除"多段路径行军(寻路)"
- §8 加本阶段提交列表
- 顶部更新日期改为 2026-06-23

- [ ] **Step 4: Commit**

```bash
git add docs/HANDOFF.md web/hoi4_clone.wasm
git commit -m "docs: 阶段总结更新 — 多段路径行军 + 航点规划 + 支援攻击邻接收敛 + 路径失效应对"
```

---

## 完成标准

- [ ] `cargo test` 全部通过(~116 测试)
- [ ] WASM 重编译成功,web/hoi4_clone.wasm 更新
- [ ] HANDOFF.md 更新
- [ ] 所有提交在 multi-hop-marching 分支

## 决策覆盖检查(实现完成后核对)

| 决策 | 实现任务 |
|---|---|
| 1 寻路 BFS + is_passable | Task 2 |
| 2 带权框架(权重1) | Task 2(edge_weight 留注释,见 spec §3.4) |
| 3 Moving 加 remaining | Task 1 |
| 4 remaining 不含 dest | Task 1/2/3 |
| 5 中途段占领续走 | Task 4(Moving)+ Task 5(Pending) |
| 6 中途遇敌复用规则 | 自动(无需新代码) |
| 7 战败/停止路径取消 | 自动(Moving→Idle/Retreating) |
| 8 航点架构 Y | Task 7 |
| 9 queue_move 命令 | Task 7 |
| 10 move 覆盖/queue 追加 | Task 3(move)+ Task 7(queue) |
| 11 战斗中命令忽略 | Task 3 + Task 10 |
| 12 同省命令忽略 | Task 3 + Task 10 |
| 13 支援攻击邻接收敛 | Task 6 |
| 14 路径失效应对 | Task 8 |
