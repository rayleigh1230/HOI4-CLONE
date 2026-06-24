# State 概念(省份上级容器) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 引入 State 作为 Province 的上级容器, 成为归属/建筑/人力的唯一权威源。Province 归属彻底从 State 派生(删 owner/controller, 加 state_id)。为补给/生产/占领系统预留接入点。

**Architecture:** World 加 `states: HashMap<u32, State>`(可变运行时状态, 不进 GameData)。Province 删 owner/controller 加 state_id, 归属通过 World 的派生查询读取。占领改 State.controller(不改 owner, 区分法理 vs 控制)。新增 create_state 命令 + state_loader。

**Tech Stack:** Rust 2021, 纯标准库, 现有 Province/World/commands/resolve/movement/wasm_api。

**关联文档:**
- 设计 spec: `docs/superpowers/specs/2026-06-24-state-concept-design.md`
- 设计原则: `docs/design-principles.md`
- 项目现状: `docs/HANDOFF.md`

---

## 文件结构

```
src/
├── runtime/
│   └── entities.rs      ← 改: Province 删 owner/controller 加 state_id; 新增 State 结构
├── runtime/world.rs     ← 改: 加 states + 派生查询 + friendly_neighbor 改派生
├── combat/
│   ├── commands.rs      ← 改: create_state 新增; create_province 删 owner 加 state; 读 controller 改派生
│   ├── resolve.rs       ← 改: 占领改 set_state_controller; 测试迁移
│   ├── movement.rs      ← 改: 读 controller 改派生; 占领改 set_state_controller; 测试迁移
│   ├── recovery.rs      ← 改: 读 controller 改派生
│   └── pathfinding.rs   ← 改: 注释更新
├── data/
│   ├── state_loader.rs  ← 新增: load_states + parse_state_block
│   └── mod.rs           ← 改: 声明 state_loader
├── wasm_api.rs          ← 改: 序列化读派生; set_controller 改 set_state_controller
├── lib.rs               ← 不动
tests/
├── battle.rs            ← 改: 建省脚本迁移 + 读 controller 改派生
├── teleport_bug.rs      ← 改: 建省脚本迁移 + 读 controller 改派生
├── scope.rs             ← 改: 建省脚本迁移
└── integration.rs       ← 改: 建省脚本迁移
src/bin/diag.rs          ← 改: 建省脚本迁移
```

### 改动清单

| 文件 | 改动 | Task |
|---|---|---|
| `runtime/entities.rs` | Province 结构改 + State 新增 | Task 1 |
| `runtime/world.rs` | states + 派生查询 + friendly_neighbor | Task 2 |
| `combat/commands.rs` | create_state + create_province 改 + 读 controller 派生 | Task 3 |
| `combat/resolve.rs` | 占领改 set_state_controller + 测试迁移 | Task 4 |
| `combat/movement.rs` | 读 controller 派生 + 占领改 + 测试迁移 | Task 5 |
| `combat/recovery.rs` | 读 controller 派生 | Task 6 |
| `wasm_api.rs` | 序列化读派生 + set_controller | Task 7 |
| `data/state_loader.rs` + `data/mod.rs` | load_states | Task 8 |
| 测试文件全量迁移 | battle/teleport/scope/integration/diag | Task 9 |
| 端到端 + 验收 | | Task 10 |

### 任务依赖

```
Task 1 (结构) → Task 2 (派生查询) → Task 3 (命令) → Task 4-7 (调用点迁移, 可并行) 
                                                              ↓
                                              Task 8 (loader) → Task 9 (测试迁移) → Task 10 (验收)
```

Task 1-2 改结构(Task 2 后编译失败, 调用点未改); Task 3-7 逐个修调用点(每修一个, 该文件编译过); Task 8 独立; Task 9 测试脚本迁移; Task 10 验收。

**关键**: Task 1+2 合并执行(结构改完一起), 之后 cargo build 会大面积失败(预期), Task 3-9 逐个修复。

---

## Task 1: Province 结构改 + State 结构定义

改 Province 删 owner/controller 加 state_id, 新增 State 结构。这是地基, 改完后全项目编译会失败(调用点未改), 由后续 Task 修复。

**Files:**
- Modify: `src/runtime/entities.rs`

- [ ] **Step 1: Province 改结构**

在 `src/runtime/entities.rs`, 把 Province 改成:

```rust
//! 游戏实体结构(M3)
#[derive(Debug, Clone, Default)]
pub struct Province {
    pub id: u32,
    /// 所属 State id(归属从 State 派生, Province 不再存 owner/controller)
    pub state_id: u32,
    pub terrain: String,
    /// 邻接省份 id 列表(行军/战斗的基础设施)
    pub neighbors: Vec<u32>,
}
```

(删除原来的 `owner` 和 `controller` 字段)

- [ ] **Step 2: 新增 State 结构**

在 `src/runtime/entities.rs` 的 Province 之后加:

```rust
use std::collections::HashMap;

/// 州/地区(Province 的上级容器, 归属/建筑/人力的唯一权威源)
/// 可变运行时状态(进 World, 不进 GameData)
/// 设计见 docs/superpowers/specs/2026-06-24-state-concept-design.md
#[derive(Debug, Clone, Default)]
pub struct State {
    pub id: u32,
    pub name: String,              // "STATE_1"(本地化 key)
    pub owner: String,             // 法理归属(谁拥有这片领土)
    pub controller: String,        // 实际控制(可能被占领, ≠ owner)
    pub manpower: f64,             // 人力(征兵来源)
    pub state_category: String,    // "town"/"city"/"megalopolis"(决定建筑槽位)
    pub cores: Vec<String>,        // 核心国 tag(谁有合法领土声索)
    pub buildings: HashMap<String, f64>,  // 建筑占位映射(后续建筑系统升级)
    pub provinces: Vec<u32>,       // 这个 State 包含哪些省份(正向映射)
}
```

注意: `use std::collections::HashMap;` 若文件顶部已有则不重复。

- [ ] **Step 3: 确认编译失败(预期)**

Run: `cargo build 2>&1 | grep "error\[E0560\]\|error\[E0063\]" | head`
Expected: 多个 "no field `controller`" / "no field `owner`" 错误(Province 字段删了, 调用点未改)。这是预期, Task 2-9 修复。

- [ ] **Step 4: 不单独提交(与 Task 2 合并)**

Task 1+2 一起提交(结构改完 + 派生查询就位, 虽然调用点还没改)。

---

## Task 2: World 加 states + 派生查询

给 World 加 State 存储和归属派生查询方法。改完与 Task 1 一起提交。

**Files:**
- Modify: `src/runtime/entities.rs`(re-export State)
- Modify: `src/runtime/world.rs`

- [ ] **Step 1: entities.rs re-export State**

在 `src/runtime/entities.rs` 末尾确认 Scope 之后无其它内容, State 已定义。在 `src/runtime/mod.rs` 的 re-export 加 State:

```rust
pub use entities::{Battle, Country, Division, Province, Scope, State};
```

- [ ] **Step 2: World 加 states 字段**

在 `src/runtime/world.rs` 的 World 结构体加字段(在 provinces 之后):

```rust
    // M3 实体存储
    pub provinces: HashMap<u32, Province>,
    pub states: HashMap<u32, State>,   // ★新增: 州(归属/建筑/人力权威源)
    pub countries: HashMap<String, Country>,
```

在 Default impl 加:

```rust
            provinces: Default::default(),
            states: Default::default(),
            countries: Default::default(),
```

在 `use crate::runtime::entities::{...}` 加 State:

```rust
use crate::runtime::entities::{Battle, Country, Division, Province, Scope, State};
```

- [ ] **Step 3: 加派生查询方法**

在 `src/runtime/world.rs` 的 `impl World` 内(friendly_neighbor 之前)加:

```rust
    // ===== State 派生查询(Province 归属从 State 派生) =====

    /// 省份 → 所属 State id
    pub fn province_state(&self, province_id: u32) -> Option<u32> {
        self.provinces.get(&province_id).map(|p| p.state_id)
    }

    /// 省份的实际控制者(从 State 派生; 找不到返回 None → 中立)
    pub fn province_controller(&self, province_id: u32) -> Option<&str> {
        let sid = self.province_state(province_id)?;
        self.states.get(&sid).map(|s| s.controller.as_str())
    }

    /// 省份的法理归属者(从 State 派生)
    pub fn province_owner(&self, province_id: u32) -> Option<&str> {
        let sid = self.province_state(province_id)?;
        self.states.get(&sid).map(|s| s.owner.as_str())
    }

    /// 设置省份的实际控制者(改所属 State 的 controller; 省份自动跟随)
    /// 占领用: 只改 controller, 不改 owner(法理归属不变)
    pub fn set_state_controller(&mut self, province_id: u32, new_controller: &str) {
        if let Some(sid) = self.province_state(province_id) {
            if let Some(state) = self.states.get_mut(&sid) {
                state.controller = new_controller.into();
            }
        }
    }
```

- [ ] **Step 4: friendly_neighbor 改派生**

把 friendly_neighbor 改成读派生:

```rust
    /// 找某省的邻接己方省(撤退目标)。无则返回 None(被包围)
    pub fn friendly_neighbor(&self, province: u32, tag: &str) -> Option<u32> {
        let prov = self.provinces.get(&province)?;
        prov.neighbors.iter().copied().find(|n| {
            self.province_controller(*n).map(|c| c == tag).unwrap_or(false)
        })
    }
```

- [ ] **Step 5: 修复 world.rs 内联测试(t_add_division 等用 Province 的地方)**

world.rs 的 tests 里构造 Province 的地方(如 t_m2_fields_default), 删 owner/controller 加 state_id。搜索 `Province {` in world.rs tests。

- [ ] **Step 6: 提交 Task 1+2**

```bash
git add src/runtime/entities.rs src/runtime/mod.rs src/runtime/world.rs
git commit -m "feat(state): Province 删owner/controller加state_id + State结构 + World派生查询"
```

> 注: 此时 cargo build 仍失败(combat 各文件调用点未改), Task 3-7 修复。

---

## Task 3: commands.rs — create_state + create_province 改造 + 读 controller 派生

**Files:**
- Modify: `src/combat/commands.rs`

- [ ] **Step 1: 新增 create_state 命令**

在 `src/combat/commands.rs` 的 register 函数内(create_province 之前)加:

```rust
    // 建州(归属/建筑/人力的权威源)
    reg.register("create_state", |w, p| {
        let id = num_of(np(p, "create_state", "id")?)? as u32;
        let name = ParamGet::get(p, "name").and_then(Arg::as_str).unwrap_or("").to_string();
        let owner = np(p, "create_state", "owner")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?;
        let controller = ParamGet::get(p, "controller").and_then(Arg::as_str).unwrap_or(owner).to_string();
        let manpower = ParamGet::get(p, "manpower").and_then(Arg::as_num).unwrap_or(0.0);
        let category = ParamGet::get(p, "state_category").and_then(Arg::as_str).unwrap_or("wasteland").to_string();
        // cores = { GER FRA } 裸值列表
        let mut cores = Vec::new();
        if let Some(Arg::Block(fields)) = ParamGet::get(p, "cores") {
            for (_, v) in fields {
                if let Some(s) = v.as_str() { cores.push(s.to_string()); }
            }
        }
        // buildings = { infrastructure = 5 ... } 命名块
        let mut buildings = std::collections::HashMap::new();
        if let Some(Arg::Block(fields)) = ParamGet::get(p, "buildings") {
            for (k, v) in fields {
                if let Some(n) = v.as_num() { buildings.insert(k.clone(), n); }
            }
        }
        w.states.insert(id, crate::runtime::State {
            id, name, owner: owner.into(), controller, manpower,
            state_category: category, cores, buildings,
            provinces: vec![],
        });
        Ok(())
    });
```

- [ ] **Step 2: create_province 改造(删 owner, 加 state)**

把 create_province 命令改成:

```rust
    // 创建省份(行军基础设施: state_id/terrain/neighbors)
    reg.register("create_province", |w, p| {
        let id = num_of(np(p, "create_province", "id")?)? as u32;
        let state_id = num_of(np(p, "create_province", "state")?)? as u32;
        let terrain = ParamGet::get(p, "terrain").and_then(Arg::as_str).unwrap_or("plains");
        let mut neighbors = Vec::new();
        if let Some(Arg::Block(fields)) = ParamGet::get(p, "neighbors") {
            for (_, v) in fields {
                if let Some(n) = v.as_num() { neighbors.push(n as u32); }
            }
        }
        w.provinces.insert(id, crate::runtime::Province {
            id, state_id, terrain: terrain.into(), neighbors,
        });
        // 反向注册: 省 id 加入所属 State 的 provinces 列表
        if let Some(state) = w.states.get_mut(&state_id) {
            state.provinces.push(id);
        }
        Ok(())
    });
```

- [ ] **Step 3: join_as_attacker 读 controller 改派生(307 行)**

```rust
// 改造前(307行附近):
.map(|p| p.controller == owner).unwrap_or(false);
// 改造后:
.map(|_| w.province_controller(world.battles[bidx].province).map(|c| c == owner).unwrap_or(false));
```

> 注意: 这处实际是检查目标省是否己方。用 `w.province_controller(target)` 派生。具体看 307 行上下文, 可能是检查 `first` 省或 target 省。按实际语义改: 把 `p.controller` 换成 `w.province_controller(pid)`。

- [ ] **Step 4: move_division 读 controller 改派生(346, 476 行)**

346 行:
```rust
// 改造前:
let first_controller = w.provinces.get(&first).map(|p| p.controller.as_str()).unwrap_or("");
// 改造后:
let first_controller = w.province_controller(first).unwrap_or("");
```

476 行(queue_move 里):
```rust
// 改造前:
.map(|p| p.controller != owner).unwrap_or(false);
// 改造后:
w.province_controller(first).map(|c| c != owner).unwrap_or(false);
```

- [ ] **Step 5: 不单独提交(commands.rs 还没全改完, 与其它文件一起或单独)**

可单独提交 commands.rs(若编译过)。检查: `cargo build 2>&1 | grep "commands.rs" | head`。若 commands.rs 自身无错, 提交。

```bash
git add src/combat/commands.rs
git commit -m "feat(state): create_state 命令 + create_province 删owner加state + 读controller派生"
```

---

## Task 4: resolve.rs — 占领改 set_state_controller + 测试迁移

**Files:**
- Modify: `src/combat/resolve.rs`

- [ ] **Step 1: 占领改 set_state_controller(451-452 行)**

找到 province_captures 应用处(cleanup_battles 末尾):

```rust
// 改造前(451行附近):
    for (province, winner) in province_captures {
        if let Some(p) = world.provinces.get_mut(&province) {
            p.controller = winner.clone();
            p.owner = winner;
        }
    }
// 改造后(只改 controller, 不改 owner; 通过 set_state_controller):
    for (province, winner) in province_captures {
        world.set_state_controller(province, &winner);
    }
```

- [ ] **Step 2: 攻方战败归属判定改派生(404 行)**

```rust
// 改造前(404行附近):
let loc_friendly = world.provinces.get(&loc)
    .map(|p| p.controller == owner).unwrap_or(false);
// 改造后:
let loc_friendly = world.province_controller(loc).map(|c| c == owner).unwrap_or(false);
```

- [ ] **Step 3: resolve.rs 测试里读 controller 改派生(882-883 行)**

```rust
// 改造前(测试里设省份归属):
w.provinces.get_mut(&2).unwrap().controller = "GER".into();
w.provinces.get_mut(&2).unwrap().owner = "GER".into();
// 改造后(改 State controller):
w.set_state_controller(2, "GER");
```

注意: 测试里这些省份必须先有所属 State(通过 create_state 建), set_state_controller 才生效。测试建省脚本在 Task 9 统一迁移。

- [ ] **Step 4: 编译检查**

Run: `cargo build 2>&1 | grep "resolve.rs" | head`
Expected: resolve.rs 自身无错(若测试脚本未迁移会有测试编译错, 但 lib 应过)。

- [ ] **Step 5: 提交**

```bash
git add src/combat/resolve.rs
git commit -m "feat(state): resolve 占领改 set_state_controller(只改controller不改owner)"
```

---

## Task 5: movement.rs — 读 controller 派生 + 占领改 + 测试迁移

**Files:**
- Modify: `src/combat/movement.rs`

- [ ] **Step 1: check_engagements 里读 controller 改派生(226, 260 行)**

```rust
// 改造前(226行附近):
.map(|p| p.controller != a.owner)
// 改造后:
// 这处在闭包里读 province, 改成 w.province_controller(pid)
// (需把 province id 先取出, 闭包外调 w.province_controller)
```

具体: 226 行和 260 行在 advance_movement 的到达判定里, 检查目标省 controller。把 `p.controller` 改成调 `world.province_controller(pid)`。由于这些在闭包内, 可能需要重构: 先取 province_id, 闭包外查 controller。

- [ ] **Step 2: Capture 占领改 set_state_controller(264-265 行)**

```rust
// 改造前(264行):
p.controller = a.owner.clone();
p.owner = a.owner;
// 改造后:
world.set_state_controller(province_id, &a.owner);
```

- [ ] **Step 3: RetreatIntoEnemy 占领改(319-320 行)**

```rust
// 改造前(319行):
p.controller = owner.clone();
p.owner = owner;
// 改造后:
world.set_state_controller(province_id, &owner);
```

- [ ] **Step 4: 读 controller 改派生(305, 311 行)**

同 Step 1, 把 `p.controller` 改成 `world.province_controller(pid)`。

- [ ] **Step 5: 测试里读 controller 改派生(451, 590-591 行)**

```rust
// 改造前(451行):
assert_eq!(w.provinces.get(&2).unwrap().controller, "GER", ...);
// 改造后:
assert_eq!(w.province_controller(2).unwrap_or(""), "GER", ...);

// 改造前(590行):
w.provinces.get_mut(&3).unwrap().controller = "FRA".into();
w.provinces.get_mut(&3).unwrap().owner = "FRA".into();
// 改造后:
w.set_state_controller(3, "FRA");
```

- [ ] **Step 6: 编译检查 + 提交**

```bash
cargo build 2>&1 | grep "movement.rs" | head
git add src/combat/movement.rs
git commit -m "feat(state): movement 读controller派生 + 占领改set_state_controller"
```

---

## Task 6: recovery.rs — 读 controller 派生

**Files:**
- Modify: `src/combat/recovery.rs`

- [ ] **Step 1: 读 controller 改派生(36 行)**

```rust
// 改造前(36行附近):
.map(|p| p.controller == div.owner_tag)
// 改造后:
// 这处检查师的归属省 controller。改成 world.province_controller(pid)
```

具体: recovery.rs 在循环里, 用 `world.provinces.get(&loc).map(|p| p.controller == div.owner_tag)`。改成 `world.province_controller(loc).map(|c| c == div.owner_tag).unwrap_or(false)`。

注意借用: recovery 遍历 `world.divisions.iter_mut()`, 同时要读 `world.province_controller`(借 world)。需用快照模式: 先取 controller 字符串再进 iter_mut, 或把检查移出循环。看实际代码结构调整。

- [ ] **Step 2: 编译检查 + 提交**

```bash
cargo build 2>&1 | grep "recovery.rs" | head
git add src/combat/recovery.rs
git commit -m "feat(state): recovery 读controller派生"
```

---

## Task 7: wasm_api.rs — 序列化读派生 + set_controller

**Files:**
- Modify: `src/wasm_api.rs`

- [ ] **Step 1: 序列化省份改读派生(351 行)**

```rust
// 改造前(346-351行):
for p in world.provinces.values() {
    // ...
    s.push_str(&format!(
        "{{\"id\":{},\"controller\":\"{}\",\"neighbors\":[",
        p.id, p.controller
    ));
// 改造后:
for p in world.provinces.values() {
    let controller = world.province_controller(p.id).unwrap_or("");
    let owner = world.province_owner(p.id).unwrap_or("");
    // ...
    s.push_str(&format!(
        "{{\"id\":{},\"controller\":\"{}\",\"owner\":\"{}\",\"neighbors\":[",
        p.id, controller, owner
    ));
```

注意借用: `for p in world.provinces.values()` 借了 world, 不能再调 `world.province_controller`。需先把数据快照出来:

```rust
// 快照模式(避借用冲突):
let prov_data: Vec<(u32, String, String, Vec<u32>)> = world.provinces.values().map(|p| {
    let controller = world.province_controller(p.id).unwrap_or("").to_string();
    let owner = world.province_owner(p.id).unwrap_or("").to_string();
    (p.id, controller, owner, p.neighbors.clone())
}).collect();
for (id, controller, owner, neighbors) in &prov_data {
    // 序列化...
}
```

- [ ] **Step 2: set_province_controller 改 set_state_controller(80-81 行)**

```rust
// 改造前(80行):
p.controller = tag.to_string();
p.owner = tag.to_string();
// 改造后(在调用点改, 不在闭包内直接改 province):
// wasm_api 里有设置省份归属的 FFI, 改成调 world.set_state_controller(pid, tag)
```

具体看 80 行上下文——若是 engine_set_province_controller 之类的 FFI, 改成内部调 `world.set_state_controller(province_id, tag)`。

- [ ] **Step 3: WASM 编译检查**

Run: `cargo build --target wasm32-unknown-unknown --lib --release 2>&1 | tail -3`
Expected: 成功。

- [ ] **Step 4: 提交**

```bash
git add src/wasm_api.rs
git commit -m "feat(state): wasm_api 序列化读派生 + set_state_controller"
```

---

## Task 8: state_loader — 加载真实 State 文件

**Files:**
- Create: `src/data/state_loader.rs`
- Modify: `src/data/mod.rs`
- Create: `src/data_raw/states/`(拷贝真实文件)

- [ ] **Step 1: 拷贝真实 state 文件子集**

```bash
cd /g/projects/hoi4-clone
mkdir -p src/data_raw/states
# 拷贝几个典型 state(德国/法国周边)用于测试加载
cp "/g/steam/steamapps/common/Hearts of Iron IV/history/states/1-France.txt" src/data_raw/states/
cp "/g/steam/steamapps/common/Hearts of Iron IV/history/states/64-Northern France.txt" src/data_raw/states/ 2>/dev/null || true
ls src/data_raw/states/
```

- [ ] **Step 2: 在 data/mod.rs 声明 state_loader**

```rust
pub mod equipment;
pub mod loader;
pub mod state_loader;
pub mod subunit;
pub mod template;
```

- [ ] **Step 3: 写 state_loader.rs**

Create `src/data/state_loader.rs`:

```rust
//! State 加载器: history/states/*.txt → Vec<State>(初始集合)
//!
//! 原版结构(见 spec §5):
//!   state={ id=1 name="X" manpower=N state_category=town
//!           history={ owner=FRA add_core_of=COR buildings={...} }
//!           provinces={ 3838 9851 } }
//! owner/cores/buildings 在 history={} 子块; provinces 是裸数字列表(Value::List)

use crate::parser::{Block, Value};
use crate::runtime::State;
use std::collections::HashMap;

/// 解析 state 文件, 产出初始 State 集合
pub fn load_states(src: &str) -> Vec<State> {
    let block = match crate::parser::parse(src) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[data] 警告: state 文件解析失败: {:?}", e);
            return vec![];
        }
    };
    block.fields.iter()
        .filter(|f| f.key == "state")
        .filter_map(|f| {
            if let Value::Block(sb) = &f.value { parse_state_block(sb) } else { None }
        })
        .collect()
}

fn parse_state_block(b: &Block) -> Option<State> {
    let num = |k: &str| b.fields.iter()
        .find(|f| f.key == k).and_then(|f| f.value.as_scalar_num());
    let str_val = |k: &str| b.fields.iter()
        .find(|f| f.key == k).and_then(|f| f.value.as_scalar_str()).unwrap_or("").to_string();

    let id = num("id")? as u32;
    let name = str_val("name");
    let manpower = num("manpower").unwrap_or(0.0);
    let category = str_val("state_category");

    // owner/cores/buildings 在 history={} 子块
    let history = find_block(b, "history")?;
    let owner = history.fields.iter()
        .find(|f| f.key == "owner").and_then(|f| f.value.as_scalar_str()).unwrap_or("").to_string();
    let cores: Vec<String> = history.fields.iter()
        .filter(|f| f.key == "add_core_of")
        .filter_map(|f| f.value.as_scalar_str().map(String::from))
        .collect();
    let buildings: HashMap<String, f64> = find_block(history, "buildings")
        .map(|bb| bb.fields.iter()
            .filter_map(|f| f.value.as_scalar_num().map(|v| (f.key.clone(), v)))
            .collect())
        .unwrap_or_default();

    let provinces = parse_provinces_list(b);

    Some(State {
        id, name,
        owner: owner.clone(),  // 法理归属
        controller: owner,     // 初始 controller = owner(未占领)
        manpower, state_category: category, cores, buildings, provinces,
    })
}

/// 解析 provinces={ 3838 9851 } 块(裸数字列表)
/// parser 把 { num num } 解析成 Value::List, 不是 Value::Block
fn parse_provinces_list(state_block: &Block) -> Vec<u32> {
    let Some(pf) = state_block.fields.iter().find(|f| f.key == "provinces") else {
        return vec![];
    };
    match &pf.value {
        Value::List(items) => items.iter()
            .filter_map(|s| s.parse::<u32>().ok())
            .collect(),
        Value::Block(b) => b.fields.iter()
            .filter_map(|f| f.value.as_scalar_num().map(|v| v as u32))
            .collect(),
        _ => vec![],
    }
}

fn find_block<'a>(block: &'a Block, key: &str) -> Option<&'a Block> {
    block.fields.iter()
        .find(|f| f.key == key)
        .and_then(|f| if let Value::Block(b) = &f.value { Some(b) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_load_state_from_block() {
        let src = r#"state={
            id=42
            name="STATE_42"
            manpower = 100000
            state_category = city
            history={
                owner = GER
                add_core_of = GER
                add_core_of = FRA
                buildings = { infrastructure = 3 industrial_complex = 2 }
            }
            provinces={ 100 101 102 }
        }"#;
        let states = load_states(src);
        assert_eq!(states.len(), 1);
        let s = &states[0];
        assert_eq!(s.id, 42);
        assert_eq!(s.owner, "GER");
        assert_eq!(s.controller, "GER"); // 初始 = owner
        assert!((s.manpower - 100000.0).abs() < 1e-9);
        assert_eq!(s.state_category, "city");
        assert_eq!(s.cores, vec!["GER".to_string(), "FRA".to_string()]);
        assert_eq!(s.provinces, vec![100, 101, 102]);
        assert!((s.buildings.get("infrastructure").copied().unwrap_or(0.0) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn t_load_real_state_file() {
        let src = include_str!("../data_raw/states/1-France.txt");
        let states = load_states(src);
        assert!(states.len() >= 1, "应解析出至少 1 个 state");
        let s = states.iter().find(|s| s.id == 1).expect("应有 state id=1");
        assert_eq!(s.owner, "FRA");
        assert!(!s.provinces.is_empty(), "科西嘉应含省份");
    }
}
```

- [ ] **Step 4: 运行 state_loader 测试**

Run: `cargo test data::state_loader::`
Expected: 2 passed; 0 failed

- [ ] **Step 5: 提交**

```bash
git add src/data/state_loader.rs src/data/mod.rs src/data_raw/states/
git commit -m "feat(state): state_loader 加载 history/states/*.txt(裸数字列表+history子块)"
```

---

## Task 9: 测试脚本全量迁移

这是最大工作量但最机械的部分。所有用 `create_province { owner=X }` 的测试脚本, 改成先 `create_state` 再 `create_province { state=Y }`。

**Files:**
- Modify: `tests/battle.rs`, `tests/teleport_bug.rs`, `tests/scope.rs`, `tests/integration.rs`, `src/bin/diag.rs`

- [ ] **Step 1: 迁移规律(全机械)**

每个测试文件里:
```hoi4
# 改造前(每省带 owner):
create_province = { id = 1 owner = FRA neighbors = { 2 3 } }
create_province = { id = 2 owner = GER neighbors = { 1 } }

# 改造后(先建州, 省引用州; 同国共用一个测试 State):
create_state = { id = 100 owner = FRA }
create_state = { id = 200 owner = GER }
create_province = { id = 1 state = 100 neighbors = { 2 3 } }
create_province = { id = 2 state = 200 neighbors = { 1 } }
```

读 controller 的断言:
```rust
// 改造前:
assert_eq!(w.provinces.get(&1).unwrap().controller, "FRA");
// 改造后:
assert_eq!(w.province_controller(1).unwrap_or(""), "FRA");
```

设 controller(占领模拟):
```rust
// 改造前:
w.provinces.get_mut(&1).unwrap().controller = "GER".into();
// 改造后:
w.set_state_controller(1, "GER");
```

- [ ] **Step 2: 迁移 tests/battle.rs**

搜索 `create_province` 和 `.controller`/`.owner` in battle.rs。每个:
- 建省脚本加 create_state + state 引用
- `.controller` 读改 `province_controller`
- `.controller =` 写改 `set_state_controller`

- [ ] **Step 3: 迁移 tests/teleport_bug.rs, scope.rs, integration.rs, bin/diag.rs**

同 Step 2 规律, 逐文件迁移。

- [ ] **Step 4: 全量编译 + 测试**

Run: `cargo test`
Expected: 全绿(164 测试迁移后全过)。

若有失败, 多半是某处 `.controller`/`.owner` 漏改, 或建省脚本缺 create_state。用 `cargo build 2>&1 | grep "controller\|owner"` 定位。

- [ ] **Step 5: 提交**

```bash
git add tests/ src/bin/diag.rs
git commit -m "test(state): 全量测试脚本迁移(create_state + state引用 + controller派生)"
```

---

## Task 10: 端到端 + 验收

**Files:**
- Modify: `tests/integration.rs`(加 State 端到端测试)

- [ ] **Step 1: 端到端测试 — 占领改 State controller, 同州省跟随**

```rust
#[test]
fn t_occupation_changes_state_controller() {
    use hoi4_clone::runtime::{World, Interpreter, Registry, GameClock};
    use hoi4_clone::commands::register_all;
    use hoi4_clone::ast::lower::lower_effects;
    use hoi4_clone::parser::parse;

    let mut w = World::new();
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);

    // 建 State 100(FRA), 含两省 1 和 2
    let setup = r#"
        create_state = { id = 100 owner = FRA }
        create_province = { id = 1 state = 100 neighbors = { 2 } }
        create_province = { id = 2 state = 100 neighbors = { 1 } }
    "#;
    interp.run(&lower_effects(&parse(setup).unwrap()), &mut w);

    // 初始: 两省都属 FRA
    assert_eq!(w.province_controller(1).unwrap_or(""), "FRA");
    assert_eq!(w.province_controller(2).unwrap_or(""), "FRA");

    // 占领省 1 → 改 State 100 的 controller → 省 2 也跟随变 GER
    w.set_state_controller(1, "GER");
    assert_eq!(w.province_controller(1).unwrap_or(""), "GER", "省1应被占");
    assert_eq!(w.province_controller(2).unwrap_or(""), "GER", "同州省2应跟随");
    // owner 不变(法理仍 FRA)
    assert_eq!(w.province_owner(1).unwrap_or(""), "FRA", "法理归属不变");
}
```

- [ ] **Step 2: 端到端测试 — create_state + 派生查询**

```rust
#[test]
fn t_create_state_and_province_reference() {
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
```

- [ ] **Step 3: 全量回归 + WASM**

```bash
cargo test
cargo build --target wasm32-unknown-unknown --lib --release
```
Expected: 测试全绿, WASM 编译通过。

- [ ] **Step 4: 验收对照(spec §11)**

1. cargo test 全绿 ✓
2. create_state + create_province 引用 ✓(Step 2)
3. province_controller 派生 ✓
4. 占领改 State controller, 同州省跟随 ✓(Step 1)
5. owner ≠ controller 能区分 ✓(Step 1)
6. load_states 读真实文件 ✓(Task 8)
7. WASM 编译通过 ✓
8. 后续系统读 State 字段不改结构(spec §10)✓

- [ ] **Step 5: 更新 HANDOFF.md**

加 State 里程碑, 更新代码结构(entities 加 State/Province 改; world 加 states)。

- [ ] **Step 6: 提交**

```bash
git add tests/integration.rs docs/HANDOFF.md
git commit -m "feat(state): 端到端测试 + 验收(占领改State/同州省跟随/法理vs控制)"
```

---

## 实现顺序提示

严格 Task 1→10。**关键风险与注意点**:

- **Task 1+2 后大面积编译失败**: Province 删字段, 所有 `.controller`/`.owner` 调用点报错。这是预期。Task 3-7 逐文件修复。
- **Task 9 是最大工作量**: 全量测试脚本迁移。纯机械但有量(~12 处建省脚本 + ~12 处断言)。建议用 grep 找全后批量改。
- **借用冲突**: recovery.rs(Task 6)和 wasm_api(Task 7)在遍历时读 province_controller(借 world), 需快照模式。看实际代码调整。
- **movement.rs 闭包内读 controller(Task 5)**: 226/260 行在闭包里, 可能需重构取 pid 后闭包外查。
- **真实 state 文件的 history 子块**: 原版 owner/cores/buildings 在 `history={}` 里, 不在 state 顶层。loader 要处理这个嵌套(Task 8 已含)。
