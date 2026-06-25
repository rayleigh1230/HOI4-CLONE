# 国家资源模型重构(全局变量 → 国家级) 设计文档

> 日期: 2026-06-25
> 状态: 已批准(头脑风暴),待实现
> 关联: `docs/design-principles.md`(原则1: 原版设计是首要参考)
> 关联: `docs/HANDOFF.md`(§4 下阶段方向 — P0-1 地基)
> 关联: `docs/superpowers/specs/2026-06-24-modifier-layer-design.md`(modifier 统一修正接口)

---

## 0. 背景与目标

### 现状问题

`political_power`(政治点)和 `stability`(稳定度)现在是 **`World.vars` 这个全局 HashMap 里的 key**, 不是 `Country` 字段; `war_support`(战争支持度)**完全不存在**。

这是最隐蔽的结构性债:

- **全局而非国家级**: 一个 `political_power` 变量无法表示多个国家各自的 PP。任何"每个国家独立一份资源"的系统(国策花 PP、科技、宣战消耗、稳定度触发事件)都会撞这堵墙。
- **modifier 接口不通**: `parse_modifier_token` 现在拒绝 `stability_factor`/`political_power`(返回 None, `modifier.rs:273`), 资源无法被 modifier 修正。
- **war_support 缺失**: 原版三件套之一, 本项目根本没有。

### 为什么必须先做(优先级依据)

- **越晚改越痛**: 现在改成本最低(就 `vars.rs` 几个命令 + `Country` 加字段 + `interp.rs` trigger 一处)。等做了国策/科技再回头改, 要动很多调用点。
- **是后续系统的前置**: 国策(花 PP)、科技(花 PP/科研槽)、贸易、宣战消耗都依赖国家级资源。
- **独立性高**: 不碰战斗结算/生产/补给逻辑, 只动 `Country` 结构 + 资源命令 + trigger 作用域解析。

### 目标

把三件套(PP / 稳定度 / 战争支持度)从全局变量改成 **`Country` 上的具名字段**, 并打通 modifier 接口(让资源能被修正)。为后续国策/科技/政治系统铺地基。

### 范围(本次做)

- **`Country` 加三个具名字段**: `political_power` / `stability` / `war_support`(存 base 值)
- **modifier 接口打通**: `ModifierStat` 加 `Stability`/`WarSupport`/`PoliticalPower`, `parse_modifier_token` 接受对应 token; 复用现有 `ModifierStack` 全套机制(零新数据结构)
- **资源命令改造**: 现有 `add/set_stability`、`add/set_political_power` 从全局改读写**当前作用域国家**的资源
- **新增命令**: `create_country`(建国家+设资源初值)、`add_war_support`/`set_war_support`
- **trigger Compare 作用域化**: `interp.rs:151` 的 `political_power >= 150` 改读当前国家的 effective 值
- **序列化**: `get_state` 吐出国家资源字段(供顶栏 UI, UI 本身不在本轮)
- **作用域语义**: 栈优先回退 `player_tag`(与 `current_country()` 现有逻辑一致)
- **无国家时报错**: 栈空 + `player_tag` 空时资源命令返回 `CmdError`(选项甲, 逼出隐藏 bug)

### 非目标(本次不做)

- ❌ **on_daily 默认 PP 增长**(原版 +2/天, 受政体/精神修正; 留给政治/政体系统)
- ❌ **稳定度/战争支持度对生产的实际效果**(低稳定度减工厂产出等; 只存值+算 effective, 效果留对应系统)
- ❌ **fuel / 科研槽 / 民用工厂消耗**等其他国家资源(留对应系统)
- ❌ **顶栏 UI 显示资源**(数据先出, UI 后做)
- ❌ **政党支持度(popularity)**(独立系统, 影响稳定度修正但本身不在三件套内)
- ❌ **向后兼容全局变量路径**(全量迁移, 不保留双写债)

---

## 1. 核心设计决策

| # | 决策 | 选择 | 依据 |
|---|---|---|---|
| 1 | 资源存储形态 | `Country` 具名字段(`political_power: f64` 等), 非 `HashMap<String,f64>` | 三件套是固定核心资源; 与现有 `Province` 派生/`Country` 具名字段风格一致; 避免"全局 HashMap 痛点换地方" |
| 2 | modifier 模型 | **复用现有 `ModifierStat`/`ModifierStack`**, 非独立加法池 | 原版调研: 稳定度走统一 modifier 框架, `stability`(Add) + `stability_factor`(Multiply) 两 token, 公式 `(1+ΣAdd)×Π(1+Multiply)` 与现有 `multiplier()` 完全一致。"加法叠加+buffer"只是 Add-only 特例 + clamp 逻辑, 非另一套模型 |
| 3 | 资源读取语义 | **effective(带 modifier)**: trigger/序列化读 effective; 命令改 base | 原版: trigger(`political_power >= 150`)和 UI 读的是玩家可见的 effective 值; 事件/国策改的是 base |
| 4 | 作用域解析 | 栈优先(`Scope::Country`), 回退 `player_tag` | 与现有 `current_country()` 一致; demo 单国家场景兼容(player_tag 兜底) |
| 5 | 无国家时行为 | 返回 `CmdError`(报错, 非静默) | 资源无处可写时报错比静默吞掉利于调试; 逼脚本/测试显式建国家 |
| 6 | 向后兼容 | 不兼容, 全量迁移现有测试 | 双写是技术债; 国家级化是正确方向 |
| 7 | buffer 机制 | `effective = clamp(base × mult, 0, 1)`; buffer = `max(raw-1, 0)` 超额保留 | 原版: 超 100% 部分不丢失, 作为缓冲抵御未来负修正 |

---

## 2. 数据模型

### 2.1 `Country` 字段(`src/runtime/entities.rs`)

```rust
#[derive(Debug, Clone, Default)]
pub struct Country {
    pub tag: String,
    pub owned_states: Vec<u32>,
    pub capital_state: u32,
    // ★ 新增: 国家级资源三件套(存 base 值, 不含 modifier)
    pub political_power: f64,   // 政治点(累积值, 无上下限修正; 原版范围 -500..2000)
    pub stability: f64,         // 基础稳定度(0.0-1.0; 受事件/国策改)
    pub war_support: f64,       // 基础战争支持度(0.0-1.0)
    // 现有字段(不动)
    pub equipment_stockpile: std::collections::HashMap<String, f64>,
    pub manpower_pool: f64,
    pub modifiers: crate::combat::modifier::ModifierStack,  // 战斗+资源修正统一栈
    pub faction: Option<String>,
}
```

**Default 值**(对齐原版 `BASE_STABILITY=0.5` / `BASE_WAR_SUPPORT=0.5`):
- `political_power = 0.0`
- `stability = 0.5`
- `war_support = 0.5`

### 2.2 `ModifierStat` 扩展(`src/combat/modifier.rs`)

```rust
pub enum ModifierStat {
    // 现有 8 个(不动)
    SoftAttack, HardAttack, Defense, Breakthrough, Armor, Piercing,
    CombatWidth, OrgRegain,
    // ★ 新增: 资源属性
    Stability,        // stability / stability_factor
    WarSupport,       // war_support / war_support_factor
    PoliticalPower,   // political_power / political_power_factor
}
```

`parse_modifier_token` 加映射(复用现有"无后缀=Add, `_factor`=Multiply"约定):
```rust
"stability" => ModifierStat::Stability,
"war_support" => ModifierStat::WarSupport,
"political_power" => ModifierStat::PoliticalPower,
```

### 2.3 读取方法(Country impl, entities.rs)

```rust
impl Country {
    /// 有效稳定度 = clamp(base × 资源modifier, 0, 1)
    pub fn effective_stability(&self) -> f64 {
        (self.stability * self.modifiers.multiplier(ModifierStat::Stability)).clamp(0.0, 1.0)
    }
    /// 稳定度 buffer(超 100% 部分, 抵御未来负修正; 对齐原版)
    pub fn stability_buffer(&self) -> f64 {
        let raw = self.stability * self.modifiers.multiplier(ModifierStat::Stability);
        (raw - 1.0).max(0.0)
    }
    pub fn effective_war_support(&self) -> f64 {
        (self.war_support * self.modifiers.multiplier(ModifierStat::WarSupport)).clamp(0.0, 1.0)
    }
    /// PP 不 clamp(累积值, 原版 -500..2000); 但应用 modifier
    pub fn effective_political_power(&self) -> f64 {
        self.political_power * self.modifiers.multiplier(ModifierStat::PoliticalPower)
    }
}
```

### 2.4 关键语义:effective vs base

| 操作 | 读/写 | 值 |
|---|---|---|
| trigger Compare(`political_power >= 150`) | 读 | **effective**(含 modifier) |
| 序列化 `get_state`(供 UI) | 读 | **effective**(玩家可见值) |
| `add_stability`/`set_stability` 命令 | 写 | **base**(不含 modifier) |
| `add_political_power` 命令 | 写 | base |

**设计要点**: 写改 base, 读用 effective。这样 modifier(如 `stability_factor +0.05` 来自国家精神)是"读取时叠加", 不污染 base; 移除 modifier 后 base 不变, 自动恢复。对齐原版"modifier 是临时/可移除的修正层"。

---

## 3. 命令与作用域

### 3.1 资源命令改造(`src/commands/vars.rs`)

现有 4 个全局命令, 改读写当前作用域国家。命令签名不变(脚本兼容), 内部从 `world.get_var` 改为 `current_country()` 的字段:

| 命令 | 现状(全局) | 改造后(国家级) |
|---|---|---|
| `add_political_power = N` | `world.add_var("political_power", N)` | `country.political_power += N` |
| `set_political_power = N` | `world.set_var(...)` | `country.political_power = N` |
| `add_stability = N` | `world.add_var("stability", N)` | `country.stability += N`(改 base) |
| `set_stability = N` | `world.set_var(...)` | `country.stability = N` |

`country` 由 `world.current_country()` 解析(栈优先, 回退 `player_tag`)。无国家时返回 `CmdError::RuntimeError`(决策 5)。

### 3.2 新增命令

| 命令 | 作用 | 对齐原版 |
|---|---|---|
| `create_country = { tag=GER political_power=50 stability=0.5 war_support=0.5 capital_state=1 }` | 建国家实体 + 设资源初值(字段可选, 缺省 Default)。**重复 tag 行为**: 已存在则覆盖资源字段(以最后一次 create_country 为准, 对齐原版 history 加载"后者覆盖"语义) | history/countries 加载语义 |
| `add_war_support = N` / `set_war_support = N` | 战争支持度增减(同 stability 套路) | 原版有此命令 |

`create_country` 补全了"显式建国家"入口(现在 Country 靠 `entry().or_default()` 隐式建, 无显式建国家命令)。

### 3.3 trigger Compare 作用域化(`src/runtime/interp.rs:151`)

现状:
```rust
Trigger::Compare { lhs, op, rhs } => {
    let l = world.get_var(lhs);   // ← 全局读
    ...
}
```

改造: **已知资源名走国家 effective, 未知名走全局变量**:
```rust
Trigger::Compare { lhs, op, rhs } => {
    let l = match lhs {
        "political_power" => current_country_effective_pp(world),
        "stability"       => current_country_effective_stability(world),
        "war_support"     => current_country_effective_war_support(world),
        other             => world.get_var(other),  // 其他变量仍走全局
    };
    ...
}
```

无国家作用域时: 资源名返回 `0.0`(trigger 自然判 false, 不 panic)。

> **与 §3.1 命令行为的区别(刻意设计)**: 命令(`add_political_power`)无国家时报错, trigger(查询)无国家时返回 0。理由: 命令是"写"操作, 资源无处可写应报错逼出 bug; trigger 是"读/查询", 无国家时资源视为 0 是合理默认(trigger 自然不满足, 不会 panic 中断脚本)。两者不对称是有意为之。

---

## 4. modifier 接口(落地细节)

### 4.1 零命令改动

现有 `add_country_modifier`(commands.rs:572)已把修正塞进 `Country.modifiers`。本次只让 `stability_factor` 等 token 不再被拒绝(parse_modifier_token 现返回 None), 于是:
```
add_country_modifier = { tag=GER stat=stability_factor value=0.05 }
```
自然生效——**命令本身不用改**, 只改 token 映射。这是设计上的优雅点: modifier 接口"打通"几乎免费。

### 4.2 CombatContext 不受影响

`CombatContext::build`(modifier.rs:136)汇总国家+省份+师 modifier 用于战斗结算。资源 modifier(Stability/WarSupport/PoliticalPower)虽进了 `Country.modifiers` 同一个栈, 但战斗结算只查战斗属性 stat(SoftAttack 等), 不会误用资源 stat。无需特殊隔离——`multiplier(SoftAttack)` 只看 SoftAttack 类的修正, 资源修正自动被忽略。

---

## 5. 序列化(`src/wasm_api.rs`)

`get_state` 序列化 countries 时加资源字段(现只吐 factions):

```rust
for (tag, country) in &world.countries {
    // ... 现有 fields ...
    "political_power": country.effective_political_power(),
    "stability": country.effective_stability(),
    "war_support": country.effective_war_support(),
}
```

**序列化 effective**(带 modifier), 对齐"UI 显示玩家可见值"。顶栏 UI 显示是后续任务, 本轮只保证数据可达。

---

## 6. 测试策略(TDD, 全量迁移)

### A. 新增能力测试(先写, 红)

| 测试 | 验收点 |
|---|---|
| `t_country_has_per_instance_resources` | 两个 Country 的 PP/stability 互不影响(核心验收: 国家级化) |
| `t_create_country_sets_resources` | `create_country` 设初值生效 |
| `t_add_stability_uses_scope_country` | 设 GER 作用域后 `add_stability` 只改 GER, 不碰 FRA |
| `t_trigger_compare_reads_country_resource` | `political_power >= 150` 读该国 effective |
| `t_modifier_stability_factor_applies` | `add_country_modifier stat=stability_factor` 让 effective ≠ base |
| `t_stability_clamp_and_buffer` | base+修正超 1.0 时 effective=1.0, buffer 保留超额 |
| `t_no_country_scope_errors` | 栈空+player_tag 空 → `add_political_power` 返回 Err |
| `t_war_support_commands` | add/set_war_support 读写该国 |

### B. 迁移现有测试(改, 保绿)

| 文件 | 改动 |
|---|---|
| `clock.rs` 2 个测试 | 现用全局 `political_power` + `on_daily`; 改成先建国家作用域, `add_political_power` 作用于该国 |
| `vars.rs` 3 个测试 | 改读 `Country` 字段而非 `world.get_var` |
| `integration.rs` `focus_add_pp_then_stability` | setup 建国家作用域, 国策脚本作用该国 |

### C. 不受影响(不动)

- 战斗测试(battle.rs / movement.rs / width.rs 等): 不涉及国家资源
- modifier 现有 8 个 stat 单元测试: 只新增 3 个资源 stat 测试

---

## 7. 实施顺序建议

1. `ModifierStat` + `parse_modifier_token` 扩展(零破坏, 现有测试全绿)
2. `Country` 加字段 + Default + effective 方法
3. `vars.rs` 命令改造 + 新增命令 + 无国家报错
4. `interp.rs` trigger Compare 作用域化
5. `wasm_api.rs` 序列化
6. 迁移现有测试(A 类先写红 → 实现 → B 类迁移保绿)
7. 全量 `cargo test` + WASM 构建验证

每步后跑 `cargo test` 确认增量绿, 最后全量回归。

---

## 附: 原版调研备忘

| 规则 | 结论 | 来源 |
|---|---|---|
| 稳定度 modifier 模型 | 走统一 modifier 框架; `stability`(Add) + `stability_factor`(Multiply); 公式与战斗属性一致 | Modifiers wiki + Government wiki |
| 稳定度 buffer | 超 100% 部分不丢失, 作为缓冲; effective = clamp(base×mult, 0, 1) | Government wiki |
| BASE_STABILITY / BASE_WAR_SUPPORT | 0.5(50%) | defines NDefines.NCountry |
| PP 基础增长 | +2/天(受稳定度修正: 0%→×0.5, 100%→×1.2, 50%→×1.0); **本次不做** | Government wiki |
| trigger 读 effective | trigger/UI 读玩家可见值(effective); 事件/国策改 base | Government wiki + 调研 |
| PP 范围 | -500..2000, 累积值无 modifier 叠加(但增长率受修正) | defines |
