# M2 — 运行时基础设施重构 + 主循环

> **创建日期**: 2026-06-20
> **状态**: 设计已定稿,待实现
> **依据**: `docs/specs/2026-06-20-architecture-design.md` §4.2 + M1 code review 的 P0 反馈
> **前置**: M1 (`m1-complete` tag)

---

## 0. 目标

M1 验证了"脚本可被解析执行",但 reviewer 指出当前 Registry/Arg 设计有 3 个 P0 缺陷,
**在它们之上堆命令会反复撞墙**。M2 不碰战斗/生产公式,只做两件事:

1. **P0 基础设施重构**(3 项)—— 让命令系统足够健壮,支撑 M3+ 的 ~80 个战斗命令
2. **主循环骨架** —— 实现 spec §4.2.1 的 hourly tick + on_actions 钩子分发

M2 完成后,M3 可以放心地在健壮的地基上实现战斗引擎。

---

## 1. P0-1: 结构化命令参数(替换 Arg::Str("k=v") hack)

### 问题(M1 review)
`lower.rs` 把 `add_to_variable = { x = 0.05 }` 降级成 `Arg::Str("x=0.05")`,扁平化丢类型;
遇到嵌套块参数(`add_equipment_production = { equipment = {...} count = 10 }`)直接丢数据且无报错。

### 方案
`Effect::Command` 携带结构化参数,而非扁平字符串:

```rust
// 改前
enum Arg { Num(f64), Str(String), Bool(bool) }
Effect::Command { name, args: Vec<Arg> }

// 改后
enum Arg {
    Num(f64),
    Str(String),
    Bool(bool),
    Block(Vec<(String, Arg)>),  // 有序键值对,支持嵌套
}
Effect::Command { name, params: Vec<(String, Arg)> }  // 命令参数是命名字段
```

这样 `add_to_variable = { x = 0.05 }` → `params: [("x", Num(0.05))]`,
嵌套块自然递归。命令 handler 按 key 取参,类型明确。

### 兼容性
M1 的 7 个变量命令从 `args: &[Arg]`(取 first) 改成 `params: &[(String,Arg)]`(按 key 取)。
所有 M1 测试同步更新。

---

## 2. P0-2: Trigger Registry

### 问题
`Registry` 只有 `effects` 表;`Trigger::Check` 在 interp 恒返回 true。
M2 战斗命令里大量 trigger(`is_in_combat`, `has_equipment` 等)无处注册。

### 方案
`Registry` 增加 `triggers` 表,Interpreter 求值 `Trigger::Check` 时查表分发:

```rust
type TriggerFn = fn(&World, &[(String, Arg)]) -> Result<bool, CmdError>;

pub struct Registry {
    pub effects: HashMap<String, EffectFn>,
    pub triggers: HashMap<String, TriggerFn>,  // 新增
}
```

`Trigger::Check { name, args }` 求值:查 triggers 表,未注册则返回 `false`(保守,不执行)。
比 M1 的"恒 true"更安全——M3 接入真实判定时逐步注册。

---

## 3. P0-3: 可失败命令签名

### 问题
`EffectFn = fn(&mut World, &[Arg]) -> ()`,不能失败。坏参数静默忽略。

### 方案(用户选定: Result 显式错误)

```rust
#[derive(Debug)]
pub enum CmdError {
    UnknownCommand(String),
    BadParam { cmd: String, key: String, reason: String },
    RuntimeError(String),
}

type EffectFn = fn(&mut World, &[(String, Arg)]) -> Result<(), CmdError>;
```

Interpreter `run_one` 收集错误(不中止整个执行),记录到 `World.error_log`,
让 M3 转译脚本能审计"哪些命令失败"。

---

## 4. 主循环骨架(spec §4.2.1)

### 方案
实现 hourly tick + on_actions 钩子分发。M2 只搭骨架(空钩子),M3 接入战斗/生产。

```rust
pub struct GameClock { pub hour: u64 }

impl World {
    pub fn tick(&mut self, interp: &Interpreter) {
        self.hour += 1;
        interp.fire_event("on_hourly", self);
        // M3: combat::resolve, production::produce, movement::update
        if self.hour % 24 == 0 {
            interp.fire_event("on_daily", self);
            interp.fire_event(&format!("on_daily_{}", self.player_tag), self);
        }
        if self.hour % (24*7) == 0 { interp.fire_event("on_weekly", self); }
    }
}
```

EventBus 用 `HashMap<String, Vec<Effect>>`(钩子名 → 注册的 effect 块)。
M2 验证:注册一个 `on_daily` 钩子,tick 24 次后它被执行。

---

## 5. 范围边界(YAGNI)

**M2 做**:
- Arg/Effect/Registry/Interpreter 的签名重构(P0-1/2/3)
- 所有 M1 命令迁移到新签名
- GameClock + EventBus + tick 骨架
- 更新全部 M1 测试到新 API

**M2 不做**(留 M3+):
- 战斗/生产/科技/移动的实际逻辑(钩子是空的)
- on_actions 脚本文件的解析加载(M3 内容转译)
- ECS 完整实体存储(World 仍是 vars/flags,实体在 M3)
- 存档序列化

---

## 6. 验收标准

- [ ] 所有 M1 测试迁移到新 API 后仍通过(20 → 约同等数量)
- [ ] 新增测试:嵌套块参数不丢数据(P0-1)
- [ ] 新增测试:Trigger.Check 查表(P0-2)
- [ ] 新增测试:命令返回 CmdError 被 World 记录(P0-3)
- [ ] 新增测试:tick 24 次 on_daily 钩子触发(主循环)
- [ ] clippy 0 警告
- [ ] demo 仍可跑

---

## 7. 下一步

spec 审阅通过后 → writing-plans 出 M2 实现计划。
