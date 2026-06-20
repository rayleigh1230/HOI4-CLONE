# M2 运行时基础设施重构 + 主循环 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 修复 M1 review 指出的 3 个 P0 缺陷(结构化参数/Trigger Registry/可失败命令),并搭建主循环骨架,为 M3 战斗引擎铺好健壮地基。

**Architecture:** 渐进式重构——先改类型签名(P0-1/2/3),迁移 M1 命令到新 API,再加主循环(GameClock + EventBus)。每步保持测试绿,绝不大爆炸式重写。

**Tech Stack:** Rust 2021,零外部依赖(沿用 M1),`stable-x86_64-pc-windows-gnu`。

**Spec 依据:** `docs/specs/2026-06-20-m2-runtime-refactor.md`

---

## 文件结构(变更)

```
src/
├── ast/
│   ├── effect.rs        [改] Arg 加 Block 变体; Effect::Command 用 params
│   └── lower.rs         [改] 降级产生命名字段参数; 嵌套块递归
├── runtime/
│   ├── world.rs         [改] 加 hour/player_tag/error_log/event_bus
│   ├── registry.rs      [改] 加 triggers 表; 命令签名 → Result
│   ├── interp.rs        [改] 求值 Check 查表; 收集 CmdError; fire_event
│   └── clock.rs         [新] GameClock + tick 逻辑
├── commands/
│   └── vars.rs          [改] 7 命令迁移到新签名
└── parser/              [不改] lexer/block 不动
```

---

## Task 1: CmdError 类型 + Arg 扩展

**Files:**
- Create: `src/runtime/error.rs`
- Modify: `src/ast/effect.rs`
- Modify: `src/runtime/mod.rs`

- [ ] **Step 1: 创建 src/runtime/error.rs**

```rust
//! 命令执行错误类型
use std::fmt;

#[derive(Debug)]
pub enum CmdError {
    UnknownCommand(String),
    BadParam { cmd: String, key: String, reason: String },
    RuntimeError(String),
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CmdError::UnknownCommand(c) => write!(f, "未注册的命令: {c}"),
            CmdError::BadParam { cmd, key, reason } => {
                write!(f, "命令 {cmd} 参数 {key} 错误: {reason}")
            }
            CmdError::RuntimeError(m) => write!(f, "运行时错误: {m}"),
        }
    }
}

impl std::error::Error for CmdError {}
```

- [ ] **Step 2: 改 src/ast/effect.rs —— Arg 加 Block,Command 用 params**

替换整个 effect.rs 内容:

```rust
//! Effect: 改变世界状态的命令。对应原版 effect 块。
use crate::ast::trigger::Trigger;

#[derive(Debug, Clone)]
pub enum Effect {
    /// 基础命令。params 为命名字段;位置参数用空 key ("", Arg)
    Command { name: String, params: Vec<(String, Arg)> },
    /// if = { limit = { ... } <then> else = { ... } }
    If { cond: Trigger, then: Vec<Effect>, els: Vec<Effect> },
    /// 作用域遍历: every_owned_state = { limit = {...} <body> }
    ForEach { scope: String, filter: Option<Trigger>, body: Vec<Effect> },
    /// random_events = { 100 = xxx 100 = yyy }
    Random { table: Vec<(f64, RandomPick)> },
}

#[derive(Debug, Clone)]
pub enum Arg {
    Num(f64),
    Str(String),
    Bool(bool),
    /// 嵌套块参数: add_equipment_production = { equipment=... count=10 }
    Block(Vec<(String, Arg)>),
}

impl Arg {
    /// 辅助:取 Num 值
    pub fn as_num(&self) -> Option<f64> {
        if let Arg::Num(n) = self { Some(*n) } else { None }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Arg::Str(s) = self { Some(s) } else { None }
    }
}

#[derive(Debug, Clone)]
pub enum RandomPick {
    EventId(String),
    Nested(Vec<Effect>),
}
```

- [ ] **Step 3: 改 src/runtime/mod.rs 加 error 模块**

```rust
pub mod clock;
pub mod error;
pub mod interp;
pub mod registry;
pub mod world;

pub use clock::GameClock;
pub use error::CmdError;
pub use interp::Interpreter;
pub use registry::{EffectFn, Registry, TriggerFn};
pub use world::World;
```

- [ ] **Step 4: 验证编译(此时 lower/interp/registry/commands 会因签名变化报错,这是预期的)**

Run: `cargo check 2>&1 | grep -c "error"`
Expected: 多个错误(因为依赖方还没改),记下错误数,后续 Task 逐个消除

- [ ] **Step 5: 提交**

```bash
git add src/runtime/error.rs src/ast/effect.rs src/runtime/mod.rs
git commit -m "refactor(m2): CmdError 类型 + Arg::Block 变体 + Command 用 params"
```

---

## Task 2: Registry 加 triggers 表 + 新命令签名

**Files:**
- Modify: `src/runtime/registry.rs`

- [ ] **Step 1: 重写 registry.rs**

```rust
//! Registry: effect 和 trigger 命令注册表
use crate::ast::Arg;
use crate::runtime::error::CmdError;
use crate::runtime::World;
use std::collections::HashMap;

/// params: 命令参数(命名字段); 返回 Result 表达成功/失败
pub type EffectFn = fn(&mut World, &[(String, Arg)]) -> Result<(), CmdError>;
/// trigger 求值: 返回 bool + 可能的错误
pub type TriggerFn = fn(&World, &[(String, Arg)]) -> Result<bool, CmdError>;

/// 命令参数辅助取值 trait
pub trait ParamGet {
    fn pos(&self, i: usize) -> Option<&Arg>;
    fn get(&self, key: &str) -> Option<&Arg>;
}
impl ParamGet for [(String, Arg)] {
    fn pos(&self, i: usize) -> Option<&Arg> {
        self.iter().nth(i).map(|(_, v)| v)
    }
    fn get(&self, key: &str) -> Option<&Arg> {
        self.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

#[derive(Default)]
pub struct Registry {
    pub effects: HashMap<String, EffectFn>,
    pub triggers: HashMap<String, TriggerFn>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, name: &str, f: EffectFn) {
        self.effects.insert(name.to_string(), f);
    }
    pub fn register_trigger(&mut self, name: &str, f: TriggerFn) {
        self.triggers.insert(name.to_string(), f);
    }
    pub fn get_effect(&self, name: &str) -> Option<&EffectFn> {
        self.effects.get(name)
    }
    pub fn get_trigger(&self, name: &str) -> Option<&TriggerFn> {
        self.triggers.get(name)
    }
}
```

- [ ] **Step 2: 提交**

```bash
git add src/runtime/registry.rs
git commit -m "refactor(m2): Registry 加 triggers 表 + Result 命令签名"
```

---

## Task 3: lower.rs 适配新 Arg/Command 结构

**Files:**
- Modify: `src/ast/lower.rs`

核心改动: `key = scalar` → params 用 `[("", arg)]`; `key = {k=v}` → params 用 `[("k", arg)]` 且嵌套递归。

- [ ] **Step 1: 改 lower_field_as_effect 的命令分支**

找到 `lower_field_as_effect` 中两处生成 `Effect::Command` 的代码:

```rust
// 改前(标量)
(k, Value::Scalar(s)) => {
    out.push(Effect::Command { name: k.clone(), args: vec![parse_arg(s)] });
}
// 改前(块参数)
(k, Value::Block(inner)) => {
    let args = inner.fields.iter()
        .map(|f| Arg::Str(format!("{}={}", f.key, scalar_str(&f.value)))).collect();
    out.push(Effect::Command { name: k.clone(), args });
}
```

替换为:

```rust
// 改后(标量): 位置参数用空 key
(k, Value::Scalar(s)) => {
    out.push(Effect::Command { name: k.clone(), params: vec![(String::new(), parse_arg(s))] });
}
// 改后(块参数): 命名字段, 嵌套块递归成 Arg::Block
(k, Value::Block(inner)) => {
    let params = inner.fields.iter()
        .map(|f| (f.key.clone(), parse_value(&f.value))).collect();
    out.push(Effect::Command { name: k.clone(), params });
}
```

- [ ] **Step 2: 加 parse_value 辅助函数(替代 scalar_str)**

在 lower.rs 末尾(parse_arg 附近)加:

```rust
/// 把 Value 递归转 Arg, 嵌套块 → Arg::Block (P0-1 修复:不再扁平化丢数据)
fn parse_value(v: &Value) -> Arg {
    match v {
        Value::Scalar(s) => parse_arg(s),
        Value::Block(b) => {
            Arg::Block(b.fields.iter().map(|f| (f.key.clone(), parse_value(&f.value))).collect())
        }
    }
}
```

删除旧的 `scalar_str` 函数(不再需要)。

- [ ] **Step 3: 更新 lower.rs 的单元测试 t_lower_string_arg**

```rust
#[test]
fn t_lower_string_arg() {
    let b = parse(r#"set_country_name = "Germany""#).unwrap();
    let effs = lower_effects(&b);
    match &effs[0] {
        Effect::Command { name, params } => {
            assert_eq!(name, "set_country_name");
            assert!(matches!(params[0].1, Arg::Str(ref s) if s == "Germany"));
        }
        _ => panic!(),
    }
}
```

- [ ] **Step 4: 加嵌套块参数测试(P0-1 验证)**

在 lower.rs tests 模块加:

```rust
#[test]
fn t_lower_nested_block_param_no_data_loss() {
    // P0-1 回归: 嵌套块参数不能丢数据
    let src = "add_equipment_production = { equipment_type = infantry_weapons amount = 10 }";
    let b = parse(src).unwrap();
    let effs = lower_effects(&b);
    match &effs[0] {
        Effect::Command { name, params } => {
            assert_eq!(name, "add_equipment_production");
            // 嵌套块 → Arg::Block, 两个字段都在
            assert_eq!(params.len(), 2);
            let block_arg = params.iter().find(|(k, _)| k == "amount");
            assert!(matches!(block_arg, Some((_, Arg::Num(n))) if (*n - 10.0).abs() < 1e-9));
        }
        _ => panic!("应为 Command"),
    }
}
```

- [ ] **Step 5: 运行 lower 测试**

Run: `cargo test --lib ast::lower 2>&1 | tail -10`
Expected: lower 的 5 个测试(原4 + 新1)PASS。其他模块仍报错(interp/commands 未改)

- [ ] **Step 6: 提交**

```bash
git add src/ast/lower.rs
git commit -m "refactor(m2): lower 适配 params 结构 + 嵌套块递归(P0-1 修复)"
```

---

## Task 4: interp.rs 适配新签名 + Check 查表 + 错误收集

**Files:**
- Modify: `src/runtime/interp.rs`

- [ ] **Step 1: 重写 interp.rs**

```rust
//! Interpreter: 解释执行 Effect AST
use crate::ast::{Arg, CompareOp, Effect, Trigger};
use crate::runtime::error::CmdError;
use crate::runtime::registry::ParamGet;
use crate::runtime::{Registry, World};

pub struct Interpreter {
    pub reg: Registry,
}

impl Interpreter {
    pub fn new(reg: Registry) -> Self {
        Self { reg }
    }

    pub fn run(&self, effs: &[Effect], world: &mut World) {
        for e in effs {
            if let Err(err) = self.run_one(e, world) {
                world.error_log.push(err);
            }
        }
    }

    fn run_one(&self, e: &Effect, world: &mut World) -> Result<(), CmdError> {
        match e {
            Effect::Command { name, params } => match self.reg.get_effect(name) {
                Some(f) => f(world, params),
                None => {
                    eprintln!("[warn] 未注册的 effect: {name}");
                    Err(CmdError::UnknownCommand(name.clone()))
                }
            },
            Effect::If { cond, then, els } => {
                if self.eval(cond, world)? {
                    self.run(then, world);
                } else {
                    self.run(els, world);
                }
                Ok(())
            }
            Effect::ForEach { scope, filter, body } => {
                // M2: 仍简化为单次执行(M3 接入实体枚举)
                let pass = match filter {
                    Some(t) => self.eval(t, world)?,
                    None => true,
                };
                if pass {
                    eprintln!("[info] {scope}: 执行作用域体(M2 简化为单次)");
                    self.run(body, world);
                }
                Ok(())
            }
            Effect::Random { table } => {
                if let Some((_, crate::ast::RandomPick::EventId(id))) = table.first() {
                    eprintln!("[info] random_events 选中: {id} (M2 不触发事件)");
                }
                Ok(())
            }
        }
    }

    pub fn eval(&self, t: &Trigger, world: &World) -> Result<bool, CmdError> {
        match t {
            Trigger::Always(b) => Ok(*b),
            Trigger::And(parts) => {
                for p in parts {
                    if !self.eval(p, world)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Trigger::Or(parts) => {
                for p in parts {
                    if self.eval(p, world)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Trigger::Not(inner) => Ok(!self.eval(inner, world)?),
            Trigger::Compare { lhs, op, rhs } => {
                let l = world.get_var(lhs);
                let r = match rhs {
                    Arg::Num(n) => *n,
                    _ => return Ok(false),
                };
                Ok(match op {
                    CompareOp::Lt => l < r,
                    CompareOp::Gt => l > r,
                    CompareOp::Le => l <= r,
                    CompareOp::Ge => l >= r,
                    CompareOp::Eq => (l - r).abs() < 1e-9,
                    CompareOp::Ne => (l - r).abs() >= 1e-9,
                })
            }
            // P0-2: Check 查 triggers 表, 未注册则 false(保守)
            Trigger::Check { name, args } => match self.reg.get_trigger(name) {
                Some(f) => f(world, args),
                None => {
                    eprintln!("[debug] 未注册的 trigger: {name}, 默认 false");
                    Ok(false)
                }
            },
        }
    }
}
```

- [ ] **Step 2: 验证编译(interp 不再报错,但 world 缺 error_log 字段)**

Run: `cargo check 2>&1 | grep "error\[" | head`
Expected: world.rs 缺 error_log 字段的错误(下个 Task 修)

- [ ] **Step 3: 提交**

```bash
git add src/runtime/interp.rs
git commit -m "refactor(m2): interp 适配 Result + Check 查表(P0-2/3)"
```

---

## Task 5: World 扩展(error_log + hour + player_tag + event_bus)

**Files:**
- Modify: `src/runtime/world.rs`

- [ ] **Step 1: 扩展 World 结构**

在 world.rs 的 World struct 定义中加字段(保留所有 M1 字段):

```rust
use crate::runtime::error::CmdError;
use crate::ast::Effect;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct World {
    // M1 字段(保留)
    pub vars: HashMap<String, f64>,
    pub flags: HashMap<String, bool>,
    pub strings: HashMap<String, String>,
    // M2 新增
    pub hour: u64,
    pub player_tag: String,
    pub error_log: Vec<CmdError>,
    pub event_bus: HashMap<String, Vec<Effect>>, // on_action 钩子名 → effects
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }
    // M1 方法保留: set_var/get_var/add_var/set_flag/has_flag/set_string/get_string

    /// M2: 注册事件钩子
    pub fn on(&mut self, event: &str, effs: Vec<Effect>) {
        self.event_bus.entry(event.to_string()).or_default().extend(effs);
    }
    /// M2: 触发事件钩子(执行注册的 effects)
    pub fn fire_event(&mut self, interp: &crate::runtime::Interpreter, event: &str) {
        if let Some(effs) = self.event_bus.get(event) {
            let effs = effs.clone();
            interp.run(&effs, self);
        }
    }
}
```

保留所有 M1 的 set_var/get_var/add_var/set_flag/has_flag/set_string/get_string 方法(原样)。
删除 `scope_stack` 字段(M1 review P3-12:死字段)。

- [ ] **Step 2: 更新 world.rs 测试(M1 测试不变,加 error_log 默认空测试)**

在 tests 模块加:

```rust
#[test]
fn t_error_log_starts_empty() {
    let w = World::new();
    assert!(w.error_log.is_empty());
    assert_eq!(w.hour, 0);
}
```

- [ ] **Step 3: 运行 world 测试**

Run: `cargo test --lib runtime::world 2>&1 | tail -8`
Expected: 4 个测试(原3 + 新1)PASS

- [ ] **Step 4: 提交**

```bash
git add src/runtime/world.rs
git commit -m "refactor(m2): World 加 error_log/hour/event_bus, 删 scope_stack"
```

---

## Task 6: 迁移 M1 命令到新签名

**Files:**
- Modify: `src/commands/vars.rs`

- [ ] **Step 1: 重写 vars.rs 的 7 个命令**

```rust
//! 变量类命令注册(M2 新签名)
use crate::ast::Arg;
use crate::runtime::error::CmdError;
use crate::runtime::registry::ParamGet;
use crate::runtime::Registry;

pub fn register(reg: &mut Registry) {
    reg.register("set_stability", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("set_stability", 0))?;
        w.set_var("stability", n);
        Ok(())
    });
    reg.register("add_stability", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("add_stability", 0))?;
        w.add_var("stability", n);
        Ok(())
    });
    reg.register("add_political_power", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("add_political_power", 0))?;
        w.add_var("political_power", n);
        Ok(())
    });
    reg.register("set_political_power", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("set_political_power", 0))?;
        w.set_var("political_power", n);
        Ok(())
    });
    reg.register("add_to_variable", |w, p| {
        // M2: 现在用命名字段 params: [("x", Num(0.05))]
        let key = p.pos(0).map(|a| a.as_str()).flatten()
            .ok_or_else(|| bad_param("add_to_variable", 0))?;
        // 注意:add_to_variable 的第一个 param 可能是 ("", Block([(x,0.05)]))
        apply_var_block(w, p, "add_to_variable", false)
    });
    reg.register("set_variable", |w, p| {
        apply_var_block(w, p, "set_variable", true)
    });
    reg.register("set_flag", |w, p| {
        let s = p.pos(0).and_then(Arg::as_str).ok_or_else(|| bad_param("set_flag", 0))?;
        w.set_flag(s);
        Ok(())
    });
}

/// 处理 add_to_variable/set_variable 的块参数
fn apply_var_block(w: &mut crate::runtime::World, p: &[(String, Arg)], cmd: &str, is_set: bool) -> Result<(), CmdError> {
    // 块参数在 pos(0) 或单个命名字段
    for (k, v) in p {
        if let Arg::Block(fields) = v {
            for (vk, vv) in fields {
                if let Some(n) = vv.as_num() {
                    if is_set { w.set_var(vk, n); } else { w.add_var(vk, n); }
                }
            }
        } else if !k.is_empty() {
            // 直接命名字段 x=0.05
            if let Some(n) = v.as_num() {
                if is_set { w.set_var(k, n); } else { w.add_var(k, n); }
            }
        }
    }
    let _ = cmd;
    Ok(())
}

fn bad_param(cmd: &str, i: usize) -> CmdError {
    CmdError::BadParam { cmd: cmd.to_string(), key: format!("pos[{i}]"), reason: "缺少或类型错误".into() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::World;

    #[test]
    fn t_add_stability_cmd() {
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        let f = reg.get_effect("add_stability").unwrap();
        f(&mut w, &[("".into(), Arg::Num(0.05))]).unwrap();
        assert!((w.get_var("stability") - 0.05).abs() < 1e-9);
    }

    #[test]
    fn t_add_to_variable_named_field() {
        // M2 新格式: params = [("x", Num(0.05))]
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        let f = reg.get_effect("add_to_variable").unwrap();
        f(&mut w, &[("AFG_x".into(), Arg::Num(0.05))]).unwrap();
        assert!((w.get_var("AFG_x") - 0.05).abs() < 1e-9);
    }

    #[test]
    fn t_command_returns_error_on_bad_param() {
        // P0-3 验证: 坏参数返回 Err
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        let f = reg.get_effect("add_stability").unwrap();
        let result = f(&mut w, &[]); // 空参数
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: 运行 vars 测试**

Run: `cargo test --lib commands::vars 2>&1 | tail -8`
Expected: 3 个测试 PASS

- [ ] **Step 3: 提交**

```bash
git add src/commands/vars.rs
git commit -m "refactor(m2): 7 个命令迁移到 Result + params 签名"
```

---

## Task 7: 全量测试修复 + 主循环(GameClock)

**Files:**
- Create: `src/runtime/clock.rs`
- Modify: `tests/integration.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: 创建 src/runtime/clock.rs**

```rust
//! 游戏主循环: hourly tick + on_actions 钩子分发(spec §4.2.1)
use crate::runtime::Interpreter;
use crate::runtime::World;

pub struct GameClock;

impl GameClock {
    /// 推进游戏 1 小时, 触发相应钩子
    pub fn tick(interp: &Interpreter, world: &mut World) {
        world.hour += 1;
        world.fire_event(interp, "on_hourly");

        // M3 接入: combat::resolve / production::produce / movement::update
        if world.hour % 24 == 0 {
            world.fire_event(interp, "on_daily");
            world.fire_event(interp, &format!("on_daily_{}", world.player_tag));
        }
        if world.hour % (24 * 7) == 0 {
            world.fire_event(interp, "on_weekly");
        }
        if world.hour % (24 * 30) == 0 {
            world.fire_event(interp, "on_monthly");
        }
    }

    /// 推进 n 小时
    pub fn advance(interp: &Interpreter, world: &mut World, hours: u64) {
        for _ in 0..hours {
            Self::tick(interp, world);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Arg, Effect};
    use crate::commands::register_all;
    use crate::runtime::{Interpreter, Registry, World};

    #[test]
    fn t_daily_hook_fires_after_24_ticks() {
        let mut reg = Registry::new();
        register_all(&mut reg);
        let interp = Interpreter::new(reg);
        let mut world = World::new();
        // 注册 on_daily 钩子: 加 1 政治点
        world.on("on_daily", vec![Effect::Command {
            name: "add_political_power".into(),
            params: vec![("".into(), Arg::Num(1.0))],
        }]);
        // tick 23 次:daily 不应触发
        GameClock::advance(&interp, &mut world, 23);
        assert!((world.get_var("political_power") - 0.0).abs() < 1e-9);
        // 第 24 次:daily 触发
        GameClock::tick(&interp, &mut world);
        assert!((world.get_var("political_power") - 1.0).abs() < 1e-9, "24h 后 on_daily 应触发");
    }

    #[test]
    fn t_hourly_fires_every_tick() {
        let mut reg = Registry::new();
        register_all(&mut reg);
        let interp = Interpreter::new(reg);
        let mut world = World::new();
        world.on("on_hourly", vec![Effect::Command {
            name: "add_political_power".into(),
            params: vec![("".into(), Arg::Num(0.5))],
        }]);
        GameClock::advance(&interp, &mut world, 10);
        assert!((world.get_var("political_power") - 5.0).abs() < 1e-9, "10 tick 应加 5.0");
    }
}
```

- [ ] **Step 2: 修复 tests/integration.rs 适配新签名**

集成测试里命令调用方式不变(通过 interp.run),但若直接构造 Effect 需用 params。
检查 integration.rs,把任何 `args:` 改成 `params:`(本 Task 的测试用 parse+lower,不直接构造 Effect,通常无需改)。

Run: `cargo test --test integration 2>&1 | tail -10`
Expected: 4 个集成测试 PASS(它们走 parse→lower→interp,签名变化已由 lower 适配)

- [ ] **Step 3: 修复 src/main.rs(若有直接构造 Effect 的地方)**

main.rs 走 parse→lower,通常无需改。运行验证:

Run: `cargo run --bin hoi4_demo 2>&1 | tail -5`
Expected: demo 正常输出

- [ ] **Step 4: 运行全部测试**

Run: `cargo test 2>&1 | grep "test result"`
Expected: 所有测试 PASS(lower 加1, world 加1, vars 加1, clock 加2, 集成4, 其余不变 ≈ 23+)

- [ ] **Step 5: 提交**

```bash
git add src/runtime/clock.rs tests/integration.rs src/main.rs
git commit -m "feat(m2): GameClock 主循环 + on_actions 钩子分发"
```

---

## Task 8: M2 收尾 — 全量验证 + clippy + 报告

**Files:**
- Create: `docs/milestones/M2-complete.md`

- [ ] **Step 1: 全量测试 + clippy**

Run: `cargo test && cargo clippy --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 全部测试 PASS, clippy 0 警告。若有警告修复

- [ ] **Step 2: 手动验证 P0 修复**

```bash
cargo run --bin hoi4_demo
```
Expected: demo 仍输出正确值(political_power=150 等)

- [ ] **Step 3: 写 M2 完成报告**

`docs/milestones/M2-complete.md`:

```markdown
# M2 完成报告 — 运行时重构 + 主循环

**日期**: <填入>
**结论**: ✅ 3 个 P0 缺陷全部修复,主循环骨架就位

## P0 修复验证
- P0-1 结构化参数: Arg::Block 嵌套递归, 嵌套块参数不再丢数据(有回归测试)
- P0-2 Trigger Registry: Trigger::Check 查表求值, 未注册返回 false
- P0-3 可失败命令: EffectFn 返回 Result, 错误记入 World.error_log

## 主循环
- GameClock::tick 实现 hourly/daily/weekly/monthly 钩子分发
- 测试验证: 24 tick 后 on_daily 触发

## 测试: N passed, 0 failed, 0 clippy warnings

## M3 准备就绪
地基已稳, 可在健壮的 Registry/Arg/主循环上实现战斗引擎。
```

- [ ] **Step 4: 提交 + tag**

```bash
git add docs/milestones/M2-complete.md
git commit -m "docs(m2): M2 完成报告"
git tag m2-complete
```

---

## 自检结果

**Spec 覆盖:**
- ✅ P0-1 结构化参数 → Task 1(Arg)+Task 3(lower)+Task 6(命令)
- ✅ P0-2 Trigger Registry → Task 2(registry)+Task 4(interp Check 查表)
- ✅ P0-3 可失败命令 → Task 1(CmdError)+Task 2(签名)+Task 4(收集)
- ✅ 主循环 → Task 7(clock)
- ✅ 验收标准(§6) 6 项全覆盖

**无占位符:** 所有代码完整, `<填入>` 是运行时日期。

**类型一致:** `EffectFn`/`TriggerFn`/`ParamGet`/`CmdError` 跨 Task 签名一致;
`params: Vec<(String, Arg)>` 在 effect/lower/interp/commands 统一。

**注意:** 这是渐进式重构,Task 1-4 中间状态会编译失败(签名在变),这是预期的。
Task 5-7 完成后恢复全绿。每个 Task 仍单独提交(即使中间编译不过,commit 记录重构过程)。
```
