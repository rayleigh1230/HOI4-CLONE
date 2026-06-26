# 生产系统 + 装备补充闭环 设计文档

> 日期: 2026-06-26
> 状态: 已批准(头脑风暴),待实现
> 关联: `docs/design-principles.md`(原则1: 原版设计是首要参考)
> 关联: `docs/HANDOFF.md`(§4 下阶段方向 — 生产+装备系统)
> 关联: `docs/superpowers/specs/2026-06-24-data-driven-engine-design.md`(GameData 装备层)
> 关联: `docs/superpowers/specs/2026-06-25-country-resources-design.md`(Country 资源字段)

---

## 0. 背景与目标

### 现状

`Country.equipment_stockpile` + `manpower_pool` 已存在(entities.rs:45,47), `reinforce_all` 已经从国家仓库扣装备补师(reinforce.rs:66-71)。 HANDOFF §4 描述的"增援改造"已完成。

**真正缺失**: 没有生产循环 — 工厂不产装备, 仓库只会越用越空。师打废后无补充来源, "损耗→生产→补给→再战"闭环断裂。

### 目标

实现工厂持续产装备入仓库, 与现有 reinforce 形成完整闭环:
- arms_factory 工厂每日按生产线产出装备 → Country.equipment_stockpile
- 产出对齐原版公式(BASE_FACTORY_SPEED_MIL=4.5 IC/工厂/日)
- 含效率机制(起始 10%, cap 50%, 每日逼近)和资源惩罚(缺资源 -5%/工厂/单位)
- UI 面板供玩家管理生产线和查看库存

### 范围(本轮)

**包含**:
- 生产线实体(per-slot 效率, 严格原版)
- 生产循环 `production_step`(每日调用)
- 效率增长 + 切换 variant 保留率(90%/重置, 严格原版)
- 资源机制(State.resources 从原版加载, -5%/工厂/单位惩罚)
- 5 个新命令(create/set/change/remove/add_equipment 改造)
- 装备 key 链路重构(chassis→variant, 见 §1.D)
- 完整生产面板 UI + 顶栏仓库徽章
- demo setup 改造

**排除(YAGNI)**:
- civilian_factory(民用工厂, 留后续建造系统)
- 工厂建造/升级(本轮厂是外部输入)
- 资源贸易(本轮仅本土静态资源)
- per-archetype / per-family 保留率(70%/30%/20% — 我们的装备只有 chassis/variant 两层)
- 工厂受损/修复(SABOTAGE / BASE_FACTORY_REPAIR)
- 生产许可证(LICENSE_PRODUCTION_TECH_BONUS)
- 装备过剩系数(PRODUCTION_EQUIPMENT_SURPLUS_FACTOR, AI 用)

---

## 1. 数据模型(严格对齐原版)

### 1.A 新增/修改实体

```rust
// src/runtime/entities.rs

/// 一个工厂槽位(per-slot 效率, 对齐原版 EFFICIENCY_LOSS_PER_UNUSED_DAY)
#[derive(Debug, Clone, Default)]
pub struct FactorySlot {
    pub efficiency: f64,        // 0..0.50(激活后从 START=0.10 增长向 MAX=0.50 逼近)
    pub active: bool,           // true = 此槽位当前有工厂在生产
}

/// 一条生产线(对齐原版 production_line, 固定 15 槽位)
#[derive(Debug, Clone)]
pub struct ProductionLine {
    pub id: u32,                    // 国家内唯一
    pub chassis: String,            // 绑定的 chassis 名(如 "infantry_equipment"), 从 variant 派生
    pub variant: String,            // 具体型号全名(如 "infantry_equipment_1"), 入库 key
    pub slots: Vec<FactorySlot>,    // 固定 15 槽位, 每槽独立 efficiency
    pub active_count: u32,          // 激活槽位数(≤15, 玩家分配的工厂数)
}

impl ProductionLine {
    pub fn new(id: u32, variant: String, chassis: String) -> Self {
        Self {
            id, chassis, variant,
            slots: (0..15).map(|_| FactorySlot::default()).collect(),
            active_count: 0,
        }
    }
    /// 激活前 N 个槽位(对齐原版: 加工厂时从前往后填)
    /// 新激活(之前 inactive)的槽从 START_EFFICIENCY 起步
    pub fn set_active(&mut self, n: u32, start_eff: f64) {
        let n = n.min(15) as usize;
        for i in 0..15 {
            let was_active = self.slots[i].active;
            self.slots[i].active = i < n;
            if !was_active && i < n && self.slots[i].efficiency == 0.0 {
                self.slots[i].efficiency = start_eff;
            }
        }
        self.active_count = n as u32;
    }
}

// Country 新增字段
pub struct Country {
    // ...现有字段(entities.rs:34-52)...
    pub production_lines: Vec<ProductionLine>,           // 新增
}

// State 新增字段
pub struct State {
    // ...现有字段(entities.rs:21-31)...
    pub resources: HashMap<String, f64>,                 // 新增(steel/tungsten/aluminium/chromium/oil/rubber)
}

// EquipmentDef 新增字段(数据驱动)
pub struct EquipmentDef {
    // ...现有字段(data/equipment.rs:77-84)...
    pub resources: Vec<(String, f64)>,                   // 新增, 如 [("steel", 2.0)]
}
```

### 1.B 常量(原版对齐, 硬编码)

```rust
// src/economy/mod.rs
pub const FACTORY_SPEED_MIL: f64       = 4.5;   // BASE_FACTORY_SPEED_MIL(defines:601)
pub const EFFICIENCY_START: f64        = 0.10;  // BASE_FACTORY_START_EFFICIENCY_FACTOR(defines:603)
pub const EFFICIENCY_MAX: f64          = 0.50;  // BASE_FACTORY_MAX_EFFICIENCY_FACTOR(defines:604)
pub const EFFICIENCY_GAIN: f64         = 1.0;   // BASE_FACTORY_EFFICIENCY_GAIN(defines:605)
pub const EFFICIENCY_BALANCE: f64      = 0.1;   // BASE_FACTORY_EFFICIENCY_BALANCE_FACTOR(defines:606)
pub const VARIANT_RETENTION: f64       = 0.90;  // BASE_FACTORY_EFFICIENCY_VARIANT_CHANGE_FACTOR(defines:607)
pub const RESOURCE_LACK_PENALTY: f64   = 0.05;  // |PRODUCTION_RESOURCE_LACK_PENALTY|(defines:623)
pub const INACTIVE_SLOT_DECAY: f64     = 0.01;  // EFFICIENCY_LOSS_PER_UNUSED_DAY(defines:598, 简化为线性)
pub const SLOTS_PER_LINE: usize        = 15;    // 原版硬编码
```

### 1.C 槽位管理规则(对齐原版)

- **加工厂**: 从前向后激活 inactive 槽; 新激活(之前 inactive 且 efficiency=0)槽 → `efficiency = EFFICIENCY_START (0.10)`
- **减工厂**: 从后向前关闭; 被关槽保留 efficiency(不重置), 开始衰减
- **重激活曾用过的槽**: 保留剩余 efficiency(不重置)
- **inactive 槽衰减**: 每日 `eff = max(0, eff - INACTIVE_SLOT_DECAY)`(简化线性, 原版 EFFICIENCY_LOSS_PER_UNUSED_DAY=1 的精确公式 wiki 模糊, 本轮简化)

### 1.D Efficiency 切换保留(严格原版)

当 line 的 chassis 或 variant 改变时:

```rust
fn change_production(line: &mut ProductionLine, new_variant: &str, new_chassis: &str) {
    let retention = if new_chassis != line.chassis {
        0.0   // 不同 chassis: 重置(对齐 ARCHETYPE_CHANGE 0% / 不同 archetype 完全重置)
    } else if new_variant != line.variant {
        VARIANT_RETENTION  // 同 chassis 不同 variant: 90%
    } else {
        1.0   // 无变化
    };
    for slot in &mut line.slots {
        if retention == 0.0 {
            slot.efficiency = if slot.active { EFFICIENCY_START } else { 0.0 };
        } else {
            slot.efficiency *= retention;
        }
    }
    line.chassis = new_chassis.to_string();
    line.variant = new_variant.to_string();
}
```

> FAMILY(70%) / PARENT(30%) / ARCHETYPE(20%) 因素本轮用不到 — 我们的装备模型只有 chassis/variant 两层, 留扩展点。

### 1.E 装备 key 链路重构(chassis → variant, 关键决策)

**三层分离(原版语义)**:

| 实体 | key 粒度 | 含义 | 例子 |
|---|---|---|---|
| 营需求(subunit.rs `need`) | **chassis** | 营需要 infantry 类装备 | `"infantry_equipment"` |
| 师 `equipment_need` | **chassis** | 从营汇总 | `"infantry_equipment"` |
| 师 `equipment_held` | **variant 全名** | 实际持有, 可混多 variant | `"infantry_equipment_1"` |
| 国家 `equipment_stockpile` | **variant 全名** | 按变体分池 | `"infantry_equipment_1"` |

**辅助函数**(新增 `economy/mod.rs`):

```rust
/// 从 variant 全名解析 chassis: "infantry_equipment_1" → "infantry_equipment"
/// "light_tank_chassis_1" → "light_tank_chassis"
/// "infantry_equipment"(无 _数字 后缀) → "infantry_equipment"
pub fn variant_chassis(variant: &str) -> &str {
    if let Some(pos) = variant.rfind('_') {
        let suffix = &variant[pos + 1..];
        if suffix.bytes().all(|b| b.is_ascii_digit()) {
            return &variant[..pos];
        }
    }
    variant
}
```

**reinforce 改造**(`src/combat/reinforce.rs`):
- 现状: `for (eq, need) in &div.equipment_need` 按 chassis 直接转移 held(也按 chassis)
- 改造: need 仍按 chassis; held/stockpile 按 variant; 缺口时按 chassis 在 stockpile 找所有 variant(按字母倒序优先取最新)补充到 held

**建师路径改造**:
- `create_division` 填 held 时查 GameData 找该 chassis 默认 variant(最早/数字最小)
- 用 variant 名作 held key; need 保持 chassis 名

**equipment_data.rs 硬编码表改造**:
- 每条装备补 `variant_of: &str` 字段(如 `infantry_equipment` → `infantry_equipment_1`)
- 资源字段补: infantry steel=2, artillery tungsten=1 steel=2, light_tank steel=2 rubber=1 等(对齐原版)

---

## 2. 生产公式(每日产出循环)

### 2.A production_step 主流程

新增 `src/economy/production.rs`:

```rust
pub fn production_step(world: &mut World) {
    let game_data = world.game_data.clone();  // Arc<GameData>
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

    // 阶段 2: 每条 line 计算产出 + 更新 slot efficiency
    // 借用策略: 快照→计算→写回(沿用 reinforce.rs 风格)
    let mut outputs: Vec<(String /*tag*/, String /*variant*/, f64)> = Vec::new();
    let mut slot_updates: Vec<(String, u32 /*line_id*/, Vec<(usize, f64)>)> = Vec::new();

    for (tag, country) in &world.countries {
        let res = country_resources.get(tag).cloned().unwrap_or_default();
        for line in &country.production_lines {
            let equipment = match game_data.find_equipment(&line.variant) {
                Some(e) => e,
                None => continue,
            };
            let res_mult = resource_penalty(line, equipment, &res);
            let mut total_output = 0.0;
            let mut new_effs: Vec<(usize, f64)> = Vec::new();
            for (i, slot) in line.slots.iter().enumerate() {
                if !slot.active {
                    if slot.efficiency > 0.0 {
                        let new_e = (slot.efficiency - INACTIVE_SLOT_DECAY).max(0.0);
                        new_effs.push((i, new_e));
                    }
                    continue;
                }
                let out = FACTORY_SPEED_MIL * slot.efficiency * res_mult / equipment.build_cost_ic;
                total_output += out;
                let new_e = slot.efficiency + (EFFICIENCY_MAX - slot.efficiency)
                            * EFFICIENCY_GAIN * EFFICIENCY_BALANCE;
                new_effs.push((i, new_e));
            }
            if total_output > 0.0 {
                outputs.push((tag.clone(), line.variant.clone(), total_output));
            }
            slot_updates.push((tag.clone(), line.id, new_effs));
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
```

### 2.B 资源惩罚(严格 -5%/工厂/单位)

```rust
fn resource_penalty(line: &ProductionLine, equipment: &EquipmentDef,
                    country_resource_avail: &HashMap<String, f64>) -> f64 {
    let mut penalty: f64 = 0.0;
    for (resource, need_per_factory) in &equipment.resources {
        let total_need = line.active_count as f64 * need_per_factory;
        let available = country_resource_avail.get(resource).copied().unwrap_or(0.0);
        let shortage_units = (total_need - available).max(0.0);
        penalty += shortage_units * RESOURCE_LACK_PENALTY;
    }
    (1.0 - penalty).max(0.0)
}
```

**例**:
- 10 工厂产 artillery(tungsten=1, steel=2/factory), 国家 tungsten=0, steel=20
  - tungsten 缺 10 单位 → -50%
  - steel 缺 0 单位 → 0%
  - 总惩罚 50%, 产出 ×= 0.5
- 5 工厂产 infantry(steel=2/factory), 国家 steel=20
  - 钢缺 0(20 ≥ 10) → 0% 惩罚

### 2.C 产出公式关键值

```
单 slot 日产出 = FACTORY_SPEED_MIL × slot.efficiency × res_mult / equipment.build_cost_ic
                = 4.5 × eff × res_mult / bc

例(满效率, 资源足):
  1 工厂 / eff=50% / infantry(bc=0.43): 4.5 × 0.5 / 0.43 ≈ 5.23 件/日
  1 工厂 / eff=10%(起始) / infantry:    4.5 × 0.1 / 0.43 ≈ 1.05 件/日
  10 工厂 / eff=50% / infantry:         10 × 5.23 ≈ 52 件/日
  1 工厂 / eff=50% / artillery(bc=3.5): 4.5 × 0.5 / 3.5 ≈ 0.64 件/日

效率增长(每日):
  eff += (MAX - eff) × GAIN × BALANCE = (0.5 - eff) × 0.1
  例: eff=0.10 → +0.04 → 0.14; eff=0.40 → +0.01 → 0.41
  从 0.10 到 ~0.49 约 30 日
```

---

## 3. 命令 & 事件

### 3.A 新命令(注册到 registry.rs)

```rust
// 1. 创建生产线
// 语法: create_production_line = { country = GER variant = infantry_equipment_1 factories = 5 }
// 行为: 在该国加一条新线, 激活 N 个槽(从 EFFICIENCY_START 起步)
// 错误: country 不存在 / variant 在 GameData 找不到 / factories > 15 截到 15

// 2. 调整生产线工厂数
// 语法: set_line_factories = { country = GER line_id = 0 factories = 8 }
// 行为: 加工厂(前向激活 inactive 槽) / 减工厂(后向关闭槽, 保留 efficiency)
// 边界: factories > 15 截到 15; < 0 截到 0

// 3. 切换生产线型号
// 语法: change_line_variant = { country = GER line_id = 0 variant = infantry_equipment_2 }
// 行为: 同 chassis 不同 variant → 每 slot efficiency × VARIANT_RETENTION(0.9)
//      不同 chassis → 全部 slot 重置(EFFICIENCY_START 或 0)
// 错误: variant 在 GameData 找不到 → CmdError

// 4. 删除生产线
// 语法: remove_production_line = { country = GER line_id = 0 }

// 5. (改造现有)add_equipment
// 现: add_equipment = { country = GER amount = 100 type = infantry_equipment }
// 改: type 接受 variant 名(infantry_equipment_1) — 入库到 variant key
```

### 3.B 事件 hook

```rust
// src/runtime/clock.rs(现状, 行 21-24)
if world.hour % 24 == 0 {
    world.fire_event(interp, "on_daily");
    world.fire_event(interp, &format!("on_daily_{}", world.player_tag));
    crate::combat::reinforce::reinforce_all(world);
}

// 改造后: production_step 插在 on_daily 后, reinforce_all 前
if world.hour % 24 == 0 {
    world.fire_event(interp, "on_daily");
    world.fire_event(interp, &format!("on_daily_{}", world.player_tag));
    crate::economy::production::production_step(world);   // 新增
    crate::combat::reinforce::reinforce_all(world);
}
```

**顺序理由**: 每日先产装备入仓库 → 再由 reinforce 从仓库补给师。今日产出当天可用于补充。

### 3.C WASM API 导出

`wasm_api.rs` 的 `get_state` 在 countries 数组里加字段:

```rust
// equipment_stockpile(variant key)
"stockpile": { "infantry_equipment_1": 52, "artillery_equipment_1": 8, ... },

// production_lines
"production_lines": [
    {
        "id": 0,
        "variant": "infantry_equipment_1",
        "chassis": "infantry_equipment",
        "active": 5,
        "slots": [0.38, 0.42, 0.36, 0.40, 0.41, 0, 0, ...]  // 15 个 efficiency
    },
    ...
]
```

UI 通过此字段读取生产/库存状态。

---

## 4. UI 面板

### 4.A 新增文件

```
web/js/views/
├── productionPanel.js          ← 新增(生产面板主体)
└── stockpilePanel.js           ← 新增(仓库面板, 鼠标悬停顶栏徽章弹出)
```

### 4.B 触发与布局

- **顶栏新增 "🏭 生产" 按钮**(左起第 3, 靠"切视角") → 打开 productionPanel
- **顶栏右上加仓库徽章**(显示库存总装备数 / 总产线数) → 鼠标悬停展开 stockpilePanel
- ESC / 关闭按钮关闭面板(沿用现有 ESC 风格)

### 4.C 生产面板布局

```
┌─ 生产管理 — GER ──────────────────────────── [X] ┐
│                                                    │
│ ▌生产概览                                          │
│   军用工厂: 10 / 12 (在产 8, 闲置 2)              │
│   本土资源: 钢 18 / 钨 4 / 铝 0                   │
│                                                    │
│ ▌生产线 (3)                       [+ 新建生产线]  │
│ ┌──────────────────────────────────────────────┐  │
│ │ #1  Infantry Equipment I      [5/15] ▶━━━○  │  │
│ │     eff 38%  ·  日产 4.5 件 ·  需 钢×10/日   │  │
│ │     [切换型号 ▾]  [工厂: − 5 +]  [删除]     │  │
│ ├──────────────────────────────────────────────┤  │
│ │ #2  Artillery Equipment I     [3/15] ▶━○    │  │
│ │     eff 22%  ·  日产 0.7 件 ·  需 钢×6 钨×3  │  │
│ │     [切换型号 ▾]  [工厂: − 3 +]  [删除]     │  │
│ └──────────────────────────────────────────────┘  │
│                                                    │
│ ▌仓库 (按变体)                                     │
│   Infantry Equipment                               │
│     ├ infantry_equipment_1:    52                  │
│     └ infantry_equipment_2:     0                  │
│   Artillery                                        │
│     └ artillery_equipment_1:    8                  │
└────────────────────────────────────────────────────┘
```

- 每条线: variant 名 / 工厂进度条(X/15) / 效率进度条(eff%) / 日产估算 / 资源需求
- 工厂 ± 按钮: 调 `set_line_factories`
- 切换型号下拉: 列 GameData 里同 chassis 的所有 variant, 调 `change_line_variant`
- 新建线按钮: 弹小框选 chassis+variant+工厂数

### 4.D 仓库面板

鼠标悬停顶栏徽章弹出, 按 chassis 分组显示 variant 库存(简化浮层)。

### 4.E UI 数据来源

- 读 `state.countries[GER].production_lines` → 渲染生产线
- 读 `state.countries[GER].stockpile` → 渲染仓库
- 读 `state.countries[GER].owned_states` → 汇总 `state.states[sid].resources` → 显示本土资源
- 每日 tick 后 production_step 更新, UI 自动刷新(沿用 store 路径级脏标记订阅)

### 4.F 交互细节

- 工厂 ± 按钮防抖(连续点 ≤ 1 次/100ms)
- 切换 variant 弹确认(efficiency 会保留 90% 或重置)
- 删除线弹确认
- 仅显示玩家国家(权限与 deployPanel 一致)

### 4.G demo setup 改造(main.js)

```javascript
// 现状: setup 后 GER 用 engine_supply 一次性补满装备
// 改造后:
// 1. 给 GER/FRA 各创建 2-3 条初始生产线(infantry/artillery/light_tank_chassis)
// 2. 给 GER 加些初始仓库库存(模拟"已生产几天", add_equipment 调试命令)
// 3. engine_supply 调用移除或改成 add_equipment 给起步库存
// 4. 让玩家点 ▶ 后能看到生产循环跑起来(每日+装备、+效率)
```

---

## 5. State 资源加载(state_loader.rs 改造)

### 5.A 现状

`state_loader.rs` 已加载 `buildings`, **未加载** `resources` 块。State 实体当前无 resources 字段。

### 5.B 改造

```rust
// src/data/state_loader.rs
// parser 已支持 Num 作 key / ident 列表(buildings 已类似处理)
// 新增解析:
//   resources = { steel = 2 }              → HashMap<steel, 2.0>
//   resources = { chromium = 3 }           → HashMap<chromium, 3.0>
//   resources = { aluminium = 10.000 }     → HashMap<aluminium, 10.0>
//   (无 resources 块时, 空 HashMap, 如 France state 1)

// State 实体加字段(见 §1.A):
//   pub resources: HashMap<String, f64>,
```

### 5.C demo 数据

嵌入的 1-France.txt 可能无 resources 块(原版 1-France 实测无), 因此 demo setup 需手动给 GER/FRA 加 State 资源(脚本或硬编码), 否则生产一律缺钢产出 0:

```javascript
// main.js setup 阶段
// 给 GER/FRA 的初始 State 加些资源(模拟德国/法国本土钢产)
add_state_resource('GER', 1, 'steel', 16);  // 例: GER state 1 钢产 16/日
add_state_resource('FRA', 7, 'steel', 12);
```

(具体命令名定 add_state_resource, 实现简单 — State.resources[key] += val)

---

## 6. 测试策略

### 6.A 单元测试(src/economy/tests.rs)

**1. 工厂槽位管理**
- `t_set_active_fills_slots_from_front` — 加工厂前向激活 inactive 槽, 新激活 efficiency=START
- `t_reduce_factories_keeps_efficiency` — 减工厂后向关闭, 被关槽保留 efficiency 不重置
- `t_reactivate_preserves_efficiency` — 重激活曾用过的槽, efficiency 保留(不重置)
- `t_active_count_clamped_at_15` — 超 15 自动截

**2. 效率增长**
- `t_efficiency_grows_toward_cap` — 10% 跑 N 日逼近 50%
- `t_efficiency_never_exceeds_cap` — 即使跑很久也 ≤ 0.50
- `t_inactive_slot_decays_daily` — inactive 槽每日 -0.01

**3. 切换 variant 保留(严格原版)**
- `t_variant_change_keeps_90pct` — 同 chassis 切 variant → efficiency × 0.9
- `t_chassis_change_resets_to_start` — 不同 chassis → 全槽重置
- `t_change_to_same_variant_noop` — 同 variant 不变

**4. 产出公式**
- `t_output_formula_basic` — 1 工厂 eff=10% infantry(bc=0.43) → `4.5×0.1/0.43 ≈ 1.05`
- `t_output_scales_with_factories` — 5 工厂 ×0.5eff 同上 → ≈26
- `t_output_zero_when_no_active_slots` — 0 工厂 → 0 产出

**5. 资源惩罚(严格 -5%/工厂/单位)**
- `t_no_penalty_when_resources_sufficient`
- `t_steel_shortage_5pct_per_unit` — 缺 2 钢 → -10%
- `t_multiple_resource_penalties_stack` — artillery 缺钨 1 + 缺钢 1 → -10%
- `t_zero_output_when_full_shortage` — 资源全缺 → 产出 0

**6. variant_chassis 辅助**
- `t_variant_chassis_strips_suffix` — `infantry_equipment_1` → `infantry_equipment`
- `t_variant_chassis_handles_no_suffix` — `infantry_equipment` → 同名

### 6.B reinforce 改造测试(改/新)

- `t_reinforce_fills_shortage_from_stockpile`(改 key) — held 用 `infantry_equipment_1`
- `t_reinforce_prefers_newer_variant`(新) — 仓库有 _1 + _2 → 优先补 _2
- `t_reinforce_mixed_variants_fill_chassis_need`(新) — _1 不够时混 _2 补
- `t_reinforce_partial_when_stockpile_low`(改 key)
- `t_no_transfer_when_full`(改 key)

### 6.C integration 测试(tests/production.rs, 新增)

- 端到端: 建国家+State(buildings 含 arms_factory) → 建生产线 → 跑 30 日 → 库存积累 + 效率达到 ~50%
- 资源耗尽: 删 State 的 steel → 产出降为 0
- variant 切换端到端: 跑 10 日 → 切 variant → 跑 5 日 → 验证效率保留 90%
- reinforce 端到端: 库存 → 师损耗 → reinforce 补 → 仓库扣

### 6.D Playwright 验证(tests/web_demo.mjs 加测试)

- 生产面板打开 + 显示初始生产线
- 顶栏仓库徽章显示库存数
- 加工厂按钮 → 工厂数+1 → 每日产出增加
- 切换 variant 弹确认 → 效率显示 ×0.9
- 跑 24 小时 → 库存数字增长(生产闭环跑通)

### 6.E 回归

跑全量 `cargo test` 确认 **208 → ~225+**(新增 15+ 单元 + 4 integration)保持全绿。CLAUDE.md 红线 2 要求全量 `cargo test`(含 tests/ 集成)。

---

## 7. 模块组织

```
src/
├── economy/                  ← 新增
│   ├── mod.rs                # 模块声明 + 常量 + FactorySlot + ProductionLine + variant_chassis
│   ├── production.rs         # production_step + resource_penalty + change_line_variant
│   └── tests.rs              # 单元测试
├── runtime/
│   ├── entities.rs           # 改: Country 加 production_lines, State 加 resources
│   └── clock.rs              # 改: on_daily 后调 production_step
├── combat/
│   ├── reinforce.rs          # 改: chassis 查 variant 池
│   └── equipment_data.rs     # 改: 补 variant_of + resources
├── data/
│   ├── equipment.rs          # 改: EquipmentDef 加 resources
│   └── state_loader.rs       # 改: 加载 resources 块
├── commands.rs(or mod)       # 改: 注册 4 新命令, 改造 add_equipment
├── wasm_api.rs               # 改: get_state 加 stockpile/production_lines 字段
└── lib.rs                    # 改: pub mod economy

web/
├── index.html                # 改: 加顶栏生产按钮 + 仓库徽章
├── css/app.css               # 改: 生产面板样式
└── js/
    ├── main.js               # 改: setup 加初始生产线/资源, ESC 关面板
    ├── engine/commands.js    # 改: 加 create_production_line 等 5 命令封装
    └── views/
        ├── productionPanel.js    ← 新增
        └── stockpilePanel.js     ← 新增

tests/
├── web_demo.mjs              # 改: 加 5 项生产面板端到端
└── production.rs             ← 新增 integration
```

---

## 8. 实施顺序建议

1. **数据层** — entities.rs / equipment.rs / state_loader.rs 改造(纯结构, 无新逻辑)
2. **economy 模块** — ProductionLine + slot 管理 + variant_chassis + 单元测试
3. **production_step** — 主产出循环 + 资源惩罚 + 单元测试
4. **change_line_variant** — variant 切换保留逻辑 + 单元测试
5. **reinforce 改造** — chassis 查 variant 池 + 测试改 key
6. **命令注册** — 5 个新命令 + 改造 add_equipment
7. **clock.rs 接入** — production_step 挂到 on_daily 后
8. **wasm_api 序列化** — 导出 stockpile / production_lines
9. **equipment_data.rs 改造** — 补 variant_of + resources 字段
10. **UI** — productionPanel + stockpilePanel + 顶栏按钮 + 徽章
11. **demo setup 改造** — 初始生产线/资源/库存
12. **integration test + Playwright** — tests/production.rs + web_demo.mjs 新增项
13. **HANDOFF 更新** — 标注本轮成果

每步独立可测, 改完跑 `cargo test` 全量, 测试基线 208 → ~225+。

---

## 9. 关键决策汇总

| 决策点 | 选择 | 依据 |
|---|---|---|
| 效率粒度 | **per-slot**(严格原版) | 用户确认, 对齐 EFFICIENCY_LOSS_PER_UNUSED_DAY 注释 |
| chassis 切换 | efficiency 重置(不同 archetype) | 原版 0% 保留 |
| variant 切换 | efficiency × 0.9 | defines BASE_FACTORY_EFFICIENCY_VARIANT_CHANGE_FACTOR |
| 资源惩罚 | **-5%/工厂/单位**(严格) | defines PRODUCTION_RESOURCE_LACK_PENALTY |
| 工厂/线 上限 | 15 | 原版硬编码 |
| IC/工厂/日 | 4.5(arms_factory) | BASE_FACTORY_SPEED_MIL |
| 效率 cap | 50% | BASE_FACTORY_MAX_EFFICIENCY_FACTOR(无科技加成) |
| 装备 key | **variant 全名**(选项 B) | 用户确认, 对齐原版"需求按族, 持有按变体" |
| need key | chassis(营需求层) | 原版营按 archetype 指定, variant 在分配时决定 |
| held/stockpile key | variant 全名 | 同上, 支持混合装备 |
| inactive 槽衰减 | 线性 -0.01/日 | 原版公式 wiki 模糊, 简化数值 |
| 民用工厂 | 本轮无作用 | YAGNI, 留建造系统 |
| 资源贸易 | 本轮无 | 本土静态资源足够 |
| 数据存 Country 字段 | production_lines 在 Country 上 | 沿用 equipment_stockpile 风格 |
| 逻辑放 economy/ 模块 | 文件组织 | 模块边界清晰 |
| architecture | 方案 A(Country 子字段) | 用户确认 |

---

## 10. 风险与待办

### 风险

- **equipment_data.rs 改造影响建师**: variant_of 字段加上后, 现有 5 种装备的 held key 全要同步改(从 `infantry_equipment` 到 `infantry_equipment_1`), 涉及多处 test 校准。需逐处审查。
- **State resources 加载**: 1-France 嵌入数据无 resources 块, demo 需手动注入, 否则 GER/FRA 无钢产出 0。setup 改造时务必加 add_state_resource。
- **integration 偶发 flaky**: 既有问题(TEST_BLOCKED thread-local 泄漏), 用 `cargo test -- --test-threads=1` 兜底。
- **借用冲突**: production_step 三阶段写回风格(快照→计算→写回)避借用冲突, 沿用 reinforce.rs 模式。

### 待办(本轮外)

- 民用工厂 + 建造系统
- 资源贸易(民厂换资源)
- 装备升级链(科技解锁 variant 2/3, 老库存保留/降级)
- 工厂受损/修复
- per-family / per-parent 保留率(70%/30%/20%)

---

## 11. 引用文档

- `docs/design-principles.md`(原则1: 原版是首要参考)
- `docs/HANDOFF.md`(§4 下阶段方向)
- `docs/superpowers/specs/2026-06-24-data-driven-engine-design.md`(GameData 装备层基础)
- `docs/superpowers/specs/2026-06-25-country-resources-design.md`(Country 资源字段基础)
- 原版数据: `D:\Program Files (x86)\Steam\steamapps\common\Hearts of Iron IV\common\defines\00_defines.lua`(行 598-623 生产常量)
- 原版数据: `common/units/equipment/*.txt`(各装备 resources 字段)
- 原版数据: `history/states/*.txt`(State resources 块)
- 原版数据: `common/buildings/00_buildings.txt`(arms_factory military_production=1)
