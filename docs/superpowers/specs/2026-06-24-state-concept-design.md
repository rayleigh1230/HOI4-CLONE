# State 概念(省份上级容器 + 归属权威源) 设计文档

> 日期: 2026-06-24
> 状态: 已批准(头脑风暴),待实现
> 关联: `docs/design-principles.md`(原则1: 原版设计是首要参考)
> 关联: `docs/HANDOFF.md`(项目现状, Province 现为平铺最高层)
> 关联: 补给系统(后续独立项目, 依赖本 State)

---

## 0. 背景与目标

### 现状问题

当前项目里 `Province` 是地图的最高层级——每个省份直接存 `owner`/`controller`, 平铺无上级。原版 HOI4 是**两级地图结构**:

```
State(州/地区)        ← 缺这一层
  ├── Province 1
  ├── Province 2      ← 我们只有这一级
  └── Province 3
```

原版里很多核心系统不挂在省份上, 而挂在 State 上: 工厂/建筑/人力/占领/核心领土/补给枢纽。缺 State 这层, 后续生产/补给/占领系统无处挂载数据。

### 目标

引入 State 作为 Province 的上级容器, 成为**归属/建筑/人力的唯一权威源**。Province 归属彻底从 State 派生(单一数据源, 无同步隐患)。为补给/生产/占领系统预留接入点。

### 范围(本次做)

- **State 数据结构**: id/name/owner/controller/manpower/state_category/cores/buildings/provinces
- **归属派生**: Province 删 owner/controller, 加 state_id; 归属从 State 派生
- **命令**: create_state / create_province(改造, 删 owner 加 state)
- **Loader**: load_states 读 history/states/*.txt 产出初始 State 集合
- **占领语义**: 占领改 State.controller(不改 owner), 省份自动跟随

### 非目标(本次不做)

- **补给系统**: 依赖 State 但本身是复杂动态机制(supply flow/衰减/枢纽网络), 下一个独立项目
- **建筑系统**: buildings 用简单映射占位, 建模/建造/修复是独立项目
- **剧本切换**: 本次 load_states 产出 1936 默认初始值; 1939 等剧本覆盖留接口(运行时 effect 改归属)
- **victory_points / local_supplies**: 字段后续系统用到再加
- **抵抗/顺从度**: 占领系统的子机制, 后续

---

## 1. 核心设计决策(头脑风暴结论)

| # | 决策 | 选择 |
|---|---|---|
| 1 | 归属语义 | 方式B: Province 归属彻底从 State 派生(删 owner/controller, 加 state_id) |
| 2 | 字段范围 | 核心+地基: id/name/owner/controller/manpower/state_category/cores/buildings/provinces |
| 3 | buildings | 简单映射 HashMap<String, f64> 占位(不区分 state/province 级, 不建模建筑定义) |
| 4 | State 归属 | 进 World(可变运行时状态), 不进 GameData(只读定义) |
| 5 | 进 World 的理由 | State 运行时可变(被占领/割让/建筑变更); 且剧本切换需运行时改归属 |
| 6 | create_province | 改彻底: 删 owner 参数, 必须先建 State, 省份引用 state_id |
| 7 | 占领语义 | 只改 State.controller, 不改 owner(法理归属 vs 实际控制) |
| 8 | controller 默认值 | 初始 = owner(未占领时法理=控制) |
| 9 | provinces 维护 | 双向: State 存 provinces(正向), Province 存 state_id(反向); 建省时自动注册正向 |

---

## 2. 数据模型与归属派生

### 2.1 State 结构

```rust
/// 州/地区(Province 的上级容器, 归属/建筑/人力的唯一权威源)
/// 可变运行时状态(进 World, 不进 GameData)
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

### 2.2 Province 改造

```rust
/// 省份(行军/战斗的基础单元)
/// 彻底改: 删 owner/controller, 加 state_id(归属从 State 派生)
#[derive(Debug, Clone, Default)]
pub struct Province {
    pub id: u32,
    pub state_id: u32,             // ★新增: 指向所属 State
    pub terrain: String,
    pub neighbors: Vec<u32>,
    // owner/controller 删除 —— 从 State 派生
}
```

### 2.3 World 加 State 存储 + 派生查询

```rust
pub struct World {
    pub states: HashMap<u32, State>,       // ★新增: 可变运行时
    pub provinces: HashMap<u32, Province>,
    pub divisions: HashMap<u64, Division>,
    // ... 其余不变
}

impl World {
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
}
```

### 2.4 friendly_neighbor 改造

```rust
// 改造前:
pub fn friendly_neighbor(&self, province: u32, tag: &str) -> Option<u32> {
    let prov = self.provinces.get(&province)?;
    prov.neighbors.iter().copied().find(|n| {
        self.provinces.get(n).map(|p| p.controller == tag).unwrap_or(false)
    })
}

// 改造后(读 State 派生):
pub fn friendly_neighbor(&self, province: u32, tag: &str) -> Option<u32> {
    let prov = self.provinces.get(&province)?;
    prov.neighbors.iter().copied().find(|n| {
        self.province_controller(n).map(|c| c == tag).unwrap_or(false)
    })
}
```

---

## 3. 命令接口

### 3.1 create_state(新增)

```hoi4
create_state = {
    id = 1
    name = "Berlin"
    owner = GER
    controller = GER          # 可选, 缺省 = owner
    manpower = 500000
    state_category = large_city
    cores = { GER }
    buildings = { infrastructure = 5 industrial_complex = 4 }
}
```

实现:
```rust
reg.register("create_state", |w, p| {
    let id = num_of(np(p, "create_state", "id")?)? as u32;
    let name = ParamGet::get(p, "name").and_then(Arg::as_str).unwrap_or("").to_string();
    let owner = np(p, "create_state", "owner")?.as_str()?;
    let controller = ParamGet::get(p, "controller").and_then(Arg::as_str).unwrap_or(owner).to_string();
    let manpower = ParamGet::get(p, "manpower").and_then(Arg::as_num).unwrap_or(0.0);
    let category = ParamGet::get(p, "state_category").and_then(Arg::as_str).unwrap_or("wasteland").to_string();
    let cores = parse_str_list(p, "cores");
    let buildings = parse_num_map(p, "buildings");
    w.states.insert(id, State {
        id, name, owner: owner.into(), controller, manpower,
        state_category: category, cores, buildings,
        provinces: vec![],
    });
    Ok(())
});
```

### 3.2 create_province 改造(删 owner, 加 state)

```hoi4
# 改造前:
create_province = { id = 1 owner = FRA neighbors = { 2 3 } }

# 改造后:
create_province = { id = 1 state = 100 neighbors = { 2 3 } }
```

实现:
```rust
reg.register("create_province", |w, p| {
    let id = num_of(np(p, "create_province", "id")?)? as u32;
    let state_id = num_of(np(p, "create_province", "state")?)? as u32;
    let terrain = ParamGet::get(p, "terrain").and_then(Arg::as_str).unwrap_or("plains").to_string();
    let neighbors = parse_num_list(p, "neighbors");
    w.provinces.insert(id, Province { id, state_id, terrain, neighbors });
    // 反向注册: 省 id 加入所属 State 的 provinces 列表
    if let Some(state) = w.states.get_mut(&state_id) {
        state.provinces.push(id);
    }
    Ok(())
});
```

---

## 4. 占领语义改造

### 4.1 占领只改 controller, 不改 owner

原 resolve.rs / movement.rs 的占领:
```rust
// 改造前(直接改 province):
province_captures.push((*province, winner.clone()));
// ...
p.controller = winner.clone();
p.owner = winner;
```

改造后:
```rust
// 改造后(改 State, 省份自动跟随; 只改 controller):
for (province_id, winner) in province_captures {
    w.set_state_controller(province_id, &winner);
    // 不改 owner —— 法理归属不变(德国控制法国领土, 但法理还是法国的)
}
```

movement.rs 的两处占领(Capture / RetreatIntoEnemy)同理改 set_state_controller。
注意: 原代码部分占领会同时改 owner(如 RetreatIntoEnemy "owner move 在此"), 改造后**都不改 owner**, 统一只改 controller。owner 的变更留给后续和平会议/割让系统。

### 4.2 归属读取改造

所有读 `p.controller` / `p.owner` 的地方(~15处), 改成派生查询:

```rust
// 改造前:
let friendly = w.provinces.get(&loc).map(|p| p.controller == owner).unwrap_or(false);

// 改造后:
let friendly = w.province_controller(loc).map(|c| c == owner).unwrap_or(false);
```

---

## 5. State Loader

### 5.1 加载真实文件

原版 `history/states/1-France.txt`:
```
state={
    id=1
    name="STATE_1"
    manpower = 322900
    state_category = town
    history={
        owner = FRA
        add_core_of = COR
        add_core_of = FRA
        buildings = { infrastructure = 2 industrial_complex = 1 }
    }
    provinces={ 3838 9851 11804 }
}
```

注意原版结构: `owner`/`buildings`/`cores` 在 `history={}` 子块里; `provinces={}` 是裸数字列表。

```rust
// src/data/state_loader.rs

/// 解析 state 文件(history/states/*.txt), 产出初始 State 集合
/// 一个文件可含多个 state={} 块
pub fn load_states(src: &str) -> Vec<State> {
    let block = match parser::parse(src) { ... };
    let mut out = Vec::new();
    for f in &block.fields {
        if f.key == "state" {
            if let Value::Block(sb) = &f.value {
                if let Some(state) = parse_state_block(sb) {
                    out.push(state);
                }
            }
        }
    }
    out
}

fn parse_state_block(b: &Block) -> Option<State> {
    let id = scalar_num(b, "id")? as u32;
    let name = scalar_str(b, "name");
    let manpower = scalar_num(b, "manpower").unwrap_or(0.0);
    let category = scalar_str(b, "state_category");
    // owner/cores/buildings 在 history={} 子块
    let history = find_block(b, "history")?;
    let owner = scalar_str(history, "owner");
    let cores: Vec<String> = history.fields.iter()
        .filter(|f| f.key == "add_core_of")
        .filter_map(|f| f.value.as_scalar_str().map(String::from))
        .collect();
    let buildings = find_block(history, "buildings")
        .map(|bb| bb.fields.iter()
            .filter_map(|f| f.value.as_scalar_num().map(|v| (f.key.clone(), v)))
            .collect())
        .unwrap_or_default();
    let provinces = parse_provinces_list(b);
    Some(State {
        id, name, owner: owner.clone(), controller: owner,  // 初始 controller = owner
        manpower, state_category: category, cores, buildings, provinces,
    })
}

/// 解析 provinces={ 3838 9851 11804 } 块(裸数字列表)
/// 注意: parser 把 { num num } 解析成 Value::List(不是 Block), 要单独处理
fn parse_provinces_list(state_block: &Block) -> Vec<u32> {
    let Some(pf) = state_block.fields.iter().find(|f| f.key == "provinces") else {
        return vec![];
    };
    match &pf.value {
        Value::List(items) => items.iter()
            .filter_map(|s| s.parse::<u32>().ok())
            .collect(),
        Value::Block(b) => b.fields.iter()  // 兼容: 某些文件可能用块形式
            .filter_map(|f| f.value.as_scalar_num().map(|v| v as u32))
            .collect(),
        _ => vec![],
    }
}
```

### 5.2 World 接收初始 State

```rust
impl World {
    /// 用初始 State 集合构造(加载真实开局数据)
    pub fn with_states(states: Vec<State>) -> World {
        let mut w = World::new();
        for s in states { w.states.insert(s.id, s); }
        w
    }
}
```

### 5.3 剧本覆盖(预留, 本次不实现)

load_states 产出 1936 默认初始值。后续剧本系统: World 初始化后按 bookmark 运行一组 transfer_state 命令覆盖归属。这只是往 world.states 改值, 不改架构。

---

## 6. WASM 序列化改造

现有 wasm_api 直接读 `p.controller` / `p.owner`。改造后 Province 无这俩字段, 序列化改成读派生:

```rust
// 改造前:
s.push_str(&format!("{{\"id\":{},\"controller\":\"{}\",...", p.id, p.controller));

// 改造后:
let controller = world.province_controller(p.id).unwrap_or("");
let owner = world.province_owner(p.id).unwrap_or("");
s.push_str(&format!("{{\"id\":{},\"controller\":\"{}\",\"owner\":\"{}\",...",
    p.id, controller, owner));
```

前端 JS 不受影响(仍收到 controller/owner 字段)。wasm_api 里设置 controller 的地方(set_province_controller 等)改成 set_state_controller。

---

## 7. 错误处理

1. **省份找不到所属 State**: `province_controller` 返回 None → 调用方当中立处理(返回 false / 空)。
2. **建省时 state_id 指向不存在的 State**: 警告 + 省份仍建(state_id 存着, State 建好后反向注册补上)。
3. **占领改 controller 时 State 不存在**: set_state_controller 内部静默跳过(找不到不 panic)。
4. **load_states 解析失败**: 警告 + 跳过该 state 块(和 equipment loader 一致)。

---

## 8. 迁移清单

| 文件 | 迁移内容 | 处数 |
|---|---|---|
| `runtime/entities.rs` | Province 删 owner/controller 加 state_id; 新增 State 结构 | 结构改 |
| `runtime/world.rs` | 加 states + province_controller/owner/set_state_controller + friendly_neighbor 改派生 | 新增+改 |
| `combat/commands.rs` | create_state 新增; create_province 删 owner 加 state; join_as_attacker/move_division 读 controller 改派生(~5处) | 改 |
| `combat/resolve.rs` | province_captures 改 set_state_controller; 测试建省脚本迁移(~4处) | 改 |
| `combat/movement.rs` | 6处读 controller 改派生; 2处占领改 set_state_controller; 测试迁移(~8处) | 改 |
| `combat/recovery.rs` | 1处读 controller 改派生 | 改 |
| `combat/pathfinding.rs` | 注释更新 | 小 |
| `wasm_api.rs` | 序列化读派生; set_controller 改 set_state_controller(2处) | 改 |
| `data/state_loader.rs` | 新增: load_states + parse_state_block | 新增 |
| `data/mod.rs` | 不加 states(State 进 World); loader 函数可选暴露 | 小 |
| 测试: battle.rs / teleport_bug.rs / scope.rs / integration.rs | 建省脚本迁移(create_state + state 引用) | 机械改(~8处) |
| `bin/diag.rs` | 建省脚本迁移 | 小 |

迁移规律(全机械):
- 建省脚本: 每国先建一个测试 State, 该国省都指向它
- 读归属: `p.controller` → `world.province_controller(pid)`
- 写归属: `p.controller = x` → `world.set_state_controller(pid, x)`

---

## 9. 测试策略

| 测试组 | 验证内容 |
|---|---|
| State 基础 | create_state 建州; create_province 引用 state_id; 反向注册 |
| 派生查询 | province_controller/owner 从 State 正确返回 |
| 占领改 State | 占领一省 → 所属 State controller 变; 同 State 其它省跟随 |
| 法理 vs 控制 | 占领后 owner 不变, controller 变 |
| 现有测试迁移 | battle/teleport/scope/integration 全量迁移后通过(零功能回归) |
| 真实文件加载 | load_states 读 history/states/*.txt 产出正确 State |
| WASM | 序列化省份仍含 controller/owner(从 State 派生) |

---

## 10. 后续扩展(本次预留, 不实现)

| 后续系统 | 如何接入(不改 State 结构) |
|---|---|
| **补给系统** | 读 State.buildings["infrastructure"](基建等级); 补给枢纽从 buildings 查; supply flow 沿 State 计算 |
| **生产系统** | 读 State.buildings["industrial_complex"/"arms_factory"](工厂数); State.manpower(征兵) |
| **建筑系统** | buildings HashMap 升级为结构化(区分 state/province 级); 但 State 结构本身不动 |
| **占领/抵抗** | State.controller(已存) + 新增 resistance/compliance 字段(加字段不改结构) |
| **核心领土** | State.cores(已存); 影响占领顺从度、生产效率 |
| **剧本切换** | World 初始化后运行 transfer_state 命令改 owner/controller |
| **胜利点/投降** | State 加 victory_points 字段(后续) |

**核心: State 这次搭好, 后续系统只读字段或加字段, 不重构结构。**

---

## 11. 验收标准

1. `cargo test` 全绿(现有 164 测试迁移后全过 + State 新增测试)
2. `create_state { owner=GER }` 建州, `create_province { state=1 }` 建省引用州
3. `province_controller(省id)` 正确返回所属州的 controller
4. 占领一省 → 所属 State controller 变, 同 State 其它省跟着变
5. `State.owner`(法理) ≠ `State.controller`(实际) 能区分
6. `load_states` 读 history/states/*.txt 产出正确 State 集合
7. WASM 编译通过, 前端收到的省份数据仍含 controller/owner
8. **后续做补给/生产系统时, 从 State 读 buildings/manpower, 不改 State 结构**
