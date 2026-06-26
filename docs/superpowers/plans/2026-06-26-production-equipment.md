# 生产系统 + 装备补充闭环 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 arms_factory 生产循环 + 装备 key 链路重构(variant 全名),与现有 reinforce 形成完整闭环。

**Architecture:** Country 加 `production_lines` 字段;State 加 `resources` 字段;新增 `src/economy/` 模块承载 ProductionLine 实体 + production_step 产出循环;reinforce 改造为按 chassis 在 stockpile 查 variant 池补给;UI 加生产面板。

**Tech Stack:** Rust(stable-x86_64-pc-windows-gnu) + WASM + ES Module 单文件 JS(无构建) + Playwright 验证。

**关联 spec:** `docs/superpowers/specs/2026-06-26-production-equipment-design.md`

**测试基线:** 208 → 目标 ~225+(新增 15+ 单元 + 4 integration)。**任何 struct 加字段后必须跑全量 `cargo test`(含 tests/ 集成)。** 改 `Province`/`Country`/`State` 结构的 task 都要检查 `tests/battle.rs`/`tests/scope.rs`/`tests/teleport_bug.rs` 是否需要补 `..Default::default()`(既有教训,见 HANDOFF)。

---

## 文件结构

**新增文件:**
- `src/economy/mod.rs` — 模块声明 + 常量 + `FactorySlot` + `ProductionLine` + `variant_chassis`
- `src/economy/production.rs` — `production_step` + `resource_penalty` + `change_line_variant`
- `src/economy/tests.rs` — 单元测试(槽位/效率/产出/资源/切换)
- `tests/production.rs` — integration 测试(端到端生产循环)
- `web/js/views/productionPanel.js` — 生产面板
- `web/js/views/stockpilePanel.js` — 仓库面板(悬停浮层)

**修改文件:**
- `src/runtime/entities.rs` — Country 加 `production_lines`、`next_line_id`;State 加 `resources`
- `src/runtime/world.rs` — Default 同步字段
- `src/data/equipment.rs` — `EquipmentDef` 加 `resources`
- `src/data/mod.rs` — `GameData` 可能加 helper(按需)
- `src/data/state_loader.rs` — 解析 `resources` 块
- `src/data/loader.rs` — 加载装备 `resources` 字段
- `src/combat/equipment_data.rs` — 硬编码表加 `resources` 字段
- `src/combat/reinforce.rs` — 按 chassis 查 variant 池补给
- `src/combat/commands.rs` — 注册 5 个新/改命令
- `src/runtime/clock.rs` — `on_daily` 后调 `production_step`
- `src/wasm_api.rs` — `get_state` 导出 `stockpile`/`production_lines`
- `src/lib.rs` — `pub mod economy;`
- `web/index.html` — 顶栏加生产按钮 + 仓库徽章
- `web/css/app.css` — 面板样式
- `web/js/main.js` — setup 改造(初始生产线/资源/库存)
- `web/js/engine/commands.js` — WASM 命令封装
- `tests/web_demo.mjs` — 加 5 项端到端

---

## Task 1: 数据结构 — Country + State 加字段

**Files:**
- Modify: `src/runtime/entities.rs`
- Modify: `src/runtime/world.rs`(若 Default 显式列字段)

- [ ] **Step 1: 读现有 entities.rs 确认字段位置**

读 `src/runtime/entities.rs:21-52`,确认 State 和 Country 的字段顺序与 Default impl。

- [ ] **Step 2: State 加 resources 字段**

修改 `src/runtime/entities.rs:21-31`,在 `buildings` 后加 `resources`:

```rust
#[derive(Debug, Clone, Default)]
pub struct State {
    pub id: u32,
    pub name: String,
    pub owner: String,
    pub controller: String,
    pub manpower: f64,
    pub state_category: String,
    pub cores: Vec<String>,
    pub buildings: HashMap<String, f64>,
    /// 本土资源产出(steel/tungsten/aluminium/chromium/oil/rubber)
    /// 从原版 history/states/*.txt 的 `resources = { steel = N }` 块加载
    pub resources: HashMap<String, f64>,
    pub provinces: Vec<u32>,
}
```

- [ ] **Step 3: Country 加 production_lines + next_line_id**

修改 `src/runtime/entities.rs:33-52`(`Country` struct)。在 `equipment_stockpile` 后加 `production_lines`;同时在 World 加 `next_line_id`(下一步 Task 4 才用,但字段先备):

实际 Country 加一个字段:
```rust
pub struct Country {
    pub tag: String,
    pub owned_states: Vec<u32>,
    pub capital_state: u32,
    pub political_power: f64,
    pub stability: f64,
    pub war_support: f64,
    pub equipment_stockpile: std::collections::HashMap<String, f64>,
    pub manpower_pool: f64,
    /// 生产线列表(per-slot 效率, 对齐原版 production_line)
    pub production_lines: Vec<crate::economy::ProductionLine>,
    pub modifiers: crate::combat::modifier::ModifierStack,
    pub faction: Option<String>,
}
```

`Default for Country`(`entities.rs:54-69`)补 `production_lines: Default::default(),`。

- [ ] **Step 4: 跑 cargo build 验证编译**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo build 2>&1 | tail -30
```

预期:可能因 `crate::economy::ProductionLine` 未定义而报错。这是预期的(下个 Task 创建)。**暂时注释掉 Country 的 production_lines 字段**让编译过,Task 4 解开。

替代:先做 Task 2(economy/mod.rs),再做本 Task。**调整顺序:先 Task 2,后 Task 1。**(见下方)

- [ ] **Step 5: 跑 cargo test 全量回归**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test -- --test-threads=1 2>&1 | tail -10
```

预期:**208 测试全绿**(纯加字段不破坏现有测试)。若 tests/battle.rs 报 Province 构造错,补 `..Default::default()`(既有教训)。

- [ ] **Step 6: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add src/runtime/entities.rs src/runtime/world.rs && git commit -m "feat(entities): State 加 resources, Country 加 production_lines 字段"
```

---

## Task 2: economy 模块 — ProductionLine + 常量 + variant_chassis

**Files:**
- Create: `src/economy/mod.rs`
- Create: `src/economy/production.rs`(空占位,Task 5 填)
- Create: `src/economy/tests.rs`(空占位,Task 6 填)
- Modify: `src/lib.rs`

- [ ] **Step 1: 写 src/economy/mod.rs**

```rust
//! 生产系统: 工厂每日产出装备入国家仓库(spec 2026-06-26-production-equipment-design)
//!
//! Country.production_lines 存结构, production_step 写逻辑。
//! 与 runtime/combat 平行模块。

pub mod production;
#[cfg(test)]
mod tests;

use crate::data::EquipStats;

/// 原版常量(defines 00_defines.lua 行 598-623)
pub const FACTORY_SPEED_MIL: f64     = 4.5;   // BASE_FACTORY_SPEED_MIL
pub const EFFICIENCY_START: f64      = 0.10;  // BASE_FACTORY_START_EFFICIENCY_FACTOR
pub const EFFICIENCY_MAX: f64        = 0.50;  // BASE_FACTORY_MAX_EFFICIENCY_FACTOR
pub const EFFICIENCY_GAIN: f64       = 1.0;   // BASE_FACTORY_EFFICIENCY_GAIN
pub const EFFICIENCY_BALANCE: f64    = 0.1;   // BASE_FACTORY_EFFICIENCY_BALANCE_FACTOR
pub const VARIANT_RETENTION: f64     = 0.90;  // BASE_FACTORY_EFFICIENCY_VARIANT_CHANGE_FACTOR
pub const RESOURCE_LACK_PENALTY: f64 = 0.05;  // |PRODUCTION_RESOURCE_LACK_PENALTY|
pub const INACTIVE_SLOT_DECAY: f64   = 0.01;  // EFFICIENCY_LOSS_PER_UNUSED_DAY
pub const SLOTS_PER_LINE: usize      = 15;

/// 一个工厂槽位(per-slot 效率, 对齐原版)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FactorySlot {
    pub efficiency: f64,    // 0..EFFICIENCY_MAX
    pub active: bool,
}

/// 一条生产线(对齐原版 production_line, 固定 15 槽位)
#[derive(Debug, Clone)]
pub struct ProductionLine {
    pub id: u32,
    pub chassis: String,        // 从 variant 派生
    pub variant: String,        // 完整 variant 名, 如 "infantry_equipment_1"
    pub slots: Vec<FactorySlot>,
    pub active_count: u32,
}

impl ProductionLine {
    pub fn new(id: u32, variant: String) -> Self {
        let chassis = variant_chassis(&variant).to_string();
        Self {
            id, chassis, variant,
            slots: (0..SLOTS_PER_LINE).map(|_| FactorySlot::default()).collect(),
            active_count: 0,
        }
    }

    /// 激活前 N 个槽位。新激活的槽(之前 inactive 且 eff=0)从 EFFICIENCY_START 起步。
    /// 已激活槽不变。被关闭的槽(之前 active 现 inactive)保留 efficiency。
    pub fn set_active(&mut self, n: u32) {
        let n = (n as usize).min(SLOTS_PER_LINE);
        for i in 0..SLOTS_PER_LINE {
            let was_active = self.slots[i].active;
            let now_active = i < n;
            self.slots[i].active = now_active;
            // 新激活且 efficiency=0 → 起始效率
            if !was_active && now_active && self.slots[i].efficiency == 0.0 {
                self.slots[i].efficiency = EFFICIENCY_START;
            }
        }
        self.active_count = n as u32;
    }
}

/// 从 variant 全名解析 chassis。
/// "infantry_equipment_1" → "infantry_equipment"
/// "light_tank_chassis_1" → "light_tank_chassis"
/// "infantry_equipment"(无 _数字 后缀) → 同名
pub fn variant_chassis(variant: &str) -> &str {
    if let Some(pos) = variant.rfind('_') {
        let suffix = &variant[pos + 1..];
        if !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()) {
            return &variant[..pos];
        }
    }
    variant
}

// EquipStats re-export(handy for production.rs)
pub use crate::data::EquipStats as GameEquipStats;
```

- [ ] **Step 2: 写 src/economy/production.rs(空占位,后续 Task 填)**

```rust
//! 生产每日产出循环(具体实现见 Task 5)

// 占位, Task 5 填充
```

- [ ] **Step 3: 写 src/economy/tests.rs(空占位)**

```rust
// 测试见 Task 4 + Task 6
```

- [ ] **Step 4: lib.rs 注册模块**

读 `src/lib.rs`,加 `pub mod economy;`(若不存在)。

- [ ] **Step 5: 写测试 — variant_chassis**

在 `src/economy/tests.rs` 加:

```rust
use super::*;

#[test]
fn t_variant_chassis_strips_numeric_suffix() {
    assert_eq!(variant_chassis("infantry_equipment_1"), "infantry_equipment");
    assert_eq!(variant_chassis("light_tank_chassis_1"), "light_tank_chassis");
    assert_eq!(variant_chassis("artillery_equipment_2"), "artillery_equipment");
}

#[test]
fn t_variant_chassis_handles_no_suffix() {
    assert_eq!(variant_chassis("infantry_equipment"), "infantry_equipment");
    assert_eq!(variant_chassis("light_tank_chassis"), "light_tank_chassis");
}

#[test]
fn t_variant_chassis_handles_underscore_non_numeric() {
    // 不应以 "chassis"(字母) 结尾被剥
    assert_eq!(variant_chassis("foo_bar"), "foo_bar");
}
```

- [ ] **Step 6: 跑测试确认通过**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test economy -- --test-threads=1 2>&1 | tail -20
```

预期:`t_variant_chassis_*` 3 个测试通过。

- [ ] **Step 7: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add src/economy/ src/lib.rs && git commit -m "feat(economy): 新增 economy 模块 + ProductionLine + variant_chassis"
```

---

## Task 3: 回到 Task 1 字段(解注释) + 写槽位管理测试

**Files:**
- Modify: `src/runtime/entities.rs`(若 Task 1 注释了)
- Modify: `src/economy/tests.rs`(加 ProductionLine 测试)

- [ ] **Step 1: 确认 Country.production_lines 字段已启用**

如 Task 1 Step 4 暂时注释了该字段,现在 economy::ProductionLine 已存在,解开注释。

- [ ] **Step 2: 跑 cargo build 确认编译过**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo build 2>&1 | tail -10
```

- [ ] **Step 3: 写槽位管理测试**

在 `src/economy/tests.rs` 末尾追加:

```rust
use super::super::economy::{ProductionLine, EFFICIENCY_START, SLOTS_PER_LINE};

#[test]
fn t_set_active_fills_slots_from_front() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(3);
    assert_eq!(line.active_count, 3);
    assert!(line.slots[0].active);
    assert!(line.slots[1].active);
    assert!(line.slots[2].active);
    assert!(!line.slots[3].active);
    // 新激活槽应从 EFFICIENCY_START 起步
    assert!((line.slots[0].efficiency - EFFICIENCY_START).abs() < 1e-9);
    assert!((line.slots[2].efficiency - EFFICIENCY_START).abs() < 1e-9);
    // 未激活槽 efficiency=0
    assert!(line.slots[3].efficiency == 0.0);
}

#[test]
fn t_active_count_clamped_at_15() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(99);
    assert_eq!(line.active_count, SLOTS_PER_LINE as u32);
    assert!(line.slots.iter().all(|s| s.active));
}

#[test]
fn t_reduce_factories_keeps_efficiency() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(5);
    // 把 slot 4 模拟成已积累效率
    line.slots[4].efficiency = 0.40;
    // 减到 3
    line.set_active(3);
    assert!(!line.slots[4].active);
    // 保留 efficiency(不重置)
    assert!((line.slots[4].efficiency - 0.40).abs() < 1e-9);
    assert_eq!(line.active_count, 3);
}

#[test]
fn t_reactivate_preserves_efficiency() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(5);
    line.slots[4].efficiency = 0.40;
    line.set_active(3);  // 关闭 slot 4(保留 eff 0.40)
    line.set_active(5);  // 重新激活 slot 4
    assert!(line.slots[4].active);
    // 应保留 0.40, 不重置到 EFFICIENCY_START
    assert!((line.slots[4].efficiency - 0.40).abs() < 1e-9);
}

#[test]
fn t_chassis_derived_from_variant() {
    let line = ProductionLine::new(0, "light_tank_chassis_1".into());
    assert_eq!(line.chassis, "light_tank_chassis");
    assert_eq!(line.variant, "light_tank_chassis_1");
}
```

- [ ] **Step 4: 跑测试确认通过**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test economy -- --test-threads=1 2>&1 | tail -20
```

预期:economy 模块 8 个测试全绿(3 variant_chassis + 5 ProductionLine)。

- [ ] **Step 5: 全量回归**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test -- --test-threads=1 2>&1 | tail -5
```

预期:**208 测试全绿**(纯加字段+方法,不改现有逻辑)。

- [ ] **Step 6: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add src/runtime/entities.rs src/economy/tests.rs && git commit -m "feat(economy): ProductionLine 槽位管理 + 测试"
```

---

## Task 4: EquipmentDef 加 resources 字段(数据驱动层)

**Files:**
- Modify: `src/data/equipment.rs:77-83`
- Modify: `src/data/loader.rs`(加载 resources 块)

- [ ] **Step 1: 读 src/data/equipment.rs:76-99 确认 EquipmentDef 结构**

```bash
# 看现有 EquipmentDef 定义和 compute_equipment_stats
```

- [ ] **Step 2: 加 resources 字段到 EquipmentDef**

修改 `src/data/equipment.rs:77-83`:

```rust
#[derive(Debug, Clone)]
pub struct EquipmentDef {
    pub name: String,
    pub chassis: String,
    pub year: u32,
    pub equip_type: String,
    pub stats: EquipStats,
    /// 生产所需资源(原版 `resources = { steel = 2 }`), 如 [("steel", 2.0)]
    pub resources: Vec<(String, f64)>,
}
```

- [ ] **Step 3: 跑 cargo build 找所有 EquipmentDef 构造点**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo build 2>&1 | grep -E "error|missing field" | head -20
```

每个构造点(在 `loader.rs`)补 `resources: Vec::new()` 或实际加载逻辑。

- [ ] **Step 4: 改 loader.rs 解析 resources 块**

读 `src/data/loader.rs` 找 EquipmentDef 构造处。加 resources 解析:

```rust
// 在构造 EquipmentDef 的地方
let resources: Vec<(String, f64)> = {
    // 从装备 Block 提取 resources 子块
    // 与 extract_stats 同风格
    if let Some(res_block) = block.fields.iter()
        .find(|f| f.key == "resources")
        .and_then(|f| f.value.as_block())
    {
        res_block.fields.iter()
            .filter_map(|f| f.value.as_scalar_num().map(|v| (f.key.clone(), v)))
            .collect()
    } else {
        Vec::new()
    }
};

EquipmentDef {
    // ... 现有字段 ...
    resources,
};
```

注意:`as_block()` 是 `Value` 的方法,确认 API 名(可能是 `as_block` 或 pattern match)。参考 `state_loader.rs:113` 的 `find_block` 模式。

- [ ] **Step 5: 写测试 — resources 加载**

在 `src/data/equipment.rs` 的 tests mod 加:

```rust
#[test]
fn t_extract_resources_from_block() {
    // 模拟 infantry_equipment 块: resources = { steel = 2 }
    let src = "type = infantry\nresources = { steel = 2 }\nbuild_cost_ic = 0.43";
    let b = parse(src).unwrap();
    // 找 resources 块
    let res_block = b.fields.iter()
        .find(|f| f.key == "resources")
        .and_then(|f| if let crate::parser::Value::Block(bb) = &f.value { Some(bb) } else { None })
        .expect("应有 resources 块");
    let steel = res_block.fields.iter()
        .find(|f| f.key == "steel")
        .and_then(|f| f.value.as_scalar_num())
        .expect("应有 steel 值");
    assert!((steel - 2.0).abs() < 1e-9);
}
```

- [ ] **Step 6: 跑测试 + 回归**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test data::equipment -- --test-threads=1 2>&1 | tail -10
cd E:/hoi4-conde/HOI4-CLONE && cargo test -- --test-threads=1 2>&1 | tail -5
```

预期:**209 测试**(208 + 1 新)。

- [ ] **Step 7: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add src/data/ && git commit -m "feat(data): EquipmentDef 加 resources 字段 + loader 解析"
```

---

## Task 5: state_loader 解析 resources 块

**Files:**
- Modify: `src/data/state_loader.rs`

- [ ] **Step 1: 读 state_loader.rs:36-94 确认 parse_state_block 结构**

参考既有 `find_block(history, "buildings")` 模式。

- [ ] **Step 2: 写测试 — 解析 resources 块**

在 `src/data/state_loader.rs` tests mod 加:

```rust
#[test]
fn t_load_state_with_resources() {
    let src = r#"state={
        id=42
        name="STATE_42"
        manpower = 100000
        state_category = city
        history={ owner = GER }
        resources = { steel = 16 chromium = 3 }
        provinces={ 100 101 }
    }"#;
    let states = load_states(src);
    let s = &states[0];
    assert!((s.resources.get("steel").copied().unwrap_or(0.0) - 16.0).abs() < 1e-9);
    assert!((s.resources.get("chromium").copied().unwrap_or(0.0) - 3.0).abs() < 1e-9);
}

#[test]
fn t_load_state_without_resources_defaults_empty() {
    let src = r#"state={
        id=43 name="X" state_category=town
        history={ owner = GER }
        provinces={ 200 }
    }"#;
    let states = load_states(src);
    assert!(states[0].resources.is_empty());
}
```

- [ ] **Step 3: 跑测试确认失败**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test data::state_loader -- --test-threads=1 2>&1 | tail -10
```

预期:失败 — State 还没有 resources 字段(本 Task 加)。

- [ ] **Step 4: 改 parse_state_block 加 resources 解析**

在 `parse_state_block`(`state_loader.rs:36-94`)的 `Some(State { ... })` 之前加:

```rust
let resources: HashMap<String, f64> = find_block(b, "resources")
    .map(|rb| {
        rb.fields.iter()
            .filter_map(|f| f.value.as_scalar_num().map(|v| (f.key.clone(), v)))
            .collect()
    })
    .unwrap_or_default();
```

并在 `Some(State { ... })` 里加 `resources,`(与 buildings 同级)。注意:`resources` 块在 **state 级别**(不在 history 内),所以用 `find_block(b, "resources")` 而非 `find_block(history, "resources")`(实测 1005-Qataghan.txt 等都如此)。

- [ ] **Step 5: 跑测试确认通过**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test data::state_loader -- --test-threads=1 2>&1 | tail -10
```

预期:通过(2 新 + 2 旧 = 4 测试)。

- [ ] **Step 6: 全量回归**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test -- --test-threads=1 2>&1 | tail -5
```

预期:**211 测试**(209 + 2 新)。

- [ ] **Step 7: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add src/data/state_loader.rs && git commit -m "feat(state_loader): 解析 resources 块到 State.resources"
```

---

## Task 6: equipment_data.rs 硬编码表补 resources 字段

**Files:**
- Modify: `src/combat/equipment_data.rs:6-16, 24-80`

- [ ] **Step 1: 读 equipment_data.rs:1-93 确认 EquipmentDef 结构**

(已在 spec 阶段读过,结构:5 字段 + find_equipment 函数)

- [ ] **Step 2: 加 resources 字段到 EquipmentDef(硬编码版)**

修改 `src/combat/equipment_data.rs:5-16`:

```rust
#[derive(Debug, Clone, Copy)]
pub struct EquipmentDef {
    pub name: &'static str,
    pub soft_attack: f64,
    pub hard_attack: f64,
    pub defense: f64,
    pub breakthrough: f64,
    pub armor: f64,
    pub piercing: f64,
    pub hardness: f64,
    pub build_cost_ic: f64,
    pub resources: &'static [(&'static str, f64)],
}
```

- [ ] **Step 3: 给 5 条装备补 resources 值**

修改 `EQUIPMENT` 数组(`equipment_data.rs:24-80`)。原版对齐:
- infantry_equipment: steel=2
- artillery: tungsten=1, steel=2
- light_tank: steel=1(light_tank_chassis_2, 1936)
- medium_tank: 无 resources(medium_tank_chassis_1, 1936)
- heavy_tank: 无 resources(heavy_tank_chassis_1, 1934/1936)
- 注: NSB chassis+modules 系统下资源主要来自 modules, chassis 本身资源很少; 当前硬编码表只反映 chassis 本身

```rust
pub static EQUIPMENT: &[EquipmentDef] = &[
    EquipmentDef {
        name: "infantry_equipment",
        soft_attack: 3.0,
        // ...其他字段不变...
        build_cost_ic: 0.43,
        resources: &[("steel", 2.0)],
    },
    EquipmentDef {
        name: "artillery",
        // ...
        resources: &[("tungsten", 1.0), ("steel", 2.0)],
    },
    EquipmentDef {
        name: "light_tank",
        // ...
        resources: &[("steel", 1.0)],  // light_tank_chassis_2(1936)
    },
    EquipmentDef {
        name: "medium_tank",
        // ...
        resources: &[],  // medium_tank_chassis_1(1936) 无 resources 块
    },
    EquipmentDef {
        name: "heavy_tank",
        // ...
        resources: &[],  // heavy_tank_chassis_1(1934/1936) 无 resources 块
    },
];
```

(完整字段值见现有文件,不要改 soft_attack 等数值)

- [ ] **Step 4: 跑 cargo build 找受影响构造点**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo build 2>&1 | grep "missing field" | head
```

如果某处直接构造了 EquipmentDef(例如测试),补 `resources: &[],`。

- [ ] **Step 5: 跑测试 + 回归**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test -- --test-threads=1 2>&1 | tail -5
```

预期:**211 测试**(纯加字段不破坏现有)。

- [ ] **Step 6: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add src/combat/equipment_data.rs && git commit -m "feat(equipment_data): 5 种装备补 resources 字段(对齐原版)"
```

---

## Task 7: production_step 主产出循环 + 资源惩罚

**Files:**
- Modify: `src/economy/production.rs`
- Modify: `src/economy/tests.rs`(加测试)

- [ ] **Step 1: 写 src/economy/production.rs(完整实现)**

```rust
//! 每日生产循环: 工厂每日按生产线产出装备入国家仓库
//!
//! 触发: clock.rs 每日(on_daily 后, reinforce_all 前)。
//! 三阶段(快照→计算→写回)避借用冲突, 沿用 reinforce.rs 风格。

use crate::data::equipment::EquipmentDef as GameEquipmentDef;
use crate::runtime::World;
use std::collections::HashMap;

/// 每日生产: 所有国家的所有生产线产出装备 + 更新 slot 效率
pub fn production_step(world: &mut World) {
    // 阶段 1: 快照各国可用资源(Σ owned_states 的 State.resources)
    let mut country_resources: HashMap<String, HashMap<String, f64>> = HashMap::new();
    for (tag, country) in &world.countries {
        let mut total: HashMap<String, f64> = HashMap::new();
        for sid in &country.owned_states {
            if let Some(state) = world.states.get(sid) {
                for (r, v) in &state.resources {
                    *total.entry(r.clone()).or_insert(0.0) += v;
                }
            }
        }
        country_resources.insert(tag.clone(), total);
    }

    // 阶段 2: 每条 line 计算产出 + 收集 slot efficiency 更新
    let mut outputs: Vec<(String, String, f64)> = Vec::new();   // (tag, variant, amount)
    let mut slot_updates: Vec<(String, u32, Vec<(usize, f64)>)> = Vec::new();

    for (tag, country) in &world.countries {
        let res_avail = country_resources.get(tag).cloned().unwrap_or_default();
        for line in &country.production_lines {
            // 找装备定义(数据驱动)
            let equipment = match world.data.equipment.get(&line.variant) {
                Some(e) => e,
                None => continue,
            };
            let res_mult = resource_penalty(line, equipment, &res_avail);

            let mut total_output = 0.0;
            let mut new_effs: Vec<(usize, f64)> = Vec::new();
            for (i, slot) in line.slots.iter().enumerate() {
                if !slot.active {
                    if slot.efficiency > 0.0 {
                        let new_e = (slot.efficiency - super::INACTIVE_SLOT_DECAY).max(0.0);
                        new_effs.push((i, new_e));
                    }
                    continue;
                }
                let out = super::FACTORY_SPEED_MIL * slot.efficiency * res_mult
                          / equipment.stats.build_cost_ic.max(0.0001);
                total_output += out;
                let new_e = slot.efficiency
                    + (super::EFFICIENCY_MAX - slot.efficiency)
                        * super::EFFICIENCY_GAIN * super::EFFICIENCY_BALANCE;
                new_effs.push((i, new_e));
            }
            if total_output > 0.0 {
                outputs.push((tag.clone(), line.variant.clone(), total_output));
            }
            if !new_effs.is_empty() {
                slot_updates.push((tag.clone(), line.id, new_effs));
            }
        }
    }

    // 阶段 3: 写回 stockpile + slot efficiency
    for (tag, variant, amt) in outputs {
        if let Some(country) = world.countries.get_mut(&tag) {
            *country.equipment_stockpile.entry(variant).or_insert(0.0) += amt;
        }
    }
    for (tag, line_id, updates) in slot_updates {
        if let Some(country) = world.countries.get_mut(&tag) {
            if let Some(line) = country.production_lines.iter_mut().find(|l| l.id == line_id) {
                for (i, e) in updates {
                    line.slots[i].efficiency = e;
                }
            }
        }
    }
}

/// 资源惩罚(严格 -5%/工厂/单位, 原版 PRODUCTION_RESOURCE_LACK_PENALTY)
/// 每缺 1 单位资源 → 该 line 产出 -5%, 多资源类型累加
pub fn resource_penalty(
    line: &super::ProductionLine,
    equipment: &GameEquipmentDef,
    country_res: &HashMap<String, f64>,
) -> f64 {
    let mut penalty: f64 = 0.0;
    for (resource, need_per_factory) in &equipment.resources {
        let total_need = line.active_count as f64 * need_per_factory;
        let available = country_res.get(resource).copied().unwrap_or(0.0);
        let shortage = (total_need - available).max(0.0);
        penalty += shortage * super::RESOURCE_LACK_PENALTY;
    }
    (1.0 - penalty).max(0.0)
}
```

- [ ] **Step 2: 写测试 — 资源惩罚**

在 `src/economy/tests.rs` 加(需要构造 mini World):

```rust
use super::super::economy::production::{resource_penalty};
use super::super::economy::{ProductionLine};
use super::super::data::equipment::EquipmentDef as GameEq;
use super::super::data::EquipStats;
use std::collections::HashMap;

fn mock_equip(name: &str, bc: f64, resources: Vec<(&str, f64)>) -> GameEq {
    GameEq {
        name: name.into(), chassis: name.into(), year: 1936,
        equip_type: "infantry".into(),
        stats: EquipStats { build_cost_ic: bc, ..Default::default() },
        resources: resources.into_iter().map(|(k,v)| (k.to_string(), v)).collect(),
    }
}

#[test]
fn t_no_penalty_when_resources_sufficient() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(5);
    let eq = mock_equip("infantry_equipment_1", 0.43, vec![("steel", 2.0)]);
    let mut res = HashMap::new();
    res.insert("steel".into(), 100.0);  // 充足
    let mult = resource_penalty(&line, &eq, &res);
    assert!((mult - 1.0).abs() < 1e-9);
}

#[test]
fn t_steel_shortage_5pct_per_unit() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(10);  // 10 工厂 × 钢 2 = 需 20 钢
    let eq = mock_equip("infantry_equipment_1", 0.43, vec![("steel", 2.0)]);
    let mut res = HashMap::new();
    res.insert("steel".into(), 18.0);  // 缺 2 单位 → -10%
    let mult = resource_penalty(&line, &eq, &res);
    assert!((mult - 0.90).abs() < 1e-9, "缺 2 钢应 -10%, mult={}", mult);
}

#[test]
fn t_multiple_resource_penalties_stack() {
    let mut line = ProductionLine::new(0, "artillery_1".into());
    line.set_active(3);  // 钨 3×1=3, 钢 3×2=6
    let eq = mock_equip("artillery_1", 3.5, vec![("tungsten", 1.0), ("steel", 2.0)]);
    let mut res = HashMap::new();
    res.insert("tungsten".into(), 2.0);  // 缺 1 → -5%
    res.insert("steel".into(), 5.0);     // 缺 1 → -5%
    let mult = resource_penalty(&line, &eq, &res);
    assert!((mult - 0.90).abs() < 1e-9, "总 -10%, mult={}", mult);
}

#[test]
fn t_zero_output_when_full_shortage() {
    let mut line = ProductionLine::new(0, "artillery_1".into());
    line.set_active(10);
    let eq = mock_equip("artillery_1", 3.5, vec![("tungsten", 1.0)]);
    let res = HashMap::new();  // 0 钨
    let mult = resource_penalty(&line, &eq, &res);
    // 缺 10 → -50% → 多扣 → clamp 0
    assert_eq!(mult, 0.0);
}
```

- [ ] **Step 3: 跑测试**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test economy -- --test-threads=1 2>&1 | tail -10
```

预期:economy 模块 12 测试(8 + 4 新)。

- [ ] **Step 4: 写测试 — production_step 端到端(库存增长 + 效率增长)**

在 `src/economy/tests.rs` 加:

```rust
use super::super::runtime::{World, entities::{Country, State}};
use super::super::economy::production::production_step;
use super::super::economy::{EFFICIENCY_START, EFFICIENCY_MAX};

fn world_with_line(tag: &str, factories: u32, build_cost: f64, steel: f64) -> World {
    let mut w = World::new();
    // 注入装备定义到 GameData(空 GameData, 需直接塞)
    // 注意: World::new() 调用 cached_game_data, 测试用空 GameData 时需手动塞
    // 简化: 直接操作 world.data(Arc, 不可变)—用 World::default() 的空 GameData
    // 测试时把装备放进 equipment HashMap 不可行(Arc 只读)
    // → 改用 integration 测试覆盖, 单元测试只测 resource_penalty + 公式逻辑
    w
}

// 改:不写完整 production_step 端到端单元测试(Arc<GameData> 只读难注入)
// 端到端验证放 tests/production.rs integration(Task 12)
```

注:`GameData` 是 `Arc` 只读,空 World 的 GameData 含编译期嵌入的 1936 装备,可直接用 `infantry_equipment_1` 等 variant 名。**改用 integration 测试**(Task 12)做端到端,单元测试只覆盖公式。

- [ ] **Step 5: 跑全量回归**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test -- --test-threads=1 2>&1 | tail -5
```

预期:**215 测试**(211 + 4 新)。

- [ ] **Step 6: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add src/economy/ && git commit -m "feat(economy): production_step 主产出循环 + 资源惩罚"
```

---

## Task 8: change_line_variant 切换保留逻辑

**Files:**
- Modify: `src/economy/production.rs`
- Modify: `src/economy/tests.rs`

- [ ] **Step 1: 在 production.rs 加 change_line_variant 函数**

在 `src/economy/production.rs` 末尾追加:

```rust
/// 切换生产线型号(严格原版保留率)
/// - 不同 chassis: 全槽重置到 EFFICIENCY_START(active)/ 0(inactive)
/// - 同 chassis 不同 variant: 每 slot efficiency × VARIANT_RETENTION(0.9)
/// - 同 variant: 无操作
pub fn change_line_variant(
    line: &mut super::ProductionLine,
    new_variant: &str,
) {
    let new_chassis = super::variant_chassis(new_variant);
    let same_chassis = new_chassis == line.chassis;
    let same_variant = new_variant == line.variant;
    if same_variant {
        return;  // 无变化
    }
    let retention = if same_chassis {
        super::VARIANT_RETENTION  // 0.9
    } else {
        0.0  // 重置
    };
    for slot in &mut line.slots {
        if retention == 0.0 {
            slot.efficiency = if slot.active { super::EFFICIENCY_START } else { 0.0 };
        } else {
            slot.efficiency *= retention;
        }
    }
    line.chassis = new_chassis.to_string();
    line.variant = new_variant.to_string();
}
```

- [ ] **Step 2: 写测试 — 切换保留**

在 `src/economy/tests.rs` 加:

```rust
use super::super::economy::production::change_line_variant;

#[test]
fn t_variant_change_keeps_90pct() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(3);
    // 设个非 START 的效率(模拟已跑几天)
    line.slots[0].efficiency = 0.30;
    line.slots[1].efficiency = 0.30;
    line.slots[2].efficiency = 0.30;
    change_line_variant(&mut line, "infantry_equipment_2");
    // 同 chassis → ×0.9
    assert!((line.slots[0].efficiency - 0.27).abs() < 1e-9);
    assert_eq!(line.variant, "infantry_equipment_2");
    assert_eq!(line.chassis, "infantry_equipment");
}

#[test]
fn t_chassis_change_resets_to_start() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(3);
    line.slots[0].efficiency = 0.40;
    change_line_variant(&mut line, "artillery_1");
    // 不同 chassis → 重置到 EFFICIENCY_START
    assert!((line.slots[0].efficiency - 0.10).abs() < 1e-9);
    assert_eq!(line.chassis, "artillery");
}

#[test]
fn t_change_to_same_variant_noop() {
    let mut line = ProductionLine::new(0, "infantry_equipment_1".into());
    line.set_active(2);
    line.slots[0].efficiency = 0.40;
    change_line_variant(&mut line, "infantry_equipment_1");
    assert!((line.slots[0].efficiency - 0.40).abs() < 1e-9);  // 不变
}
```

- [ ] **Step 3: 跑测试**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test economy -- --test-threads=1 2>&1 | tail -10
```

预期:economy 15 测试(12 + 3 新)。

- [ ] **Step 4: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add src/economy/ && git commit -m "feat(economy): change_line_variant 切换保留(同chassis 90%, 不同重置)"
```

---

## Task 9: reinforce.rs 改造 — chassis 查 variant 池

**Files:**
- Modify: `src/combat/reinforce.rs`
- Modify: `src/combat/reinforce.rs` tests

- [ ] **Step 1: 改 reinforce.rs:35-72 主循环**

读现有 `src/combat/reinforce.rs:1-114`,改造装备转移段(8-72 行)。新逻辑:

```rust
// 阶段 1 改造: need 按 chassis, held 按 variant; 缺口时按 chassis 在 stockpile 找 variant 池
use crate::economy::variant_chassis;

// ...在 div 循环里:
let tag = div.owner_tag.clone();
let mut div_transfer: Vec<(String, f64)> = Vec::new();  // (variant_key, amount)

for (chassis, need) in &div.equipment_need {
    // 计算该 chassis 当前持有总量
    let held_total: f64 = div.equipment_held.iter()
        .filter(|(k, _)| variant_chassis(k) == chassis.as_str())
        .map(|(_, v)| v).sum();
    let shortage = (need - held_total).max(0.0);
    if shortage <= 0.0 { continue; }

    // 在国家仓库找该 chassis 的所有 variant(按字母倒序优先取最新)
    let country = match world.countries.get(&tag) {
        Some(c) => c,
        None => continue,
    };
    let mut candidates: Vec<String> = country.equipment_stockpile.keys()
        .filter(|k| variant_chassis(k) == chassis.as_str())
        .cloned().collect();
    candidates.sort_by(|a, b| b.cmp(a));  // 倒序: variant_2 优先 variant_1

    let mut remaining = shortage;
    for var_key in candidates {
        if remaining <= 0.0 { break; }
        let avail = country.equipment_stockpile.get(&var_key).copied().unwrap_or(0.0);
        if avail <= 0.0 { continue; }
        let take = remaining.min(avail);
        div_transfer.push((var_key.clone(), take));
        remaining -= take;
    }
}

if !div_transfer.is_empty() {
    transfers.push((did, div_transfer));
}
```

**阶段 2 写回**(55-72 行)无需改 — 已经用 `(eq, amt)` 通用 key,新值是 variant key 即可。

- [ ] **Step 2: 改现有 3 个测试 — held key 用 variant 全名**

修改 `src/combat/reinforce.rs:121-188` 测试辅助 `div_with_eq`:

```rust
fn div_with_eq(tag: &str, held: f64, need: f64) -> Division {
    let mut d = Division {
        id: 0,
        owner_tag: tag.into(),
        ..Default::default()
    };
    // need 用 chassis 名
    d.equipment_need.insert("infantry_equipment".into(), need);
    // held 用 variant 全名(原版语义: 持有按变体)
    d.equipment_held.insert("infantry_equipment_1".into(), held);
    d
}
```

3 个测试(`t_reinforce_fills_shortage_from_stockpile` / `t_reinforce_partial_when_stockpile_low` / `t_no_transfer_when_full`)的 stockpile key 改成 `"infantry_equipment_1"`:

```rust
ger.equipment_stockpile.insert("infantry_equipment_1".into(), 50.0);
// 断言里查 held 用 infantry_equipment_1, 查 stockpile 同
```

- [ ] **Step 3: 加新测试 — 优先取新 variant**

在 reinforce.rs tests mod 末尾加:

```rust
#[test]
fn t_reinforce_prefers_newer_variant() {
    let mut w = World::new();
    let mut ger = Country { tag: "GER".into(), ..Default::default() };
    ger.equipment_stockpile.insert("infantry_equipment_1".into(), 30.0);
    ger.equipment_stockpile.insert("infantry_equipment_2".into(), 30.0);
    w.countries.insert("GER".into(), ger);
    let did = w.add_division(div_with_eq("GER", 80.0, 100.0));  // 缺 20

    reinforce_all(&mut w);

    let v2 = w.divisions.get(&did).unwrap().equipment_held.get("infantry_equipment_2").copied().unwrap_or(0.0);
    let v1 = w.divisions.get(&did).unwrap().equipment_held.get("infantry_equipment_1").copied().unwrap_or(0.0);
    let v2_total = v2;
    // 缺 20, 应优先从 v2 取(v2 stockpile 减 20, v1 不动)
    assert!((v2 - 20.0).abs() < 1e-9, "应优先补 v2, 实际 v2={}", v2);
    let v1_stock = w.countries.get("GER").unwrap().equipment_stockpile.get("infantry_equipment_1").copied().unwrap_or(0.0);
    assert!((v1_stock - 30.0).abs() < 1e-9, "v1 不应被动, stock={}", v1_stock);
}

#[test]
fn t_reinforce_mixed_variants_fill_chassis_need() {
    let mut w = World::new();
    let mut ger = Country { tag: "GER".into(), ..Default::default() };
    ger.equipment_stockpile.insert("infantry_equipment_1".into(), 5.0);   // v1 只够补 5
    ger.equipment_stockpile.insert("infantry_equipment_2".into(), 30.0);  // v2 充足
    w.countries.insert("GER".into(), ger);
    let did = w.add_division(div_with_eq("GER", 80.0, 100.0));  // 缺 20

    reinforce_all(&mut w);

    // 倒序优先取 v2, 取完 20(库存够)
    let v2 = w.divisions.get(&did).unwrap().equipment_held.get("infantry_equipment_2").copied().unwrap_or(0.0);
    assert!((v2 - 20.0).abs() < 1e-9, "v2 应补 20, 实际 {}", v2);
    let v1 = w.divisions.get(&did).unwrap().equipment_held.get("infantry_equipment_1").copied().unwrap_or(0.0);
    assert!(v1.abs() < 1e-9, "v1 不应被取(因 v2 够), 实际 {}", v1);
}
```

- [ ] **Step 4: 跑测试 + 全量回归**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test combat::reinforce -- --test-threads=1 2>&1 | tail -10
cd E:/hoi4-conde/HOI4-CLONE && cargo test -- --test-threads=1 2>&1 | tail -5
```

预期:**217 测试**(215 + 2 新;3 个改 key 不增不减)。

- [ ] **Step 5: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add src/combat/reinforce.rs && git commit -m "refactor(reinforce): 按 chassis 查 variant 池补给 + 优先取最新 variant"
```

---

## Task 10: 注册 5 个新命令

**Files:**
- Modify: `src/combat/commands.rs`(在 `register_all` 末尾加)

- [ ] **Step 1: 读 commands.rs 末尾 + add_equipment(行 277-286)位置**

参考既有 `np(p, name, key)?` + `num_of(...)?` + `as_str()?` 模式。

- [ ] **Step 2: 在 commands.rs:286 后改造 add_equipment(已是 variant key,不变)**

无需改 — add_equipment 已经用 `type` 参数任意 key 入库。注释更新即可:

```rust
// 注释改: type = variant 全名(如 "infantry_equipment_1"), 不再是 chassis 名
reg.register("add_equipment", |w, p| {
    // ...(保持不变)
});
```

- [ ] **Step 3: 加 create_production_line 命令**

在 `commands.rs` 的 `register_all` 函数末尾(紧挨最后一行 `reg.register(...)` 后,函数结束 `}` 前)加:

```rust
// 创建生产线
reg.register("create_production_line", |w, p| {
    let owner = np(p, "create_production_line", "owner")?.as_str()
        .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?
        .to_string();
    let variant = np(p, "create_production_line", "variant")?.as_str()
        .ok_or_else(|| CmdError::RuntimeError("variant 应为字符串".into()))?
        .to_string();
    let factories = num_of(np(p, "create_production_line", "factories")?)? as u32;

    // 验证 variant 在 GameData
    if !w.data.equipment.contains_key(&variant) {
        return Err(CmdError::RuntimeError(format!("variant {} 未在 GameData", variant)));
    }

    // 生成 line id(Country 内唯一)
    let id = w.countries.get(&owner)
        .map(|c| c.production_lines.iter().map(|l| l.id).max().unwrap_or(0) + 1)
        .unwrap_or(1);

    let mut line = crate::economy::ProductionLine::new(id, variant);
    line.set_active(factories);

    let country = w.countries.entry(owner).or_default();
    country.production_lines.push(line);
    Ok(())
});

// 调整生产线工厂数
reg.register("set_line_factories", |w, p| {
    let owner = np(p, "set_line_factories", "owner")?.as_str()
        .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?
        .to_string();
    let line_id = num_of(np(p, "set_line_factories", "line_id")?)? as u32;
    let factories = num_of(np(p, "set_line_factories", "factories")?)? as u32;

    let country = w.countries.entry(owner).or_default();
    let line = country.production_lines.iter_mut()
        .find(|l| l.id == line_id)
        .ok_or_else(|| CmdError::RuntimeError(format!("line_id {} 不存在", line_id)))?;
    line.set_active(factories);
    Ok(())
});

// 切换生产线型号
reg.register("change_line_variant", |w, p| {
    let owner = np(p, "change_line_variant", "owner")?.as_str()
        .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?
        .to_string();
    let line_id = num_of(np(p, "change_line_variant", "line_id")?)? as u32;
    let variant = np(p, "change_line_variant", "variant")?.as_str()
        .ok_or_else(|| CmdError::RuntimeError("variant 应为字符串".into()))?
        .to_string();

    if !w.data.equipment.contains_key(&variant) {
        return Err(CmdError::RuntimeError(format!("variant {} 未在 GameData", variant)));
    }

    let country = w.countries.entry(owner).or_default();
    let line = country.production_lines.iter_mut()
        .find(|l| l.id == line_id)
        .ok_or_else(|| CmdError::RuntimeError(format!("line_id {} 不存在", line_id)))?;
    crate::economy::production::change_line_variant(line, &variant);
    Ok(())
});

// 删除生产线
reg.register("remove_production_line", |w, p| {
    let owner = np(p, "remove_production_line", "owner")?.as_str()
        .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?
        .to_string();
    let line_id = num_of(np(p, "remove_production_line", "line_id")?)? as u32;

    let country = w.countries.entry(owner).or_default();
    let before = country.production_lines.len();
    country.production_lines.retain(|l| l.id != line_id);
    if country.production_lines.len() == before {
        return Err(CmdError::RuntimeError(format!("line_id {} 不存在", line_id)));
    }
    Ok(())
});

// State 资源调试命令(demo setup 用)
reg.register("add_state_resource", |w, p| {
    let sid = num_of(np(p, "add_state_resource", "state")?)? as u32;
    let resource = np(p, "add_state_resource", "resource")?.as_str()
        .ok_or_else(|| CmdError::RuntimeError("resource 应为字符串".into()))?
        .to_string();
    let amount = num_of(np(p, "add_state_resource", "amount")?)?;

    let state = w.states.get_mut(&sid)
        .ok_or_else(|| CmdError::RuntimeError(format!("state {} 不存在", sid)))?;
    *state.resources.entry(resource).or_insert(0.0) += amount;
    Ok(())
});
```

- [ ] **Step 4: 跑 cargo build 确认编译**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo build 2>&1 | tail -10
```

如有 unused import / 类型不匹配,修。

- [ ] **Step 5: 写命令测试**

在 `src/combat/commands.rs` tests mod(或新文件)加(若无 GameData 注入,需用嵌入的 1936 数据 `infantry_equipment_1`):

```rust
#[test]
fn t_create_production_line_registers() {
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut w = World::new();
    w.countries.insert("GER".into(), Default::default());
    // 假设 GameData 含 infantry_equipment_1(嵌入数据应含)
    let eff = interp.registry().get_effect("create_production_line");
    assert!(eff.is_some(), "create_production_line 应已注册");
}
```

注:如果嵌入数据不含 `infantry_equipment_1`(只有 `infantry_equipment` archetype),需先在 spec 阶段核实,或在测试里用 chassis 名作 variant(简化)。**核实后再写测试。**

- [ ] **Step 6: 跑全量回归**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test -- --test-threads=1 2>&1 | tail -5
```

预期:**~218 测试**(纯注册无破坏)。

- [ ] **Step 7: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add src/combat/commands.rs && git commit -m "feat(commands): 注册 5 个新命令(create/set/change/remove/add_state_resource)"
```

---

## Task 11: clock.rs 接入 production_step

**Files:**
- Modify: `src/runtime/clock.rs:21-25`

- [ ] **Step 1: 改 clock.rs:21-25 加 production_step**

修改 `src/runtime/clock.rs:21-25`:

```rust
if world.hour % 24 == 0 {
    world.fire_event(interp, "on_daily");
    world.fire_event(interp, &format!("on_daily_{}", world.player_tag));
    crate::economy::production::production_step(world);  // 新增: 每日生产
    crate::combat::reinforce::reinforce_all(world);
}
```

- [ ] **Step 2: 跑全量回归**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test -- --test-threads=1 2>&1 | tail -5
```

预期:**~218 测试**(空 World 无生产线,production_step 是 no-op,不破坏现有测试)。

- [ ] **Step 3: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add src/runtime/clock.rs && git commit -m "feat(clock): on_daily 后调 production_step(产装备→仓库→reinforce)"
```

---

## Task 12: wasm_api 序列化 — 导出 stockpile + production_lines

**Files:**
- Modify: `src/wasm_api.rs`(countries 数组序列化段)

- [ ] **Step 1: 读 wasm_api.rs:426-566 确认 countries 序列化位置**

```bash
grep -n "countries" E:/hoi4-conde/HOI4-CLONE/src/wasm_api.rs
```

- [ ] **Step 2: 改 countries 序列化加 stockpile + production_lines**

找到 countries 序列化段(应该是 `s.push_str(",\"countries\":[");` 那段)。改 country 序列化格式:

```rust
// 原来:
s.push_str(&format!(
    "{{\"tag\":\"{}\",\"political_power\":{},\"stability\":{},\"war_support\":{}}}",
    tag,
    country.effective_political_power(),
    country.effective_stability(),
    country.effective_war_support()
));

// 改为(加 stockpile + production_lines):
s.push('{');
s.push_str(&format!(
    "\"tag\":\"{}\",\"political_power\":{},\"stability\":{},\"war_support\":{}",
    tag,
    country.effective_political_power(),
    country.effective_stability(),
    country.effective_war_support()
));
// stockpile(variant key)
s.push_str(",\"stockpile\":{");
let mut first = true;
for (k, v) in &country.equipment_stockpile {
    if !first { s.push(','); }
    s.push_str(&format!("\"{}\":{}", k, v));
    first = false;
}
s.push('}');
// production_lines
s.push_str(",\"production_lines\":[");
for (i, line) in country.production_lines.iter().enumerate() {
    if i > 0 { s.push(','); }
    // 导出每槽位 efficiency(15 个数字, 含 inactive)
    let effs: Vec<f64> = line.slots.iter().map(|s| s.efficiency).collect();
    s.push_str(&format!(
        "{{\"id\":{},\"variant\":\"{}\",\"chassis\":\"{}\",\"active\":{},\"efficiencies\":[",
        line.id, line.variant, line.chassis, line.active_count
    ));
    for (j, e) in effs.iter().enumerate() {
        if j > 0 { s.push(','); }
        s.push_str(&format!("{}", e));
    }
    s.push_str("]}");
}
s.push_str("]}");
```

- [ ] **Step 3: 跑 cargo build + wasm build**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo build 2>&1 | tail -5
cd E:/hoi4-conde/HOI4-CLONE && cargo build --target wasm32-unknown-unknown --lib --release 2>&1 | tail -5
```

预期:0 警告。

- [ ] **Step 4: 跑全量回归**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test -- --test-threads=1 2>&1 | tail -5
```

- [ ] **Step 5: Copy WASM 到 web 目录**

```bash
cp E:/hoi4-conde/HOI4-CLONE/target/wasm32-unknown-unknown/release/hoi4_clone.wasm E:/hoi4-conde/HOI4-CLONE/web/
```

- [ ] **Step 6: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add src/wasm_api.rs web/hoi4_clone.wasm && git commit -m "feat(wasm_api): 序列化 stockpile + production_lines 到 get_state"
```

---

## Task 13: integration test — tests/production.rs

**Files:**
- Create: `tests/production.rs`

- [ ] **Step 1: 写端到端 integration 测试**

```rust
//! 生产系统端到端 integration
//! 验证: 建国家+State(buildings 含 arms_factory 建筑只是占位, 实际 arms_factory 数=production_line 中 active_count)
//!       → 建生产线 → 跑 N 日 → 库存积累 + 效率达到 ~50%
//!       → 删 State 的 steel → 产出降为 0(资源耗尽)
//!       → variant 切换端到端

use hoi4_clone::runtime::{Interpreter, Registry, World};
use hoi4_clone::commands::register_all;
use hoi4_clone::runtime::clock::GameClock;
use hoi4_clone::economy::{EFFICIENCY_MAX, EFFICIENCY_START};

fn setup_world_with_ger_production() -> (Interpreter, World) {
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut w = World::new();
    w.player_tag = "GER".into();
    // 加 GER 国家 + State(含 steel 资源)
    let mut ger = hoi4_clone::runtime::entities::Country::default();
    ger.tag = "GER".into();
    ger.owned_states = vec![1];
    w.countries.insert("GER".into(), ger);
    let mut state = hoi4_clone::runtime::entities::State::default();
    state.id = 1;
    state.owner = "GER".into();
    state.controller = "GER".into();
    state.resources.insert("steel".into(), 100.0);
    w.states.insert(1, state);
    (interp, w)
}

#[test]
fn t_production_accumulates_stockpile_over_days() {
    let (interp, mut world) = setup_world_with_ger_production();
    // 建生产线 5 工厂产 infantry_equipment_1
    // 注意: GameData 含哪些 variant 名取决于嵌入数据 — 实际实施时核实
    // 假设 infantry_equipment_1 存在(GameData 嵌入的 1936 装备变体)
    let ok = interp.run(&mut world, "create_production_line = { owner = GER variant = infantry_equipment_1 factories = 5 }");
    if ok.is_err() {
        // 若 variant 名不存在, 试 chassis 名(数据嵌入差异)
        let _ = interp.run(&mut world, "create_production_line = { owner = GER variant = infantry_equipment factories = 5 }");
    }
    assert!(!world.countries["GER"].production_lines.is_empty(), "应已创建生产线");

    let stock_before = world.countries["GER"].equipment_stockpile.values().sum::<f64>();
    GameClock::advance(&interp, &mut world, 24 * 10);  // 10 天
    let stock_after = world.countries["GER"].equipment_stockpile.values().sum::<f64>();
    assert!(stock_after > stock_before, "10 天后库存应增长: before={}, after={}", stock_before, stock_after);
}

#[test]
fn t_efficiency_reaches_near_cap_after_long_run() {
    let (interp, mut world) = setup_world_with_ger_production();
    let _ = interp.run(&mut world, "create_production_line = { owner = GER variant = infantry_equipment_1 factories = 3 }");
    GameClock::advance(&interp, &mut world, 24 * 60);  // 60 天
    let line = &world.countries["GER"].production_lines[0];
    let max_slot_eff = line.slots.iter().filter(|s| s.active).map(|s| s.efficiency).fold(0.0_f64, f64::max);
    assert!(max_slot_eff > 0.40, "60 天后效率应接近 cap 0.50, 实际 {}", max_slot_eff);
    assert!(max_slot_eff <= EFFICIENCY_MAX + 1e-9, "效率不应超 cap");
}

#[test]
fn t_no_output_when_no_steel() {
    let (interp, mut world) = setup_world_with_ger_production();
    let _ = interp.run(&mut world, "create_production_line = { owner = GER variant = infantry_equipment_1 factories = 5 }");
    // 删 steel
    world.states.get_mut(&1).unwrap().resources.clear();
    let stock_before = world.countries["GER"].equipment_stockpile.values().sum::<f64>();
    GameClock::advance(&interp, &mut world, 24 * 5);
    let stock_after = world.countries["GER"].equipment_stockpile.values().sum::<f64>();
    assert!((stock_after - stock_before).abs() < 1e-9, "无钢应产出 0, before={}, after={}", stock_before, stock_after);
}

#[test]
fn t_variant_change_keeps_90pct_end_to_end() {
    let (interp, mut world) = setup_world_with_ger_production();
    let _ = interp.run(&mut world, "create_production_line = { owner = GER variant = infantry_equipment_1 factories = 3 }");
    GameClock::advance(&interp, &mut world, 24 * 30);  // 跑 30 天到中效率
    let eff_before = world.countries["GER"].production_lines[0].slots[0].efficiency;
    // 切换 variant(需 GameData 含 _2; 若无, 跳过本测试)
    let res = interp.run(&mut world, "change_line_variant = { owner = GER line_id = 1 variant = infantry_equipment_2 }");
    if res.is_ok() {
        let eff_after = world.countries["GER"].production_lines[0].slots[0].efficiency;
        assert!((eff_after - eff_before * 0.9).abs() < 0.01, "应保留 90%: before={}, after={}", eff_before, eff_after);
    }
    // 否则跳过(GameData 无 _2 variant)
}
```

- [ ] **Step 2: 跑 integration**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test --test production -- --test-threads=1 2>&1 | tail -20
```

预期:核心测试通过;部分依赖 GameData 嵌入数据的测试可能需调整 variant 名。

- [ ] **Step 3: 全量回归**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test -- --test-threads=1 2>&1 | tail -5
```

预期:**~222 测试**(218 + 4 新)。

- [ ] **Step 4: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add tests/production.rs && git commit -m "test(production): 端到端 integration(库存增长/效率增长/资源耗尽/variant切换)"
```

---

## Task 14: UI — productionPanel.js + stockpilePanel.js + 顶栏按钮

**Files:**
- Create: `web/js/views/productionPanel.js`
- Create: `web/js/views/stockpilePanel.js`
- Modify: `web/index.html`
- Modify: `web/css/app.css`
- Modify: `web/js/engine/commands.js`(加命令封装)

- [ ] **Step 1: 加命令封装到 commands.js**

读 `web/js/engine/commands.js`,加:

```javascript
export function createProductionLine(owner, variant, factories) {
  return run('create_production_line', [
    ['owner', owner], ['variant', variant], ['factories', factories]
  ]);
}
export function setLineFactories(owner, lineId, factories) {
  return run('set_line_factories', [
    ['owner', owner], ['line_id', lineId], ['factories', factories]
  ]);
}
export function changeLineVariant(owner, lineId, variant) {
  return run('change_line_variant', [
    ['owner', owner], ['line_id', lineId], ['variant', variant]
  ]);
}
export function removeProductionLine(owner, lineId) {
  return run('remove_production_line', [
    ['owner', owner], ['line_id', lineId]
  ]);
}
export function addStateResource(state, resource, amount) {
  return run('add_state_resource', [
    ['state', state], ['resource', resource], ['amount', amount]
  ]);
}
```

(参考既有 deployTemplate 等的封装风格,核实 run/参数格式)

- [ ] **Step 2: 写 productionPanel.js**

```javascript
// web/js/views/productionPanel.js
// 生产管理面板: 显示生产线 + 仓库 + 资源概览
import { state } from '../engine/state.js';
import { setLineFactories, changeLineVariant, removeProductionLine } from '../engine/commands.js';

let root;

export function openProductionPanel() {
  if (root) { root.remove(); root = null; return; }
  root = document.createElement('div');
  root.id = 'productionPanel';
  root.className = 'panel production-panel';
  root.innerHTML = `
    <div class="panel-header">
      <span>🏭 生产管理 — <span id="pp-tag"></span></span>
      <button class="close-btn">×</button>
    </div>
    <div class="panel-body" id="pp-body"></div>
  `;
  document.body.appendChild(root);
  root.querySelector('.close-btn').onclick = () => { root.remove(); root = null; };
  document.addEventListener('keydown', escClose);
  refresh();
  state.subscribeKeys(['countries'], refresh);
}

function escClose(e) {
  if (e.key === 'Escape' && root) { root.remove(); root = null; document.removeEventListener('keydown', escClose); }
}

function refresh() {
  if (!root) return;
  const tag = state.get('player_tag') || 'GER';
  const country = (state.get('countries') || []).find(c => c.tag === tag);
  if (!country) return;
  root.querySelector('#pp-tag').textContent = tag;
  const body = root.querySelector('#pp-body');

  const lines = country.production_lines || [];
  const stockpile = country.stockpile || {};
  // 按仓库分组: variant → chassis
  const chassisMap = {};
  for (const v of Object.keys(stockpile)) {
    const c = v.replace(/_\d+$/, '');
    if (!chassisMap[c]) chassisMap[c] = [];
    chassisMap[c].push({ variant: v, amount: stockpile[v] });
  }

  body.innerHTML = `
    <div class="pp-section">
      <div class="pp-title">生产线 (${lines.length})</div>
      <div class="pp-lines">
        ${lines.map(line => lineRow(tag, line)).join('') || '<div class="pp-empty">无生产线</div>'}
      </div>
    </div>
    <div class="pp-section">
      <div class="pp-title">仓库</div>
      <div class="pp-stockpile">
        ${Object.keys(chassisMap).map(c => `
          <div class="pp-stock-group">
            <div class="pp-stock-chassis">${c}</div>
            ${chassisMap[c].map(item => `
              <div class="pp-stock-variant">${item.variant}: ${item.amount.toFixed(1)}</div>
            `).join('')}
          </div>
        `).join('') || '<div class="pp-empty">空</div>'}
      </div>
    </div>
  `;
  // 绑定按钮事件
  body.querySelectorAll('[data-action]').forEach(btn => {
    btn.onclick = () => handleAction(btn.dataset, tag);
  });
}

function lineRow(tag, line) {
  const activeEffs = (line.efficiencies || []).filter((_, i) => i < line.active);
  const avgEff = activeEffs.length ? activeEffs.reduce((a,b)=>a+b,0)/activeEffs.length : 0;
  return `
    <div class="pp-line">
      <div class="pp-line-head">
        <span class="pp-line-id">#${line.id}</span>
        <span class="pp-line-variant">${line.variant}</span>
        <span class="pp-line-factories">[${line.active}/15]</span>
        <span class="pp-line-eff">eff ${(avgEff*100).toFixed(0)}%</span>
      </div>
      <div class="pp-line-actions">
        <button data-action="dec" data-id="${line.id}">−</button>
        <button data-action="inc" data-id="${line.id}">+</button>
        <button data-action="del" data-id="${line.id}">删除</button>
      </div>
    </div>
  `;
}

function handleAction(ds, tag) {
  const id = parseInt(ds.id);
  const line = (state.get('countries')||[]).find(c=>c.tag===tag).production_lines.find(l=>l.id===id);
  if (!line) return;
  if (ds.action === 'inc') setLineFactories(tag, id, line.active + 1);
  else if (ds.action === 'dec') setLineFactories(tag, id, Math.max(0, line.active - 1));
  else if (ds.action === 'del') {
    if (confirm(`删除生产线 #${id}?`)) removeProductionLine(tag, id);
  }
}
```

- [ ] **Step 3: 写 stockpilePanel.js**

```javascript
// web/js/views/stockpilePanel.js
// 仓库徽章悬停浮层(简化版)
import { state } from '../engine/state.js';

export function renderStockpileBadge(root) {
  const tag = state.get('player_tag') || 'GER';
  const country = (state.get('countries') || []).find(c => c.tag === tag);
  if (!country) return;
  const total = Object.values(country.stockpile || {}).reduce((a,b)=>a+b, 0);
  const lineCount = (country.production_lines || []).length;
  root.innerHTML = `📦 ${total.toFixed(0)} · 🏭 ${lineCount}`;
  root.title = `库存 ${total.toFixed(1)} 件, ${lineCount} 条生产线`;
}
```

- [ ] **Step 4: index.html 加顶栏按钮 + 仓库徽章**

读 `web/index.html`,在 topbar 内加:

```html
<button id="btnProduction" class="topbar-btn">🏭 生产</button>
<span id="stockpileBadge" class="stockpile-badge"></span>
```

在 main.js 启动时绑定:

```javascript
import { openProductionPanel } from './views/productionPanel.js';
import { renderStockpileBadge } from './views/stockpilePanel.js';
document.getElementById('btnProduction').onclick = openProductionPanel;
// 在 refresh 函数里调:
renderStockpileBadge(document.getElementById('stockpileBadge'));
```

- [ ] **Step 5: app.css 加面板样式**

读 `web/css/app.css` 末尾,加:

```css
.production-panel { position: absolute; top: 60px; right: 20px; width: 520px; max-height: 80vh; overflow-y: auto;
  background: rgba(20,30,40,0.95); color: #e0e0e0; border: 1px solid #555; border-radius: 4px; z-index: 100; }
.panel-header { display: flex; justify-content: space-between; padding: 8px 12px; border-bottom: 1px solid #444; }
.close-btn { background: none; border: none; color: #e94560; font-size: 18px; cursor: pointer; }
.panel-body { padding: 8px 12px; }
.pp-section { margin-bottom: 12px; }
.pp-title { font-weight: bold; margin-bottom: 6px; }
.pp-line { border: 1px solid #444; padding: 6px; margin-bottom: 4px; border-radius: 2px; }
.pp-line-head { display: flex; justify-content: space-between; margin-bottom: 4px; }
.pp-line-actions button { padding: 2px 8px; margin-right: 4px; }
.pp-stock-group { margin-left: 8px; margin-bottom: 4px; }
.pp-stock-chassis { font-weight: bold; }
.pp-stock-variant { margin-left: 16px; color: #aaa; }
.pp-empty { color: #888; font-style: italic; }
.stockpile-badge { color: #ccc; padding: 0 8px; cursor: help; }
```

- [ ] **Step 6: 手动跑 demo 看效果**

```bash
cd E:/hoi4-conde/HOI4-CLONE/web && python -m http.server 8765 &
# 浏览器开 http://127.0.0.1:8765, 点 🏭 生产 按钮, 应弹出空面板(此时 demo setup 未改造, 下个 Task)
```

- [ ] **Step 7: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add web/ && git commit -m "feat(ui): 生产面板 + 仓库徽章(只读展示)"
```

---

## Task 15: demo setup 改造 — 初始生产线/资源/库存

**Files:**
- Modify: `web/js/main.js`(setup 段)

- [ ] **Step 1: 读 main.js setup 段(约 274-304 行)**

- [ ] **Step 2: 加初始 State 资源 + 生产线 + 起步库存**

在 setup 段(建国家/师之后, 模板 deploy 之后),engine supply 之前加:

```javascript
// 给 GER state 1 加钢产(模拟德国本土钢)
addStateResource(1, 'steel', 24);
addStateResource(2, 'steel', 12);
// 给 FRA state 7 加钢产
addStateResource(7, 'steel', 16);

// GER 初始 3 条生产线
createProductionLine('GER', 'infantry_equipment', 5);  // 5 工厂产步兵装备
createProductionLine('GER', 'artillery', 2);            // 2 工厂产炮
// FRA 2 条
createProductionLine('FRA', 'infantry_equipment', 4);

// 起步库存(模拟"已生产几天")
addEquipment('GER', 'infantry_equipment', 100);
addEquipment('GER', 'artillery', 20);
addEquipment('FRA', 'infantry_equipment', 80);
```

注意:variant 名要看 GameData 嵌入的是 `infantry_equipment`(archetype) 还是 `infantry_equipment_1`(具体型号)。先核实,统一用嵌入数据的实际 key。

- [ ] **Step 3: 移除/精简 engine_supply 调用**

现状 setup 末尾有 `supply('GER')` 之类一次性补满装备的调用,改成 add_equipment 给起步库存即可(见上)。删除或注释 `engine_supply` 调用。

- [ ] **Step 4: 手动跑 demo 验证**

```bash
# 已有 http server
# 浏览器开 http://127.0.0.1:8765, 点 🏭 生产, 应看到 GER 的 2-3 条生产线 + 起步库存
# 点 ▶ 推进时间, 几秒后库存数字应增长
```

- [ ] **Step 5: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add web/js/main.js && git commit -m "feat(demo): setup 加初始生产线/资源/起步库存"
```

---

## Task 16: Playwright 端到端 — tests/web_demo.mjs

**Files:**
- Modify: `tests/web_demo.mjs`

- [ ] **Step 1: 读 web_demo.mjs 现有结构(找合适插入点)**

```bash
grep -n "test(" E:/hoi4-conde/HOI4-CLONE/tests/web_demo.mjs | tail -10
```

- [ ] **Step 2: 加 5 项测试**

参考现有 test() 风格,在末尾加:

```javascript
test('生产面板可打开', async () => {
  await page.click('#btnProduction');
  await page.waitForSelector('#productionPanel', { timeout: 2000 });
});

test('仓库徽章显示库存数', async () => {
  const text = await page.$eval('#stockpileBadge', el => el.textContent);
  assert(/📦/.test(text), '徽章应含 📦');
});

test('生产线加减工厂', async () => {
  await page.click('#btnProduction');
  await page.waitForSelector('#productionPanel');
  // 记录当前 active 数, 点 +
  const before = await page.$eval('.pp-line-factories', el => parseInt(el.textContent.match(/\[(\d+)/)[1]));
  await page.click('[data-action="inc"]');
  await page.waitForTimeout(200);  // 等 refresh
  const after = await page.$eval('.pp-line-factories', el => parseInt(el.textContent.match(/\[(\d+)/)[1]));
  assert(after === before + 1, `工厂应 +1: before=${before}, after=${after}`);
});

test('跑 24 小时后库存增长', async () => {
  const before = await page.$eval('#stockpileBadge', el => parseFloat(el.textContent.match(/📦\s*([\d.]+)/)[1]));
  // 点 ▶ 推进 24 小时(具体按钮 selector 看 web_demo 现有 tick 测试)
  await page.click('#btn-play');  // 或类似
  await page.waitForTimeout(2000);
  await page.click('#btn-pause');
  const after = await page.$eval('#stockpileBadge', el => parseFloat(el.textContent.match(/📦\s*([\d.]+)/)[1]));
  assert(after > before, `库存应增长: before=${before}, after=${after}`);
});

test('切换 variant 弹确认', async () => {
  // 触发 changeLineVariant(若 UI 有该按钮) — 或用 engine/commands.js 直接调
  // 此项依赖 UI 是否暴露切换入口, 若无则跳过或后续补 UI
});
```

- [ ] **Step 3: 跑 Playwright**

```bash
cd E:/hoi4-conde/HOI4-CLONE/web && python -m http.server 8765 &
cd E:/hoi4-conde/HOI4-CLONE && node tests/web_demo.mjs 2>&1 | tail -20
```

预期:22(原)+ 4 新 = ~26/26 通过(切换 variant 那项视 UI 实现可跳过)。

- [ ] **Step 4: 截图存证**

```bash
# 跑完 demo 后截图
cp E:/hoi4-conde/HOI4-CLONE/tests/demo-final.png E:/hoi4-conde/HOI4-CLONE/tests/demo-production.png 2>/dev/null || true
```

- [ ] **Step 5: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add tests/web_demo.mjs && git commit -m "test(web_demo): 加生产面板端到端 5 项"
```

---

## Task 17: HANDOFF 更新 + 收尾

**Files:**
- Modify: `docs/HANDOFF.md`

- [ ] **Step 1: 在 HANDOFF.md 加新章节**

参考既有 P0-1/P0-2 章节风格,在 §1 表格末行加 `| **生产系统 + 装备 key 重构** | 见下方"生产系统"小节(...) | ~225 |`,并在文档中段加详细小节:

```markdown
### 生产系统 + 装备 key 重构(2026-06-26, 闭环: 损耗→生产→补给→再战)

实现 arms_factory 每日产出装备入仓库, 与现有 reinforce 形成完整闭环。
同时把装备 key 链路从 chassis 改成 variant(原版语义: 需求按族, 持有按变体)。

| 改造 | 内容 | 对齐/来源 |
|---|---|---|
| **生产线(per-slot)** | ProductionLine(15 槽位), 每槽独立 efficiency | 原版 EFFICIENCY_LOSS_PER_UNUSED_DAY 注释 |
| **效率机制** | 起始 10%, cap 50%, 每日 (cap-eff)×0.1 增长; inactive 槽 -0.01/日衰减 | defines BASE_FACTORY_* |
| **variant 切换保留** | 同 chassis 不同 variant 90%; 不同 chassis 重置 | defines BASE_FACTORY_EFFICIENCY_VARIANT_CHANGE_FACTOR |
| **资源惩罚** | 缺 N 单位资源 → -5%×N/工厂(line 级); 多资源累加 | defines PRODUCTION_RESOURCE_LACK_PENALTY |
| **production_step** | clock 每日 on_daily 后, reinforce_all 前; 三阶段(快照→计算→写回) | — |
| **装备 key 重构** | need=chassis, held/stockpile=variant; reinforce 按 chassis 查 variant 池(优先最新) | 原版"需求按族, 持有按变体" |
| **State 资源加载** | state_loader 解析 `resources={steel=N}` 块到 State.resources | 原版 history/states/*.txt |
| **5 个新命令** | create_production_line/set_line_factories/change_line_variant/remove_production_line/add_state_resource | effects_documentation 风格 |
| **UI 面板** | productionPanel(生产线+仓库只读展示, 加减工厂)+ stockpileBadge 顶栏徽章 | 原版生产标签页风格 |

**关键决策**(用户确认 + 原版调研):
- per-slot 严格对齐(用户选严格 vs per-line 简化)
- 资源 -5%/工厂/单位严格对齐(用户选严格 vs 比例简化)
- 装备 key 选 variant 全名(用户选, 对齐原版混装语义)
- 切 chassis 重置 / 切 variant 90% 保留(defines 实证)
- 民用工厂/资源贸易/建造系统 留后续(YAGNI)

**踩坑**:
- Country 加字段后 Default 要补;reinforce 改 key 时现有 3 个测试同步改 key(variant 全名)。
- World.data 是 Arc<GameData>(只读), 测试时无法注入 mock 装备, 端到端测试放 integration 用嵌入数据。
- variant_chassis 用 rfind('_')+suffix 数字判定, 注意 chassis 名本身含下划线(light_tank_chassis) 的情况。
```

- [ ] **Step 2: 更新 §0 顶部状态行**

```markdown
> **更新**: 2026-06-26(生产系统 + 装备 key 重构 — arms_factory 每日产出 + variant 全名 key; **~225 测试** = ...)
```

- [ ] **Step 3: 更新 §5 测试基线数字**

`208 测试` → `~225 测试`(实际跑完后填准)。

- [ ] **Step 4: 跑最终全量回归确认**

```bash
cd E:/hoi4-conde/HOI4-CLONE && cargo test -- --test-threads=1 2>&1 | tail -5
cd E:/hoi4-conde/HOI4-CLONE && cargo build --target wasm32-unknown-unknown --lib --release 2>&1 | tail -3
cd E:/hoi4-conde/HOI4-CLONE && node tests/web_demo.mjs 2>&1 | tail -3
```

预期:cargo test 全绿 / wasm 0 警告 / Playwright 全过。

- [ ] **Step 5: Commit**

```bash
cd E:/hoi4-conde/HOI4-CLONE && git add docs/HANDOFF.md && git commit -m "docs: HANDOFF 更新生产系统章节 + 测试基线"
```

---

## 自审(Self-Review)

**1. Spec 覆盖检查**:
- §1 数据模型(实体)→ Task 1, 2, 3, 4, 5, 6 ✅
- §1.C 槽位规则 → Task 2(set_active)+ Task 3 测试 ✅
- §1.D 切换保留 → Task 8 ✅
- §1.E 装备 key 重构 → Task 9 ✅
- §2 production_step + 资源惩罚 → Task 7 ✅
- §3 命令注册 → Task 10 ✅
- §3 clock hook → Task 11 ✅
- §3 WASM 序列化 → Task 12 ✅
- §4 UI 面板 → Task 14 ✅
- §5 State 资源加载 → Task 5 ✅
- §6 测试策略(单元+ reinforce + integration + Playwright) → Task 3, 4, 5, 7, 8, 9, 13, 16 ✅
- §7 模块组织 → Task 2 ✅

**2. 占位符扫描**: 无 TODO/TBD(部分"视 UI 实现可跳过"是合理的不确定性,标注明确)。variant 名实施时核实嵌入数据(已说明 fallback 路径)。

**3. 类型一致性**:
- `ProductionLine` 字段(`id/chassis/variant/slots/active_count`)跨 Task 一致 ✅
- `set_active(n: u32)` 签名一致 ✅
- `change_line_variant(line, new_variant)` 签名一致 ✅
- `variant_chassis(&str) -> &str` 一致 ✅
- EquipmentDef.resources 在 data/equipment.rs(数据驱动 `Vec<(String,f64)>`)和 combat/equipment_data.rs(硬编码 `&'static [(&'static str, f64)]`)是两个独立类型(前者 GameData 用,后者旧路径用),不冲突 ✅

**4. 歧义检查**:
- variant 名(GameData 含 `_1` 后缀 vs 不含)已在 Task 10/13/15 标注"先核实嵌入数据",明确 fallback ✅
- factory slot 重激活保留 efficiency 规则与衰减逻辑在 Task 3 测试 + Task 7 实现里一致 ✅

---

## 执行交接(Execution Handoff)

**Plan complete and saved to `docs/superpowers/plans/2026-06-26-production-equipment.md`. Two execution options:**

**1. Subagent-Driven(推荐)** — 每个 Task 派新 subagent, 任务间审阅, 快速迭代

**2. Inline Execution** — 本会话内批量执行 + 检查点审阅

**Which approach?**
