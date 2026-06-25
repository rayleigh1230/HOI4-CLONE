# 国家资源模型重构(全局变量 → 国家级) 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把政治点/稳定度/战争支持度(三件套)从 `World.vars` 全局变量改成 `Country` 具名字段,打通 modifier 接口,为后续国策/科技/政治系统铺地基。

**Architecture:** `Country` 加三个具名字段存 base 值;复用现有 `ModifierStat`/`ModifierStack`(扩展三个资源 stat);资源命令改读写当前作用域国家(栈优先回退 player_tag);trigger Compare 读该国 effective(含修正);无国家时命令报错、trigger 返回 0。

**Tech Stack:** Rust(stable-x86_64-pc-windows-gnu),`cargo test`,无外部依赖。

**Spec:** `docs/superpowers/specs/2026-06-25-country-resources-design.md`

**基线:** 实现前 `cargo test` 必须 191 全绿(见 HANDOFF "测试基线修复"小节)。每步后跑测试确认增量绿。

**关键借用模式(贯穿全 plan):** `current_country()` 返回 `Option<&str>` 借自 `&World`,但资源命令要 `&mut Country`。标准模式:**先快照 tag 到 owned String(释放借用),再 `countries.get_mut(&tag)`**。参考现有 `add_equipment`(commands.rs:277)。

---

## 文件结构

| 文件 | 职责 | 改动 |
|---|---|---|
| `src/combat/modifier.rs` | modifier 框架 | `ModifierStat` 加 3 枚举值 + `parse_modifier_token` 加 3 映射 |
| `src/runtime/entities.rs` | `Country` 实体 | 加 3 字段 + Default + effective 方法 |
| `src/runtime/world.rs` | World 状态 | 加 `current_country_tag()` 可变辅助 + 加 `add_country_resource` 类辅助(可选) |
| `src/commands/vars.rs` | 资源命令 | 改造 4 个 + 新增 create_country/add_war_support/set_war_support |
| `src/runtime/interp.rs` | trigger 求值 | `Compare` 作用域化(已知资源名走国家 effective) |
| `src/wasm_api.rs` | WASM 序列化 | `get_state` 吐资源字段 |
| `tests/battle.rs` `tests/scope.rs` `tests/integration.rs` `src/runtime/clock.rs` | 现有测试 | 迁移到国家级 |
| `src/runtime/world.rs`(测试 mod) | World 单元测试 | 新增国家级资源测试 |

---

## Task 1: 扩展 ModifierStat + parse_modifier_token(零破坏)

**Files:**
- Modify: `src/combat/modifier.rs:9-22`(ModifierStat 枚举)
- Modify: `src/combat/modifier.rs:102-120`(parse_modifier_token)
- Test: `src/combat/modifier.rs`(inline test mod)

**说明:** 此步零破坏——现有 8 个 stat 不动,只是让 `stability_factor` 等 token 不再返回 None。现有测试应全绿。

- [ ] **Step 1: 写失败的 token 解析测试**

在 `src/combat/modifier.rs` 的 `#[cfg(test)] mod tests` 末尾(`t_parse_unknown_returns_none` 之后)加:

```rust
    #[test]
    fn t_parse_resource_tokens() {
        // 资源属性三件套: 无后缀=Add, _factor=Multiply(对齐原版)
        let (s, op) = parse_modifier_token("stability").unwrap();
        assert_eq!(s, ModifierStat::Stability);
        assert_eq!(op, ModifierOp::Add);
        let (s, op) = parse_modifier_token("stability_factor").unwrap();
        assert_eq!(s, ModifierStat::Stability);
        assert_eq!(op, ModifierOp::Multiply);

        let (s, op) = parse_modifier_token("war_support").unwrap();
        assert_eq!(s, ModifierStat::WarSupport);
        assert_eq!(op, ModifierOp::Add);
        let (s, _) = parse_modifier_token("war_support_factor").unwrap();
        assert_eq!(s, ModifierStat::WarSupport);

        let (s, op) = parse_modifier_token("political_power").unwrap();
        assert_eq!(s, ModifierStat::PoliticalPower);
        assert_eq!(op, ModifierOp::Add);
        let (s, _) = parse_modifier_token("political_power_factor").unwrap();
        assert_eq!(s, ModifierStat::PoliticalPower);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --lib t_parse_resource_tokens`
Expected: FAIL(stability 等返回 None,Stability 枚举不存在→编译错误)

- [ ] **Step 3: 扩展 ModifierStat 枚举**

修改 `src/combat/modifier.rs:9-22`,在 `OrgRegain,` 后加:

```rust
    // 组织度恢复率
    OrgRegain,
    // ★ 资源属性(国家级三件套)
    Stability,        // stability / stability_factor
    WarSupport,       // war_support / war_support_factor
    PoliticalPower,   // political_power / political_power_factor
}
```

- [ ] **Step 4: 扩展 parse_modifier_token 映射**

修改 `src/combat/modifier.rs:102-120` 的 `match base {` 块,在 `"org_regain" | "local_org_regain" => ModifierStat::OrgRegain,` 之后、`_ => return None,` 之前加:

```rust
        "stability" => ModifierStat::Stability,
        "war_support" => ModifierStat::WarSupport,
        "political_power" => ModifierStat::PoliticalPower,
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test --lib t_parse_resource_tokens`
Expected: PASS

- [ ] **Step 6: 全量回归确认零破坏**

Run: `cargo test`
Expected: 191 全绿(现有测试不受影响)

- [ ] **Step 7: 提交**

```bash
git add src/combat/modifier.rs
git commit -m "feat(modifier): ModifierStat 加 Stability/WarSupport/PoliticalPower 三资源属性

parse_modifier_token 接受 stability/war_support/political_power(+_factor)token。
零破坏: 现有 8 stat 不动, 只是让资源 token 不再返回 None。
对齐原版统一 modifier 框架(资源属性与战斗属性同走 ModifierStack)。"
```

---

## Task 2: Country 加资源字段 + Default + effective 方法

**Files:**
- Modify: `src/runtime/entities.rs:33-46`(Country 结构)
- Test: `src/runtime/entities.rs` 或 `src/runtime/world.rs` 测试 mod

**说明:** 加字段 + Default 值 + 读取方法。`Country` 已 `#[derive(Default)]`,加字段后 Default 自动给 0.0,但稳定度/战争支持度默认应是 0.5(原版 BASE_*=0.5)。需手写 Default。

- [ ] **Step 1: 写失败的 effective 测试**

在 `src/runtime/world.rs` 的 `#[cfg(test)] mod tests` 里(`t_world_carries_game_data` 之后)加:

```rust
    #[test]
    fn t_country_default_resources() {
        // Country 默认资源: PP=0, stability=0.5, war_support=0.5(对齐原版 BASE_*)
        let c = crate::runtime::Country::default();
        assert!((c.political_power - 0.0).abs() < 1e-9);
        assert!((c.stability - 0.5).abs() < 1e-9, "默认稳定度应 0.5");
        assert!((c.war_support - 0.5).abs() < 1e-9, "默认战争支持度应 0.5");
    }

    #[test]
    fn t_effective_stability_clamp_and_buffer() {
        // effective = clamp(base × mult, 0, 1); 无 modifier 时 = base
        let mut c = crate::runtime::Country::default();
        c.stability = 0.7;
        assert!((c.effective_stability() - 0.7).abs() < 1e-9, "无修正时 effective=base");
        // base 超 1.0(非法 base, 但测 clamp): effective 应 clamp 到 1.0
        c.stability = 1.5;
        assert!((c.effective_stability() - 1.0).abs() < 1e-9, "应 clamp 到 1.0");
        assert!((c.stability_buffer() - 0.5).abs() < 1e-9, "buffer 应保留超额 0.5");
    }

    #[test]
    fn t_country_has_per_instance_resources() {
        // 核心验收: 两个 Country 的资源互不影响(国家级化, 非全局)
        let mut a = crate::runtime::Country::default();
        a.political_power = 100.0;
        let mut b = crate::runtime::Country::default();
        b.political_power = 50.0;
        assert!((a.political_power - 100.0).abs() < 1e-9);
        assert!((b.political_power - 50.0).abs() < 1e-9, "两国 PP 应独立");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --lib t_country_default_resources`
Expected: FAIL(字段不存在→编译错误)

- [ ] **Step 3: Country 加字段 + 手写 Default**

修改 `src/runtime/entities.rs:33-46`。先把 `#[derive(Debug, Clone, Default)]` 改成 `#[derive(Debug, Clone)]`(去掉 Default derive),然后改结构:

```rust
#[derive(Debug, Clone)]
pub struct Country {
    pub tag: String,
    pub owned_states: Vec<u32>,
    pub capital_state: u32,
    /// 政治点(累积值, 无 modifier 叠加; 原版范围 -500..2000)
    pub political_power: f64,
    /// 基础稳定度(0.0-1.0; 受事件/国策改 base, modifier 读取时叠加)
    pub stability: f64,
    /// 基础战争支持度(0.0-1.0)
    pub war_support: f64,
    /// 装备库存(M4a): equipment_type → 数量
    pub equipment_stockpile: std::collections::HashMap<String, f64>,
    /// 人力池(陆战循环): 国家征召的兵员储备
    pub manpower_pool: f64,
    /// modifier 汇总(科技/精神/ideas 等国家级修正; 战斗+资源修正统一栈)
    pub modifiers: crate::combat::modifier::ModifierStack,
    /// 阵营名(None = 不在阵营; 宣战时同阵营成员自动加入)
    pub faction: Option<String>,
}

impl Default for Country {
    fn default() -> Self {
        Self {
            tag: String::new(),
            owned_states: Vec::new(),
            capital_state: 0,
            political_power: 0.0,
            stability: 0.5,      // 原版 BASE_STABILITY
            war_support: 0.5,    // 原版 BASE_WAR_SUPPORT
            equipment_stockpile: Default::default(),
            manpower_pool: 0.0,
            modifiers: Default::default(),
            faction: None,
        }
    }
}
```

- [ ] **Step 4: 加 effective 读取方法**

在 `src/runtime/entities.rs` 的 `Country` 结构后(在 `pub struct Battle` 之前)加 impl 块:

```rust
impl Country {
    /// 有效稳定度 = clamp(base × 资源modifier, 0, 1)。trigger/UI 读此值。
    pub fn effective_stability(&self) -> f64 {
        let raw = self.stability * self.modifiers.multiplier(crate::combat::modifier::ModifierStat::Stability);
        raw.clamp(0.0, 1.0)
    }
    /// 稳定度 buffer(超 100% 部分, 抵御未来负修正; 对齐原版)
    pub fn stability_buffer(&self) -> f64 {
        let raw = self.stability * self.modifiers.multiplier(crate::combat::modifier::ModifierStat::Stability);
        (raw - 1.0).max(0.0)
    }
    /// 有效战争支持度 = clamp(base × modifier, 0, 1)
    pub fn effective_war_support(&self) -> f64 {
        let raw = self.war_support * self.modifiers.multiplier(crate::combat::modifier::ModifierStat::WarSupport);
        raw.clamp(0.0, 1.0)
    }
    /// 有效政治点 = base × modifier(不 clamp; 累积值)
    pub fn effective_political_power(&self) -> f64 {
        self.political_power * self.modifiers.multiplier(crate::combat::modifier::ModifierStat::PoliticalPower)
    }
}
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test --lib t_country_default_resources t_effective_stability_clamp_and_buffer t_country_has_per_instance_resources`
Expected: 3 个全 PASS

- [ ] **Step 6: 全量回归**

Run: `cargo test`
Expected: 191 全绿(新字段有 Default,现有构造点不破)

- [ ] **Step 7: 提交**

```bash
git add src/runtime/entities.rs src/runtime/world.rs
git commit -m "feat(country): Country 加 PP/稳定度/战争支持度字段 + effective 读取方法

- political_power/stability/war_support 具名字段(存 base 值)
- 手写 Default: stability/war_support=0.5(原版 BASE_*)
- effective_* 方法: base×modifier, 稳定度/战争支持度 clamp 0..1, buffer 保留超额
- trigger/UI 读 effective, 命令改 base"
```

---

## Task 3: 加 current_country_tag 辅助(world.rs)

**Files:**
- Modify: `src/runtime/world.rs`(加方法)

**说明:** 资源命令需要"快照当前国家 tag 到 owned String"。加一个返回 `Option<String>` 的辅助,使命令代码更干净。

- [ ] **Step 1: 加辅助方法**

在 `src/runtime/world.rs` 的 `current_country()` 方法(约 108-116 行)之后加:

```rust
    /// 当前国家 tag 的 owned 副本(供命令快照后 get_mut, 避借用冲突)。
    /// 语义同 current_country(): 栈优先回退 player_tag; 无则 None。
    pub fn current_country_tag(&self) -> Option<String> {
        self.current_country().map(|s| s.to_string())
    }
```

- [ ] **Step 2: 跑测试确认编译通过**

Run: `cargo build`
Expected: 编译成功(无破坏)

- [ ] **Step 3: 提交**

```bash
git add src/runtime/world.rs
git commit -m "feat(world): 加 current_country_tag 辅助(owned String 快照)

供资源命令快照当前国家 tag 后释放借用, 再 get_mut Country。
语义同 current_country(): 栈优先回退 player_tag。"
```

---

## Task 4: 资源命令改造(vars.rs)— add/set_political_power + add/set_stability

**Files:**
- Modify: `src/commands/vars.rs:8-27`(4 个命令)
- Test: `src/commands/vars.rs`(inline test mod)

**说明:** 把 4 个全局命令改成读写当前作用域国家。无国家时返回 `CmdError`。

- [ ] **Step 1: 写失败的作用域化测试**

在 `src/commands/vars.rs` 的 `#[cfg(test)] mod tests` 末尾加:

```rust
    #[test]
    fn t_add_political_power_targets_scope_country() {
        // add_political_power 改当前作用域国家(player_tag 回退), 不是全局
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        w.player_tag = "GER".into();
        w.countries.insert("GER".into(), Default::default());
        let f = reg.get_effect("add_political_power").unwrap();
        f(&mut w, &[("".into(), Arg::Num(50.0))]).unwrap();
        let pp = w.countries.get("GER").unwrap().political_power;
        assert!((pp - 50.0).abs() < 1e-9, "PP 应加到 GER 国家");
    }

    #[test]
    fn t_add_stability_targets_scope_country() {
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        w.player_tag = "GER".into();
        w.countries.insert("GER".into(), Default::default());
        let f = reg.get_effect("add_stability").unwrap();
        f(&mut w, &[("".into(), Arg::Num(0.1))]).unwrap();
        let stab = w.countries.get("GER").unwrap().stability;
        // 默认 0.5 + 0.1 = 0.6
        assert!((stab - 0.6).abs() < 1e-9, "稳定度应 0.5+0.1=0.6, 实际 {}", stab);
    }

    #[test]
    fn t_resource_command_errors_without_country() {
        // 无国家作用域(player_tag 空) → 报错(决策5: 不静默吞)
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        w.player_tag = String::new(); // 无国家
        let f = reg.get_effect("add_political_power").unwrap();
        let result = f(&mut w, &[("".into(), Arg::Num(50.0))]);
        assert!(result.is_err(), "无国家时 add_political_power 应返回 Err");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --lib t_add_political_power_targets_scope_country`
Expected: FAIL(现写全局 vars,GER 的 political_power 仍是 0)

- [ ] **Step 3: 改造 4 个命令**

替换 `src/commands/vars.rs:7-27`(register 函数前 4 个命令注册)。需要先在文件顶部确认 `use crate::runtime::World`(已有)。把:

```rust
pub fn register(reg: &mut Registry) {
    reg.register("set_stability", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("set_stability"))?;
        w.set_var("stability", n);
        Ok(())
    });
    reg.register("add_stability", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("add_stability"))?;
        w.add_var("stability", n);
        Ok(())
    });
    reg.register("add_political_power", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("add_political_power"))?;
        w.add_var("political_power", n);
        Ok(())
    });
    reg.register("set_political_power", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("set_political_power"))?;
        w.set_var("political_power", n);
        Ok(())
    });
```

改成:

```rust
/// 取当前作用域国家的可变引用(栈优先回退 player_tag)。
/// 无国家时返回 RuntimeError(决策5: 不静默吞)。
fn scope_country_mut(w: &mut crate::runtime::World) -> Result<&mut crate::runtime::Country, crate::runtime::error::CmdError> {
    let tag = w.current_country_tag()
        .ok_or_else(|| crate::runtime::error::CmdError::RuntimeError(
            "资源命令需要国家作用域(player_tag 空或无 Country scope)".into()
        ))?;
    w.countries.entry(tag.clone()).or_default();
    w.countries.get_mut(&tag)
        .map_err(|_| crate::runtime::error::CmdError::RuntimeError(format!("国家 {tag} 不存在")))
}

pub fn register(reg: &mut Registry) {
    reg.register("set_stability", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("set_stability"))?;
        scope_country_mut(w)?.stability = n;
        Ok(())
    });
    reg.register("add_stability", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("add_stability"))?;
        scope_country_mut(w)?.stability += n;
        Ok(())
    });
    reg.register("add_political_power", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("add_political_power"))?;
        scope_country_mut(w)?.political_power += n;
        Ok(())
    });
    reg.register("set_political_power", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("set_political_power"))?;
        scope_country_mut(w)?.political_power = n;
        Ok(())
    });
```

- [ ] **Step 4: 跑新测试确认通过**

Run: `cargo test --lib t_add_political_power_targets_scope_country t_add_stability_targets_scope_country t_resource_command_errors_without_country`
Expected: 3 个全 PASS

- [ ] **Step 5: 迁移 vars.rs 旧测试**

`src/commands/vars.rs` 的旧测试 `t_add_stability_cmd`(72-80)和 `t_add_to_variable_named_field`(82-90)现在会失败(它们读 `world.get_var("stability")`)。改造:

`t_add_stability_cmd` 改成:
```rust
    #[test]
    fn t_add_stability_cmd() {
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        w.player_tag = "X".into();          // ★ 设国家作用域
        w.countries.insert("X".into(), Default::default());
        let f = reg.get_effect("add_stability").unwrap();
        f(&mut w, &[("".into(), Arg::Num(0.05))]).unwrap();
        // ★ 读 Country 字段, 非全局 var
        let stab = w.countries.get("X").unwrap().stability;
        assert!((stab - 0.55).abs() < 1e-9, "默认0.5+0.05=0.55"); // 默认 0.5 + 0.05
    }
```

`t_command_returns_error_on_bad_param`(92-101)不动(它测空参数报错,与国家无关)。
`t_add_to_variable_named_field`(82-90)测的是 `add_to_variable`(通用变量命令,仍走全局 `world.add_var`)——**不动**(add_to_variable 不是资源命令)。

- [ ] **Step 6: 跑 vars 全部测试**

Run: `cargo test --lib commands::vars`
Expected: 全 PASS

- [ ] **Step 7: 提交**

```bash
git add src/commands/vars.rs
git commit -m "feat(cmd): 资源命令(add/set_stability, add/set_political_power)改国家级

- 从 world.get_var 全局 → current_country_tag 国家字段
- 无国家作用域时返回 CmdError(决策5: 不静默吞)
- 迁移 t_add_stability_cmd 到国家级(player_tag 兜底)"
```

---

## Task 5: 新增 create_country + add/set_war_support 命令

**Files:**
- Modify: `src/commands/vars.rs`(register 函数加 3 命令)
- Test: `src/commands/vars.rs`(inline test mod)

- [ ] **Step 1: 写失败的 create_country 测试**

在 `src/commands/vars.rs` 测试 mod 加:

```rust
    #[test]
    fn t_create_country_sets_resources() {
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        let f = reg.get_effect("create_country").unwrap();
        f(&mut w, &[
            ("tag".into(), Arg::Str("GER".into())),
            ("political_power".into(), Arg::Num(50.0)),
            ("stability".into(), Arg::Num(0.7)),
            ("war_support".into(), Arg::Num(0.3)),
            ("capital_state".into(), Arg::Num(1.0)),
        ]).unwrap();
        let c = w.countries.get("GER").unwrap();
        assert!((c.political_power - 50.0).abs() < 1e-9);
        assert!((c.stability - 0.7).abs() < 1e-9);
        assert!((c.war_support - 0.3).abs() < 1e-9);
        assert_eq!(c.capital_state, 1);
        assert_eq!(c.tag, "GER");
    }

    #[test]
    fn t_add_war_support_targets_country() {
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        w.player_tag = "GER".into();
        w.countries.insert("GER".into(), Default::default());
        let f = reg.get_effect("add_war_support").unwrap();
        f(&mut w, &[("".into(), Arg::Num(0.1))]).unwrap();
        let ws = w.countries.get("GER").unwrap().war_support;
        assert!((ws - 0.6).abs() < 1e-9, "默认0.5+0.1=0.6");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --lib t_create_country_sets_resources`
Expected: FAIL(create_country 未注册 → UnknownCommand)

- [ ] **Step 3: 加 3 个命令注册**

在 `src/commands/vars.rs` 的 `register` 函数里(`set_political_power` 之后、`add_to_variable` 之前)加:

```rust
    reg.register("create_country", |w, p| {
        let tag = p.get("tag").and_then(Arg::as_str)
            .ok_or_else(|| crate::runtime::error::CmdError::BadParam {
                cmd: "create_country".into(), key: "tag".into(), reason: "缺少 tag".into()
            })?;
        let pp = p.get("political_power").and_then(Arg::as_num).unwrap_or(0.0);
        let stab = p.get("stability").and_then(Arg::as_num).unwrap_or(0.5);
        let ws = p.get("war_support").and_then(Arg::as_num).unwrap_or(0.5);
        let cap = p.get("capital_state").and_then(Arg::as_num).unwrap_or(0.0) as u32;
        // 已存在则覆盖资源字段(以最后一次为准, 对齐原版 history 加载语义)
        let c = w.countries.entry(tag.into()).or_default();
        c.tag = tag.into();
        c.political_power = pp;
        c.stability = stab;
        c.war_support = ws;
        c.capital_state = cap;
        Ok(())
    });
    reg.register("add_war_support", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("add_war_support"))?;
        scope_country_mut(w)?.war_support += n;
        Ok(())
    });
    reg.register("set_war_support", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("set_war_support"))?;
        scope_country_mut(w)?.war_support = n;
        Ok(())
    });
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test --lib t_create_country_sets_resources t_add_war_support_targets_country`
Expected: 2 个全 PASS

- [ ] **Step 5: 提交**

```bash
git add src/commands/vars.rs
git commit -m "feat(cmd): 新增 create_country + add/set_war_support 命令

- create_country: 建国家实体+设资源初值(字段可选, 缺省Default); 重复tag覆盖
- add/set_war_support: 同 stability 套路, 读写当前作用域国家"
```

---

## Task 6: trigger Compare 作用域化(interp.rs)

**Files:**
- Modify: `src/runtime/interp.rs:150-164`(Compare 分支)
- Test: `src/runtime/interp.rs` 或新测试

**说明:** 已知资源名(political_power/stability/war_support)读当前国家 effective,其他变量仍走全局。无国家时资源名返回 0.0(trigger 自然判 false,不报错——与命令报错不对称是刻意的)。

- [ ] **Step 1: 写失败的 trigger 测试**

在 `src/runtime/world.rs` 测试 mod 加(需要用 lower + interp 跑脚本):

```rust
    #[test]
    fn t_trigger_compare_reads_country_resource() {
        // trigger political_power >= 150 应读当前国家(player_tag)的 effective PP
        use crate::ast::lower::lower_effects;
        use crate::commands::register_all;
        use crate::runtime::{Interpreter, Registry};
        let mut reg = Registry::new();
        register_all(&mut reg);
        crate::combat::commands::register(&mut reg);
        let interp = Interpreter::new(reg);
        let mut w = World::new();
        w.player_tag = "GER".into();
        // GER 有 200 PP
        w.countries.insert("GER".into(), crate::runtime::Country {
            political_power: 200.0, ..Default::default()
        });
        // 脚本: if political_power >= 150 then set_flag done
        let src = "if = { limit = { political_power >= 150 } set_flag = done }";
        let b = crate::parser::parse(src).unwrap();
        let effs = lower_effects(&b);
        interp.run(&effs, &mut w);
        assert!(w.has_flag("done"), "GER PP=200 >= 150 应触发, flag=done");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --lib t_trigger_compare_reads_country_resource`
Expected: FAIL(现 trigger 读全局 political_power=0, 0>=150 false, flag 没设)

- [ ] **Step 3: 改造 Compare 分支**

修改 `src/runtime/interp.rs:150-164`。把:

```rust
            Trigger::Compare { lhs, op, rhs } => {
                let l = world.get_var(lhs);
                let r = match rhs {
                    Arg::Num(n) => *n,
                    _ => return Ok(false),
                };
```

改成:

```rust
            Trigger::Compare { lhs, op, rhs } => {
                // 已知资源名读当前国家 effective; 其他变量走全局。
                // 无国家作用域时资源视为 0(trigger 自然判 false, 不报错——与命令报错不对称是刻意的)。
                let l = match lhs.as_str() {
                    "political_power" => world.current_country()
                        .and_then(|t| world.countries.get(t))
                        .map(|c| c.effective_political_power())
                        .unwrap_or(0.0),
                    "stability" => world.current_country()
                        .and_then(|t| world.countries.get(t))
                        .map(|c| c.effective_stability())
                        .unwrap_or(0.0),
                    "war_support" => world.current_country()
                        .and_then(|t| world.countries.get(t))
                        .map(|c| c.effective_war_support())
                        .unwrap_or(0.0),
                    other => world.get_var(other),
                };
                let r = match rhs {
                    Arg::Num(n) => *n,
                    _ => return Ok(false),
                };
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test --lib t_trigger_compare_reads_country_resource`
Expected: PASS

- [ ] **Step 5: 全量回归**

Run: `cargo test`
Expected: 除 clock.rs/integration.rs 的 PP 相关测试可能红外(下个 Task 迁移),其余绿

- [ ] **Step 6: 提交**

```bash
git add src/runtime/interp.rs src/runtime/world.rs
git commit -m "feat(trigger): Compare 作用域化 — 资源名读国家 effective

political_power/stability/war_support 读当前国家(player_tag回退)的 effective 值;
其他变量仍走全局。无国家时资源返回 0.0(trigger 自然判 false, 不报错)。"
```

---

## Task 7: 迁移 clock.rs + integration.rs 现有测试

**Files:**
- Modify: `src/runtime/clock.rs:50-90`(2 个测试)
- Modify: `tests/integration.rs`(`focus_add_pp_then_stability` 测试)

**说明:** 这些测试用全局 political_power,现在 trigger 读国家级,需补国家作用域。clock.rs 的 `on_daily` 钩子测试尤其要确保 player_tag 已设。

- [ ] **Step 1: 迁移 clock.rs 两个测试**

`src/runtime/clock.rs:50-90` 的 `t_daily_hook_fires_after_24_ticks` 和 `t_hourly_fires_every_tick`。两个都先建国家作用域。改 setup 部分(在 `let mut world = World::new();` 后加):

```rust
        world.player_tag = "GER".into();
        world.countries.insert("GER".into(), Default::default());
```

然后把钩子里的命令从 `add_political_power`(现已是国家级)继续用——它现在会加到 GER。断言改读国家字段:

`t_daily_hook_fires_after_24_ticks` 断言改成:
```rust
        let pp = world.countries.get("GER").unwrap().political_power;
        assert!((pp - 1.0).abs() < 1e-9, "24h 后 on_daily 应触发, PP=1.0");
```
(原来是 `world.get_var("political_power")`)

`t_hourly_fires_every_tick` 断言改成:
```rust
        let pp = world.countries.get("GER").unwrap().political_power;
        assert!((pp - 5.0).abs() < 1e-9, "10 tick 应加 5.0 PP");
```
(原来的 `assert!(world.get_var("political_power").abs() < 1e-9)` 也要相应改成查 GER 国家字段,初始 0)

- [ ] **Step 2: 跑 clock 测试**

Run: `cargo test --lib clock`
Expected: 全 PASS

- [ ] **Step 3: 迁移 integration.rs 的 focus_add_pp_then_stability**

看 `tests/integration.rs` 的 `focus_add_pp_then_stability`(约 40 行附近)。它的 setup 建了省份,但国家靠隐式 or_default。确保 `world.player_tag` 已设(脚本命令才能命中国家)。在该测试的 world 构造后加:

```rust
    world.player_tag = "AFG".into();
    world.countries.insert("AFG".into(), Default::default());
```

(该测试用 AFG 国策脚本,player_tag 设 AFG 让 add_political_power 命中 AFG 国家)

然后断言里若读 `world.get_var("political_power")` 或 `world.get_var("stability")`,改成读 AFG 国家字段:
```rust
    let stab = world.countries.get("AFG").unwrap().stability;
    assert!((stab - 0.55).abs() < 1e-9, "AFG 稳定度应 0.5+0.05");  // 默认0.5+0.05
```
(原断言可能是 `add_stability 0.05`,现在 base 从 0.5 起算)

- [ ] **Step 4: 跑 integration 测试**

Run: `cargo test --test integration`
Expected: 全 PASS

- [ ] **Step 5: 全量回归**

Run: `cargo test`
Expected: 191 全绿(所有迁移完成)

- [ ] **Step 6: 提交**

```bash
git add src/runtime/clock.rs tests/integration.rs
git commit -m "test: 迁移 clock/integration 测试到国家级资源

clock.rs on_daily 钩子测试 + integration focus_add_pp_then_stability
补国家作用域(player_tag), 断言改读 Country 字段。"
```

---

## Task 8: 序列化资源字段(wasm_api.rs)

**Files:**
- Modify: `src/wasm_api.rs`(countries 序列化处,约 542 行)

**说明:** `get_state` 序列化 countries 时加资源字段(effective 值,供顶栏 UI)。

- [ ] **Step 1: 找到 countries 序列化位置**

Run: `grep -n "factions" src/wasm_api.rs`
找到序列化 factions 的循环(约 542 行)。资源字段加在该循环之前或之后(countries 数组序列化处)。

- [ ] **Step 2: 加资源字段序列化**

在 countries 序列化数组里,每个 country 对象加三个字段。具体改法依现有代码结构——找到序列化单个 country 的地方(可能在 factions 循环之前有个 countries 数组序列化),加:

```rust
// 在序列化每个 country 的字段后加(format! 拼 JSON):
"political_power":{country.effective_political_power()},
"stability":{country.effective_stability()},
"war_support":{country.effective_war_support()},
```

注:wasm_api.rs 是手工拼 JSON 字符串(非 serde)。若找不到单独的 country 序列化点,可能要先加一个 countries 数组序列化(目前可能只序列化 factions 映射)。需现场看代码结构。

- [ ] **Step 3: 验证 WASM 构建**

Run: `cargo build --target wasm32-unknown-unknown --lib --release`
Expected: 编译成功 0 警告

- [ ] **Step 4: 提交**

```bash
git add src/wasm_api.rs
git commit -m "feat(wasm): get_state 序列化国家资源字段(PP/稳定度/战争支持度)

序列化 effective 值(带 modifier), 供顶栏 UI 显示玩家可见值。
UI 显示本身不在本轮范围, 本轮只保证数据可达。"
```

---

## Task 9: 全量验证 + HANDOFF 更新

**Files:**
- Modify: `docs/HANDOFF.md`

- [ ] **Step 1: 全量测试**

Run: `cargo test`
Expected: 191 全绿(若 Task 2/4/6 加了新测试,数量会增加,记下新总数)

- [ ] **Step 2: WASM 构建验证**

Run: `cargo build --target wasm32-unknown-unknown --lib --release`
Expected: 0 警告

- [ ] **Step 3: 更新 HANDOFF**

在 `docs/HANDOFF.md` 的里程碑表(§1)加一行,并在 §3 接口表更新"国家资源"行(原标"全局变量",改"国家级")。在 §0 测试数更新为新总数。加一个"P0-1 国家资源重构"小节(简述:三件套国家级 + modifier 打通 + 决策要点)。

- [ ] **Step 4: 提交**

```bash
git add docs/HANDOFF.md
git commit -m "docs(handoff): P0-1 国家资源模型重构完成 — 三件套国家级 + modifier 打通"
```

---

## 自审

**Spec 覆盖检查:**
- §1 Country 字段 → Task 2 ✅
- §1 ModifierStat 扩展 → Task 1 ✅
- §1 effective 方法 → Task 2 ✅
- §2 命令改造 → Task 4 ✅
- §2 create_country + war_support → Task 5 ✅
- §2 trigger Compare 作用域化 → Task 6 ✅
- §3 modifier 接口零命令改动 → Task 1(parse_modifier_token)+ 复用 add_country_modifier(已存在)✅
- §3 CombatContext 不受影响 → 无需任务(自然成立,multiplier 按 stat 过滤)✅
- §5 序列化 → Task 8 ✅
- §6 测试 A 类新增 → Task 1/2/4/5/6 各自含 ✅
- §6 测试 B 类迁移 → Task 4(vars)+ Task 7(clock/integration)✅
- §6 测试 C 类不动 → 无需任务 ✅

**Placeholder 扫描:** Task 8 Step 2 提到"依现有代码结构""需现场看代码结构"——这是唯一略含糊处,因 wasm_api.rs 手工拼 JSON,具体插入点需执行时确认。已标注验证方式(WASM 构建)。其余步骤均有完整代码。

**类型一致性:** `scope_country_mut` 在 Task 4 定义,Task 5 复用 ✅。`current_country_tag` 在 Task 3 定义,Task 4 复用 ✅。`effective_*` 方法 Task 2 定义,Task 6 复用 ✅。Country 字段名(political_power/stability/war_support)全 plan 一致 ✅。
