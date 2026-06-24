# Modifier 层(陆战结算统一修正接口) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 引入 Modifier 层作为陆战结算的统一修正接口, 让后续系统(科技/国策/将领/堑壕/地形/昼夜)通过往 ModifierStack 塞数据影响结算, 不再各自改结算代码。

**Architecture:** 新增 `src/combat/modifier.rs` 模块(Modifier/ModifierStack/CombatContext)。op 由属性名后缀推导(`_factor`=Multiply, 无后缀=Add, 对齐原版)。结算前用 CombatContext 快照汇总国家+省份+师三层 modifier。空栈默认返回 1.0, 现有测试零破坏。

**Tech Stack:** Rust 2021, 纯标准库, 现有 `resolve.rs`/`width.rs`/`recovery.rs` 的结算点。

**关联文档:**
- 设计 spec: `docs/superpowers/specs/2026-06-24-modifier-layer-design.md`
- 设计原则: `docs/design-principles.md`
- 项目现状: `docs/HANDOFF.md`

---

## 文件结构

```
src/
├── combat/
│   ├── modifier.rs      ← 新增: Modifier/ModifierStat/ModifierOp/ModifierStack
│   │                       + parse_modifier_token + CombatContext + terrain_modifiers(占位)
│   ├── mod.rs           ← 改: 声明 modifier 子模块
│   ├── resolve.rs       ← 改: AtkStats::from/pool_value/resolve_hour 接 ctx; resolve_all_battles build ctx
│   ├── width.rs         ← 改: can_join_frontline 宽度上限乘 multiplier
│   ├── recovery.rs      ← 改: org 恢复量乘 multiplier
│   ├── movement.rs      ← 改: 无(can_join_frontline 调用点传空栈, 详见 Task 6)
│   └── commands.rs      ← 改: 注册 add_country_modifier/add_division_modifier
├── runtime/
│   └── entities.rs      ← 改: Division/Country 加 modifiers 字段; effective_* 加 mods 参数
```

### 改动清单

| 文件 | 改动 | Task |
|---|---|---|
| `src/combat/modifier.rs` | 全新模块 | Task 1-2 |
| `src/combat/mod.rs` | 声明子模块 + re-export | Task 1 |
| `src/runtime/entities.rs` | Division/Country 加 modifiers; effective_* 加 mods 参数 | Task 3 |
| `src/combat/resolve.rs` | AtkStats::from/pool_value/resolve_hour 接 ctx; resolve_all_battles build ctx | Task 4 |
| `src/combat/width.rs` | can_join_frontline 宽度乘 multiplier | Task 5 |
| `src/combat/recovery.rs` | org 恢复乘 multiplier | Task 6 |
| `src/combat/commands.rs` | add_country_modifier/add_division_modifier | Task 7 |
| 调用点修复(movement/commands 里调 can_join_frontline 处) | 传空栈 | Task 5 |

### 任务依赖

```
Task 1 (Modifier/Stack/parse) ──┐
Task 2 (CombatContext)         ──┤
Task 3 (entities: 字段+effective_*) ──┤
Task 4 (resolve: 注入 ctx) ──────────┤
Task 5 (width) ──────────────────────┤
Task 6 (recovery) ───────────────────┤
Task 7 (commands) ───────────────────┤
                                      └─ Task 8 (回归+端到端+验收)
```

Task 1-2 独立可并行, Task 3 依赖 1, Task 4 依赖 2+3, 后续按序。

---

## Task 1: Modifier 数据模型 + parse_modifier_token

定义核心数据结构和属性名解析函数。这是整个 modifier 层的基础, 不依赖任何现有代码。

**Files:**
- Create: `src/combat/modifier.rs`
- Modify: `src/combat/mod.rs`

- [ ] **Step 1: 在 combat/mod.rs 声明子模块**

Modify `src/combat/mod.rs` (在 `pub mod commands;` 之后加):

```rust
//! 战斗模块
pub mod commands;
pub mod equipment_data;
pub mod modifier;
pub mod movement;
pub mod pathfinding;
pub mod recovery;
pub mod reinforce;
pub mod resolve;
pub mod width;
```

- [ ] **Step 2: 写 modifier.rs 的数据结构 + multiplier + parse_modifier_token**

Create `src/combat/modifier.rs`:

```rust
//! Modifier 层: 陆战结算的统一修正接口
//!
//! 设计(spec §1-2):
//! - op 由属性名后缀推导(对齐原版 Paradox 约定): 无后缀=Add, _factor=Multiply
//! - 叠加公式: (1+ΣAdd) × Π(1+Multiply)
//! - 空 ModifierStack 的 multiplier 返回 1.0(默认无修正, 精确还原现状)

/// 可被修正的属性(本次覆盖战斗属性+宽度+org恢复)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierStat {
    // 战斗属性(effective_* 的 6 个)
    SoftAttack,
    HardAttack,
    Defense,
    Breakthrough,
    Armor,
    Piercing,
    // 战斗宽度上限
    CombatWidth,
    // 组织度恢复率
    OrgRegain,
}

/// 修正的叠加方式(由属性名后缀推导)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModifierOp {
    /// 无后缀(soft_attack): 加进 add 池, 同类相加
    Add,
    /// _factor 后缀(soft_attack_factor): 独立乘一层
    Multiply,
}

/// 单个 modifier: 作用在某属性上的一个修正
#[derive(Debug, Clone)]
pub struct Modifier {
    pub stat: ModifierStat,
    pub value: f64,           // 0.05 = +5%
    pub op: ModifierOp,       // 由属性名后缀推导, 构造时填好
}

/// 一组 modifier 的集合, 按 stat 查询最终乘数
#[derive(Debug, Clone, Default)]
pub struct ModifierStack {
    mods: Vec<Modifier>,
}

impl ModifierStack {
    pub fn new() -> Self {
        Self { mods: vec![] }
    }

    /// 推入一个 modifier
    pub fn push(&mut self, m: Modifier) {
        self.mods.push(m);
    }

    /// 合并另一个 stack(用于三层汇总: 国家+省份+师)
    pub fn merge(&mut self, other: &ModifierStack) {
        self.mods.extend(other.mods.iter().cloned());
    }

    /// 算某属性的总系数(面板值 × 这个 = 最终值)
    /// 公式: (1 + Σ Add类) × Π(1 + Multiply类)
    /// 空栈返回 1.0(默认无修正)
    pub fn multiplier(&self, stat: ModifierStat) -> f64 {
        let add_sum: f64 = self
            .mods
            .iter()
            .filter(|m| m.stat == stat && m.op == ModifierOp::Add)
            .map(|m| m.value)
            .sum();
        let mult_prod = self
            .mods
            .iter()
            .filter(|m| m.stat == stat && m.op == ModifierOp::Multiply)
            .fold(1.0, |acc, m| acc * (1.0 + m.value));
        (1.0 + add_sum) * mult_prod
    }

    /// 是否为空(无任何 modifier)
    pub fn is_empty(&self) -> bool {
        self.mods.is_empty()
    }
}

/// 字符串属性名 → (stat, op)
/// 规则(对齐原版 Paradox 脚本约定):
///   soft_attack        → (SoftAttack, Add)       无后缀 = 加法
///   soft_attack_factor → (SoftAttack, Multiply)  _factor 后缀 = 乘法
///   defense / defence  → (Defense, Add)          兼容美式/英式
///   armor / armor_value → (Armor, Add)
///   piercing / ap_attack → (Piercing, Add)
/// 未知属性(如 stability_factor) → None(静默跳过)
pub fn parse_modifier_token(s: &str) -> Option<(ModifierStat, ModifierOp)> {
    let (base, op) = if let Some(b) = s.strip_suffix("_factor") {
        (b, ModifierOp::Multiply)
    } else {
        (s, ModifierOp::Add)
    };
    let stat = match base {
        "soft_attack" => ModifierStat::SoftAttack,
        "hard_attack" => ModifierStat::HardAttack,
        "defense" | "defence" => ModifierStat::Defense,
        "breakthrough" => ModifierStat::Breakthrough,
        "armor" | "armor_value" => ModifierStat::Armor,
        "piercing" | "ap_attack" => ModifierStat::Piercing,
        "combat_width" => ModifierStat::CombatWidth,
        "org_regain" | "local_org_regain" => ModifierStat::OrgRegain,
        _ => return None,
    };
    Some((stat, op))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_empty_stack_returns_one() {
        let s = ModifierStack::new();
        assert!((s.multiplier(ModifierStat::SoftAttack) - 1.0).abs() < 1e-9);
        assert!(s.is_empty());
    }

    #[test]
    fn t_pure_add_sums() {
        // +5% + +10% → 1.15
        let mut s = ModifierStack::new();
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: 0.05, op: ModifierOp::Add });
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: 0.10, op: ModifierOp::Add });
        assert!((s.multiplier(ModifierStat::SoftAttack) - 1.15).abs() < 1e-9);
    }

    #[test]
    fn t_pure_multiply_products() {
        // ×5% × ×10% → 1.05 × 1.10 = 1.155
        let mut s = ModifierStack::new();
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: 0.05, op: ModifierOp::Multiply });
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: 0.10, op: ModifierOp::Multiply });
        assert!((s.multiplier(ModifierStat::SoftAttack) - 1.155).abs() < 1e-9);
    }

    #[test]
    fn t_mixed_add_then_multiply() {
        // (1+0.05) × (1+0.10) = 1.155
        let mut s = ModifierStack::new();
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: 0.05, op: ModifierOp::Add });
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: 0.10, op: ModifierOp::Multiply });
        assert!((s.multiplier(ModifierStat::SoftAttack) - 1.155).abs() < 1e-9);
    }

    #[test]
    fn t_negative_multiply_never_negative() {
        // -50% × -30% × -25% → 0.5 × 0.7 × 0.75 = 0.2625 (不负)
        let mut s = ModifierStack::new();
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: -0.50, op: ModifierOp::Multiply });
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: -0.30, op: ModifierOp::Multiply });
        s.push(Modifier { stat: ModifierStat::SoftAttack, value: -0.25, op: ModifierOp::Multiply });
        let m = s.multiplier(ModifierStat::SoftAttack);
        assert!(m > 0.0, "乘法类负修正应保持正数, 实际 {}", m);
        assert!((m - 0.2625).abs() < 1e-9);
    }

    #[test]
    fn t_merge_combines_stacks() {
        let mut a = ModifierStack::new();
        a.push(Modifier { stat: ModifierStat::Defense, value: 0.10, op: ModifierOp::Add });
        let mut b = ModifierStack::new();
        b.push(Modifier { stat: ModifierStat::Defense, value: 0.20, op: ModifierOp::Add });
        a.merge(&b);
        assert!((a.multiplier(ModifierStat::Defense) - 1.30).abs() < 1e-9);
    }

    #[test]
    fn t_parse_no_suffix_is_add() {
        let (stat, op) = parse_modifier_token("soft_attack").unwrap();
        assert_eq!(stat, ModifierStat::SoftAttack);
        assert_eq!(op, ModifierOp::Add);
    }

    #[test]
    fn t_parse_factor_suffix_is_multiply() {
        let (stat, op) = parse_modifier_token("soft_attack_factor").unwrap();
        assert_eq!(stat, ModifierStat::SoftAttack);
        assert_eq!(op, ModifierOp::Multiply);
    }

    #[test]
    fn t_parse_defence_variant() {
        let (stat, _) = parse_modifier_token("defence").unwrap();
        assert_eq!(stat, ModifierStat::Defense);
        let (stat2, _) = parse_modifier_token("defense").unwrap();
        assert_eq!(stat2, ModifierStat::Defense);
    }

    #[test]
    fn t_parse_armor_value() {
        let (stat, _) = parse_modifier_token("armor_value").unwrap();
        assert_eq!(stat, ModifierStat::Armor);
    }

    #[test]
    fn t_parse_unknown_returns_none() {
        assert!(parse_modifier_token("stability_factor").is_none());
        assert!(parse_modifier_token("ace_effectiveness_factor").is_none());
        assert!(parse_modifier_token("political_power").is_none());
    }
}
```

- [ ] **Step 3: 运行测试验证通过**

Run: `cargo test combat::modifier::`
Expected: 10 passed; 0 failed

- [ ] **Step 4: 提交**

```bash
git add src/combat/mod.rs src/combat/modifier.rs
git commit -m "feat(modifier): Modifier/ModifierStack/parse_modifier_token(_factor后缀推导op)"
```

---

## Task 2: CombatContext + terrain_modifiers 占位

定义结算上下文快照和地形 modifier 占位函数。依赖 Task 1 的 ModifierStack。

**Files:**
- Modify: `src/combat/modifier.rs`

- [ ] **Step 1: 在 modifier.rs 追加 CombatContext 和 terrain_modifiers**

在 `src/combat/modifier.rs` 末尾(`#[cfg(test)]` 之前)追加:

```rust
use crate::runtime::{Battle, World};
use std::collections::HashMap;

/// 一场战斗的结算上下文(结算前算好, 结算中只读)
/// 把 国家+省份+师 三层 modifier 汇总到每个参战师, 避免结算时借用冲突。
/// 快照设计支持动态 modifier(昼夜/天气), 详见 spec §3.4。
pub struct CombatContext {
    /// 每个参战师的 modifier 汇总(按 division_id 索引)
    stacks: HashMap<u64, ModifierStack>,
}

impl CombatContext {
    /// 结算前构造: 遍历 battle 攻守双方, 为每个师算 modifier 汇总
    /// = 国家modifier + 该师所在省modifier + 师自身modifier
    pub fn build(world: &World, battle: &Battle) -> CombatContext {
        let mut stacks = HashMap::new();
        for div_id in battle
            .attackers
            .iter()
            .chain(&battle.defenders)
            .chain(&battle.reserve_attackers)
            .chain(&battle.reserve_defenders)
        {
            let Some(d) = world.divisions.get(div_id) else {
                continue;
            };
            let mut stack = ModifierStack::new();
            // 国家层: 科技/精神/ideas
            if let Some(c) = world.countries.get(&d.owner_tag) {
                stack.merge(&c.modifiers);
            }
            // 省份层: 地形(静态查表)
            if let Some(p) = world.provinces.get(&battle.province) {
                stack.merge(&terrain_modifiers(&p.terrain));
                // 后续昼夜: stack.merge(&night_modifier(world.darkness[battle.province]))
            }
            // 师自身: 堑壕/计划/经验
            stack.merge(&d.modifiers);
            stacks.insert(*div_id, stack);
        }
        CombatContext { stacks }
    }

    /// 取某师的 modifier 汇总(找不到则返回静态空栈引用, 不 panic)
    pub fn get(&self, div_id: u64) -> &ModifierStack {
        self.stacks.get(&div_id).unwrap_or_else(|| ModifierStack::empty_static())
    }

    /// 构造一个空上下文(无任何 modifier, 用于不关心 modifier 的调用点/测试)
    pub fn empty() -> CombatContext {
        CombatContext { stacks: HashMap::new() }
    }
}

impl ModifierStack {
    /// 返回一个静态空栈引用(CombatContext::get 兜底用)
    /// OnceLock 保证只初始化一次, 零外部依赖
    pub fn empty_static() -> &'static ModifierStack {
        use std::sync::OnceLock;
        static EMPTY: OnceLock<ModifierStack> = OnceLock::new();
        EMPTY.get_or_init(ModifierStack::new)
    }
}

/// 地形 modifier 查表(占位: 本次返回空栈, 无地形数据)
/// 后续地形系统实现时, 按 terrain 名返回真实修正(森林 attack -0.15 等)
/// 夜间修正(night modifier × darkness)后续也走这里, 详见 spec §3.4。
pub fn terrain_modifiers(_terrain: &str) -> ModifierStack {
    ModifierStack::new()
}
```

- [ ] **Step 2: 追加测试到 modifier.rs 的 tests 模块**

在 `src/combat/modifier.rs` 的 `mod tests` 内追加:

```rust
    use crate::runtime::{Battle, World};

    #[test]
    fn t_empty_context_get_returns_empty_stack() {
        let ctx = CombatContext::empty();
        let m = ctx.get(999).multiplier(ModifierStat::SoftAttack);
        assert!((m - 1.0).abs() < 1e-9, "空 ctx 查任意师应返回 1.0");
    }

    #[test]
    fn t_build_aggregates_country_and_division_modifiers() {
        // 国家 GER 有 +10% soft(add), 师有 -15% soft(multiply)
        // 最终 = (1+0.10) × (1-0.15) = 1.10 × 0.85 = 0.935
        let mut w = World::new();
        let mut country = crate::runtime::Country::default();
        country.modifiers.push(Modifier {
            stat: ModifierStat::SoftAttack, value: 0.10, op: ModifierOp::Add,
        });
        w.countries.insert("GER".into(), country);

        let mut div = crate::runtime::Division::default();
        div.owner_tag = "GER".into();
        div.modifiers.push(Modifier {
            stat: ModifierStat::SoftAttack, value: -0.15, op: ModifierOp::Multiply,
        });
        let div_id = w.add_division(div);

        w.provinces.insert(1, crate::runtime::Province {
            id: 1, owner: "GER".into(), controller: "GER".into(),
            terrain: "plains".into(), neighbors: vec![],
        });

        let battle = Battle {
            id: 0, province: 1,
            attackers: vec![div_id], defenders: vec![],
            reserve_attackers: vec![], reserve_defenders: vec![],
        };
        let ctx = CombatContext::build(&w, &battle);
        let m = ctx.get(div_id).multiplier(ModifierStat::SoftAttack);
        assert!((m - 0.935).abs() < 1e-9, "国家+师 modifier 汇总应 0.935, 实际 {}", m);
    }

    #[test]
    fn t_build_skips_missing_division() {
        // battle 引用了不存在的师 id, build 不应 panic
        let w = World::new();
        let battle = Battle {
            id: 0, province: 1,
            attackers: vec![999], defenders: vec![],
            reserve_attackers: vec![], reserve_defenders: vec![],
        };
        let ctx = CombatContext::build(&w, &battle);
        assert!(ctx.get(999).is_empty());
    }
```

> 依赖: 测试用 `Division::default()` 和 `Country::default()`(需 derive Default)。Division/Country 在 Task 3 加 modifiers 字段后, 若 derive 了 Default 则空栈自动初始化。**若 Division 未 derive Default**, Task 3 里补 `#[derive(Debug, Clone, Default)]`(已存在则无需改)。

- [ ] **Step 3: 运行测试验证通过**

Run: `cargo test combat::modifier::`
Expected: 13 passed (10 old + 3 new); 0 failed

> Task 2 的测试依赖 Task 3 已加 modifiers 字段。执行顺序: Task 1 → Task 3 → Task 2, 或 Task 1→2→3 但 Task 2 的测试延迟到 Task 3 后跑。

- [ ] **Step 4: 提交**

```bash
git add src/combat/modifier.rs
git commit -m "feat(modifier): CombatContext 快照(国家+省份+师三层汇总) + terrain_modifiers 占位"
```

---

## Task 3: Division/Country 加 modifiers 字段 + effective_* 加参数

给实体加 modifier 存储字段, 改造 effective_* 方法接 ModifierStack 参数。

**Files:**
- Modify: `src/runtime/entities.rs`

- [ ] **Step 1: Division 加 modifiers 字段**

在 `src/runtime/entities.rs` 的 `Division` 结构体, 在 `pub order: OrderState,` 之后加:

```rust
    /// modifier 汇总(堑壕/计划/经验等师自身修正)
    pub modifiers: crate::combat::modifier::ModifierStack,
```

同时确认 `Division` 的 `#[derive(Default)]` 仍工作(ModifierStack 已 derive Default)。

- [ ] **Step 2: Country 加 modifiers 字段**

在 `src/runtime/entities.rs` 的 `Country` 结构体, 在 `pub manpower_pool: f64,` 之后加:

```rust
    /// modifier 汇总(科技/精神/ideas 等国家级修正)
    pub modifiers: crate::combat::modifier::ModifierStack,
```

- [ ] **Step 3: effective_* 方法加 mods 参数**

把 `src/runtime/entities.rs` 的 4 个 effective_* 方法改成接 `&ModifierStack`:

```rust
    // 有效属性 = 面板值 × 综合补给充足度 × modifier
    pub fn effective_soft_attack(&self, mods: &crate::combat::modifier::ModifierStack) -> f64 {
        self.soft_attack * self.supply_ratio()
            * mods.multiplier(crate::combat::modifier::ModifierStat::SoftAttack)
    }
    pub fn effective_hard_attack(&self, mods: &crate::combat::modifier::ModifierStack) -> f64 {
        self.hard_attack * self.supply_ratio()
            * mods.multiplier(crate::combat::modifier::ModifierStat::HardAttack)
    }
    pub fn effective_defense(&self, mods: &crate::combat::modifier::ModifierStack) -> f64 {
        self.defense * self.equipment_ratio()
            * mods.multiplier(crate::combat::modifier::ModifierStat::Defense)
    }
    pub fn effective_breakthrough(&self, mods: &crate::combat::modifier::ModifierStack) -> f64 {
        self.breakthrough * self.equipment_ratio()
            * mods.multiplier(crate::combat::modifier::ModifierStat::Breakthrough)
    }
```

注意: 把原方法签名 `(self) -> f64` 改成 `(self, mods: &ModifierStack) -> f64`。

- [ ] **Step 4: 编译会失败(resolve.rs 等调用点未更新) — 这是预期的**

Run: `cargo build 2>&1 | grep "error\[" | head`
Expected: 多个 `error[E0061]: this function takes 2 arguments but 1 was supplied`(effective_* 调用点未传 mods)

这是预期的 — 调用点在 Task 4 修复。**不要在 Task 3 修 resolve.rs**。

- [ ] **Step 5: 确认 Division/Country 的 Default 仍工作**

Division 和 Country 都已 `#[derive(Debug, Clone, Default)]`(Task 前确认过)。加了 `modifiers: ModifierStack` 字段后, 由于 ModifierStack 也 derive 了 Default(Task 1), `Division::default()` / `Country::default()` 自动工作, 无需手写。

验证: `cargo build` 应无 Default 相关错误(此时会有 effective_* 调用点错误, 那是 Task 4 修的, 与 Default 无关)。

- [ ] **Step 6: 提交(编译未全绿是预期, Task 4 修复调用点)**

```bash
git add src/runtime/entities.rs
git commit -m "feat(modifier): Division/Country 加 modifiers 字段 + effective_* 接 mods 参数(调用点待 Task4 修)"
```

> 注: 此 commit 后 cargo build 会有错误(effective_* 调用点), Task 4 修复后恢复全绿。若想保持每个 commit 可编译, 可把 Task 3+4 合并执行。

---

## Task 4: resolve.rs 注入 CombatContext

修复所有 effective_* 调用点, 在 resolve_all_battles 构造 CombatContext。

**Files:**
- Modify: `src/combat/resolve.rs`

- [ ] **Step 1: AtkStats::from 和 pool_value 接 mods**

在 `src/combat/resolve.rs`:

把 `AtkStats::from` 改成接 `&ModifierStack`:

```rust
impl AtkStats {
    fn from(d: &Division, mods: &crate::combat::modifier::ModifierStack) -> Self {
        use crate::combat::modifier::ModifierStat;
        Self {
            soft_attack: d.effective_soft_attack(mods),
            hard_attack: d.effective_hard_attack(mods),
            armor: d.armor * mods.multiplier(ModifierStat::Armor),
            piercing: d.piercing * mods.multiplier(ModifierStat::Piercing),
        }
    }
}
```

把 `CombatPool::pool_value` 改成接 `&ModifierStack`:

```rust
impl CombatPool {
    fn pool_value(self, d: &Division, mods: &crate::combat::modifier::ModifierStack) -> f64 {
        match self {
            CombatPool::Defense => d.effective_defense(mods),
            CombatPool::Breakthrough => d.effective_breakthrough(mods),
        }
    }
}
```

- [ ] **Step 2: apply_all_attackers 接 mods 并透传**

`apply_all_attackers` 需要把 mods 传给 pool_value(每个 target 一个 mods)。但 mods 是按 division_id 的, 而 apply_all_attackers 处理一组 targets。改签名: 接收 `targets_mods: &[&ModifierStack]`(与 targets 一一对应):

```rust
fn apply_all_attackers(
    attackers: &[AtkStats],
    targets: &mut [&mut Division],
    pool: CombatPool,
    targets_mods: &[&crate::combat::modifier::ModifierStack],
) {
    let n = targets.len();
    if n == 0 || attackers.is_empty() || targets_mods.len() != n {
        return;
    }
    let target_hardness = targets[0].hardness;

    for (i, tgt) in targets.iter_mut().enumerate() {
        let base = (1.0 - DAMAGE_SPLIT_FIRST) / n as f64;
        let share = if i == 0 { DAMAGE_SPLIT_FIRST + base } else { base };
        let mut total_attacks = 0.0f64;
        let mut per_atk: Vec<(f64, bool, bool)> = Vec::new();
        for atk in attackers {
            let atk_pts = atk.soft_attack * (1.0 - target_hardness) + atk.hard_attack * target_hardness;
            let on_this = atk_pts * share;
            if on_this <= 0.0 { continue; }
            total_attacks += on_this;
            per_atk.push((on_this, atk.armor > tgt.piercing, tgt.armor > atk.piercing));
        }
        if total_attacks <= 0.0 { continue; }
        // P1-5: 用目标防御池判定(传入该 target 的 mods)
        let total_hits = compute_hits(total_attacks, pool.pool_value(tgt, targets_mods[i]));
        for (atk_pts, armor_outclass, def_outclass) in per_atk {
            let hits = total_hits * (atk_pts / total_attacks);
            let mut org_dice = ORG_DICE_SIZE;
            let mut str_dice = STR_DICE_SIZE;
            if armor_outclass { org_dice += ARMOR_ORG_BONUS_DICE; str_dice += ARMOR_STR_BONUS_DICE; }
            let armor_deflect = if def_outclass { 0.5 } else { 1.0 };
            let org_dmg = hits * ((org_dice + 1.0) / 2.0) * ORG_DMG_MOD * armor_deflect;
            let str_dmg = hits * ((str_dice + 1.0) / 2.0) * STR_DMG_MOD * armor_deflect;
            tgt.org = (tgt.org - org_dmg).max(0.0);
            let hp_before = tgt.strength;
            tgt.strength = (tgt.strength - str_dmg).max(0.0);
            let hp_loss = hp_before - tgt.strength;
            if hp_loss > 0.0 { consume_losses(tgt, hp_loss); }
        }
    }
}
```

- [ ] **Step 3: resolve_hour 改签名接 ctx**

把 `resolve_hour` 改成接 `&CombatContext`:

```rust
/// 对一组攻击者 vs 一组防御者结算 1 小时
/// mods 从 ctx 按 division_id 取
pub fn resolve_hour(
    attackers: &[Division],
    defenders: &mut [&mut Division],
    ctx: &crate::combat::modifier::CombatContext,
) {
    if attackers.is_empty() || defenders.is_empty() { return; }
    let atk_stats: Vec<AtkStats> = attackers.iter()
        .map(|d| AtkStats::from(d, ctx.get(d.id))).collect();
    let def_mods: Vec<&crate::combat::modifier::ModifierStack> =
        defenders.iter().map(|d| ctx.get(d.id)).collect();
    let mut def_refs: Vec<&mut Division> = defenders.iter_mut().collect();
    apply_all_attackers(&atk_stats, &mut def_refs, CombatPool::Defense, &def_mods);
}
```

> 注意: resolve_hour 原本只做"正向(攻→守)"。反向(守→攻)在 resolve_all_battles 里对称调用 apply_all_attackers。resolve_hour 保持只做正向, 但需要 ctx。

- [ ] **Step 4: resolve_all_battles 构造 ctx 并传给 apply_all_attackers**

在 `resolve_all_battles` 的战斗循环里, build ctx 并改两个 apply_all_attackers 调用:

找到 `for (atk_ids, def_ids) in &battle_specs {` 循环, 在 clone atks/defs 之后加 ctx 构建:

```rust
    for (atk_ids, def_ids) in &battle_specs {
        // ... 现有 atk_before/def_before/atks/defs clone 逻辑不变 ...

        if atks.is_empty() || defs.is_empty() { continue; }

        // 构造 CombatContext(需要找到 battle 实例拿 province)
        let battle_idx = world.battles.iter().position(|b| b.attackers == *atk_ids && b.defenders == *def_ids);
        let ctx = if let Some(bi) = battle_idx {
            crate::combat::modifier::CombatContext::build(world, &world.battles[bi])
        } else {
            crate::combat::modifier::CombatContext::empty()
        };

        // 正向: 攻 → 守
        {
            let atk_stats: Vec<AtkStats> = atks.iter().map(|d| AtkStats::from(d, ctx.get(d.id))).collect();
            let def_mods: Vec<&crate::combat::modifier::ModifierStack> =
                defs.iter().map(|d| ctx.get(d.id)).collect();
            let mut def_refs: Vec<&mut Division> = defs.iter_mut().collect();
            apply_all_attackers(&atk_stats, &mut def_refs, CombatPool::Defense, &def_mods);
        }
        // 反向: 守 → 攻
        {
            let def_stats: Vec<AtkStats> = defs.iter().map(|d| AtkStats::from(d, ctx.get(d.id))).collect();
            let atk_mods: Vec<&crate::combat::modifier::ModifierStack> =
                atks.iter().map(|d| ctx.get(d.id)).collect();
            let mut atk_refs: Vec<&mut Division> = atks.iter_mut().collect();
            apply_all_attackers(&def_stats, &mut atk_refs, CombatPool::Breakthrough, &atk_mods);
        }
        // ... 现有 delta 累积逻辑不变 ...
```

- [ ] **Step 5: 修复 resolve.rs 内联测试的 resolve_hour 调用**

resolve.rs 的 tests 模块里有多处 `resolve_hour(&atks, &mut defs)` 调用。全部加 ctx 参数。空 ctx 用 `CombatContext::empty()`:

例如:
```rust
// 改前:
resolve_hour(&atks, &mut defs);
// 改后:
let ctx = crate::combat::modifier::CombatContext::empty();
resolve_hour(&atks, &mut defs, &ctx);
```

搜索所有 `resolve_hour(` 调用, 逐个加 `&ctx` 参数。用 `grep -n "resolve_hour(" src/combat/resolve.rs` 找全。

- [ ] **Step 6: 运行全量测试验证现有测试零回归**

Run: `cargo test`
Expected: 全部通过。空 ctx 的 multiplier 返回 1.0, effective_* 数值与改造前逐位相同。

若有测试失败, 检查是不是漏改了某个 resolve_hour 调用点(没传 ctx)。

- [ ] **Step 7: 提交**

```bash
git add src/combat/resolve.rs
git commit -m "feat(modifier): resolve 注入 CombatContext(空栈默认值, 现有测试零回归)"
```

---

## Task 5: width.rs 宽度上限乘 modifier

让战斗宽度上限可通过 modifier 改变。

**Files:**
- Modify: `src/combat/width.rs`
- Modify: `src/combat/commands.rs`(调用点传空栈)
- Modify: `src/combat/resolve.rs`(resolve_all_battles 里的 start_battle 宽度分配, 若有)

- [ ] **Step 1: can_join_frontline 加 mods 参数**

在 `src/combat/width.rs`:

```rust
use crate::combat::modifier::{ModifierStack, ModifierStat};

/// 判断新师能否加入前线(加入后宽度是否 <= 上限)
/// 上限 = BASE_COMBAT_WIDTH × mods.multiplier(CombatWidth)
pub fn can_join_frontline(
    world: &World,
    frontline: &[u64],
    new_div_width: f64,
    mods: &ModifierStack,
) -> bool {
    let used = world.used_width(frontline);
    let cap = BASE_COMBAT_WIDTH * mods.multiplier(ModifierStat::CombatWidth);
    used + new_div_width <= cap
}
```

- [ ] **Step 2: 修复所有 can_join_frontline 调用点**

搜索调用点: `grep -rn "can_join_frontline" src/`

每个调用点加空栈参数。例如 `src/combat/commands.rs`:

```rust
// 改前:
if crate::combat::width::can_join_frontline(world, &frontline_d, w_div) {
// 改后:
let empty = crate::combat::modifier::ModifierStack::new();
if crate::combat::width::can_join_frontline(world, &frontline_d, w_div, &empty) {
```

commands.rs 里有多处(jon_as_attacker / start_battle), 都加空栈。用 `grep -n "can_join_frontline" src/combat/commands.rs` 找全。

- [ ] **Step 3: 修复 width.rs 内联测试**

width.rs 的 tests 里调 can_join_frontline 的地方加空栈:

```rust
let empty = ModifierStack::new();
assert!(can_join_frontline(&w, &frontline, 20.0, &empty));
```

- [ ] **Step 4: 运行测试验证**

Run: `cargo test`
Expected: 全绿(空栈 multiplier=1.0, 宽度上限仍 70)。

- [ ] **Step 5: 提交**

```bash
git add src/combat/width.rs src/combat/commands.rs
git commit -m "feat(modifier): 战斗宽度上限乘 multiplier(空栈默认70, 现状不变)"
```

---

## Task 6: recovery.rs org 恢复乘 modifier

让组织度恢复率可通过 modifier 改变。

**Files:**
- Modify: `src/combat/recovery.rs`

- [ ] **Step 1: recover_org 内的恢复量乘 OrgRegain**

在 `src/combat/recovery.rs`, 找到算 recovery 的地方(spec §4.3):

```rust
// 改前:
let recovery = hourly * (0.5 + 0.5 * div.supply_ratio());

// 改后: 乘 org_regain modifier
let org_mult = div.modifiers.multiplier(crate::combat::modifier::ModifierStat::OrgRegain);
let recovery = hourly * (0.5 + 0.5 * div.supply_ratio()) * org_mult;
```

注意: recovery 读的是 `div.modifiers`(师自身), 不是 CombatContext(因为 recover_org 在主循环里跑, 不在战斗结算内)。这是合理的——org 恢复是师的状态变化, 师自身的 modifier(经验/精神)直接读。

- [ ] **Step 2: 检查 HOURLY_ORG_MOVEMENT_IMPACT 是否也要乘**

行军 org 损失(`HOURLY_ORG_MOVEMENT_IMPACT = -0.2`)目前不乘 modifier。原版这个受 `org_loss_when_moving_factor` 影响。本次先不改(保持现状), 后续补给系统时再加。

- [ ] **Step 3: 运行测试验证现有 recovery 测试零回归**

Run: `cargo test combat::recovery::`
Expected: 全绿(空 modifiers 栈, OrgRegain multiplier=1.0, 恢复量不变)。

- [ ] **Step 4: 提交**

```bash
git add src/combat/recovery.rs
git commit -m "feat(modifier): org 恢复量乘 OrgRegain multiplier(空栈默认不变)"
```

---

## Task 7: add_country_modifier / add_division_modifier 命令

注册运行时动态加 modifier 的命令。

**Files:**
- Modify: `src/combat/commands.rs`

- [ ] **Step 1: 加 add_country_modifier 命令**

在 `src/combat/commands.rs` 的 `register` 函数内追加:

```rust
    // 加国家级 modifier(科技/国策/精神触发)
    // stat 用原版属性名(带或不带 _factor), op 由后缀推导
    reg.register("add_country_modifier", |w, p| {
        let tag = np(p, "add_country_modifier", "tag")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("tag 应为字符串".into()))?;
        let token = np(p, "add_country_modifier", "stat")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("stat 应为字符串".into()))?;
        let value = num_of(np(p, "add_country_modifier", "value")?)?;
        let (stat, op) = crate::combat::modifier::parse_modifier_token(token)
            .ok_or_else(|| CmdError::RuntimeError(format!("未知属性: {token}")))?;
        let country = w.countries.entry(tag.into()).or_default();
        country.modifiers.push(crate::combat::modifier::Modifier { stat, value, op });
        Ok(())
    });
```

- [ ] **Step 2: 加 add_division_modifier 命令**

紧接 add_country_modifier 之后追加:

```rust
    // 加师级 modifier(堑壕/计划/经验)
    reg.register("add_division_modifier", |w, p| {
        let div_id = num_of(np(p, "add_division_modifier", "division")?)? as u64;
        let token = np(p, "add_division_modifier", "stat")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("stat 应为字符串".into()))?;
        let value = num_of(np(p, "add_division_modifier", "value")?)?;
        let (stat, op) = crate::combat::modifier::parse_modifier_token(token)
            .ok_or_else(|| CmdError::RuntimeError(format!("未知属性: {token}")))?;
        let Some(d) = w.divisions.get_mut(&div_id) else {
            return Err(CmdError::RuntimeError(format!("师 {div_id} 不存在")));
        };
        d.modifiers.push(crate::combat::modifier::Modifier { stat, value, op });
        Ok(())
    });
```

- [ ] **Step 3: 运行编译验证**

Run: `cargo build`
Expected: 成功。

- [ ] **Step 4: 提交**

```bash
git add src/combat/commands.rs
git commit -m "feat(modifier): add_country_modifier/add_division_modifier 命令(stat后缀推导op)"
```

---

## Task 8: 端到端测试 + 全量回归 + 验收

验证 modifier 层端到端工作, 确认验收标准。

**Files:**
- Modify: `tests/integration.rs`

- [ ] **Step 1: 端到端测试 — add_country_modifier 影响战斗**

在 `tests/integration.rs` 追加:

```rust
#[test]
fn t_country_modifier_affects_combat() {
    // GER 加 +100% soft_attack(Add), 战斗伤害应显著提升
    use hoi4_clone::runtime::{World, Interpreter, Registry};
    use hoi4_clone::commands::register_all;
    use hoi4_clone::ast::lower::lower_effects;
    use hoi4_clone::parser::parse;

    let mut w = World::new();
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);

    // 先加 modifier
    let setup = r#"
        add_country_modifier = { tag = GER stat = soft_attack value = 1.0 }
    "#;
    interp.run(&lower_effects(&parse(setup).unwrap()), &mut w);

    // 建 GER 师和 FRA 师, 开战
    let battle_setup = r#"
        create_division = { owner = GER location = 1 soft_attack = 30 hard_attack = 0 defense = 10 max_strength = 100 }
        create_division = { owner = FRA location = 2 soft_attack = 0 hard_attack = 0 defense = 10 max_strength = 100 }
        create_province = { id = 1 owner = GER neighbors = { 2 } }
        create_province = { id = 2 owner = FRA neighbors = { 1 } }
        start_battle = { attacker = GER defender = FRA province = 2 }
    "#;
    interp.run(&lower_effects(&parse(battle_setup).unwrap()), &mut w);

    // 记录 FRA 师 HP, 结算 1 小时
    let fra_id = w.divisions_of("FRA")[0];
    let hp_before = w.divisions.get(&fra_id).unwrap().strength;

    use hoi4_clone::runtime::GameClock;
    GameClock::advance(&interp, &mut w, 1);

    let hp_after = w.divisions.get(&fra_id).unwrap().strength;
    let loss = hp_before - hp_after;
    // +100% soft_attack → 攻击翻倍 → 伤害应明显(>0)
    assert!(loss > 0.0, "modifier 生效后应有伤害, 实际 loss={loss}");

    // 对照: 无 modifier 时同样配置的伤害(应小于有 modifier)
    let mut w2 = World::new();
    let mut reg2 = Registry::new();
    register_all(&mut reg2);
    let interp2 = Interpreter::new(reg2);
    interp2.run(&lower_effects(&parse(battle_setup).unwrap()), &mut w2);
    let fra_id2 = w2.divisions_of("FRA")[0];
    let hp_before2 = w2.divisions.get(&fra_id2).unwrap().strength;
    GameClock::advance(&interp2, &mut w2, 1);
    let loss2 = hp_before2 - w2.divisions.get(&fra_id2).unwrap().strength;

    assert!(loss > loss2, "有 +100% modifier 的伤害应大于无 modifier: {loss} > {loss2}");
}
```

- [ ] **Step 2: 端到端测试 — _factor 后缀走 Multiply**

在 `tests/integration.rs` 追加:

```rust
#[test]
fn t_factor_suffix_parses_as_multiply() {
    // soft_attack_factor 应解析为 Multiply(独立乘), soft_attack 应解析为 Add
    use hoi4_clone::combat::modifier::{parse_modifier_token, ModifierOp};

    let (_, op1) = parse_modifier_token("soft_attack").unwrap();
    let (_, op2) = parse_modifier_token("soft_attack_factor").unwrap();
    assert_eq!(op1, ModifierOp::Add);
    assert_eq!(op2, ModifierOp::Multiply);
}
```

- [ ] **Step 3: 端到端测试 — 空栈精确还原现状**

在 `tests/integration.rs` 追加(验证验收标准 §10.2):

```rust
#[test]
fn t_empty_modifiers_exact_same_as_before() {
    // 空 ModifierStack 时 effective_soft_attack 应等于 面板×补给(无 modifier)
    use hoi4_clone::runtime::Division;
    use hoi4_clone::combat::modifier::ModifierStack;

    let mut d = Division::default();
    d.soft_attack = 30.0;
    d.equipment_held.insert("x".into(), 100.0);
    d.equipment_need.insert("x".into(), 100.0);
    d.manpower_held = 1000.0;
    d.manpower_need = 1000.0;

    let empty = ModifierStack::new();
    let with_mods = d.effective_soft_attack(&empty);
    // 满编时 supply_ratio=1.0, 空 modifier multiplier=1.0 → 30.0
    assert!((with_mods - 30.0).abs() < 1e-9, "空栈应精确还原: 30×1.0×1.0=30, 实际 {}", with_mods);
}
```

> 注意: 此测试依赖 Division::default()。若 Division 没有 derive Default, 用 Task 3 的 default_for_test() 或手写完整字段。

- [ ] **Step 4: 全量回归**

Run: `cargo test`
Expected: 全部通过(现有 147 + 新增端到端 + modifier 单测)。

记录测试数:
```bash
cargo test 2>&1 | grep "test result" | grep -v "0 passed; 0 failed"
```

- [ ] **Step 5: 验收对照(逐条确认 spec §10)**

逐条核对:
1. cargo test 全绿 ✓
2. 空 ModifierStack 数值逐位相同(Step 3 测试)✓
3. add_country_modifier +50% 提升攻击(Step 1 测试)✓
4. _factor 解析 Multiply, 无后缀 Add(Step 2 测试 + Task 1 单测)✓
5. 宽度可改(Task 5, 空栈默认 70)✓
6. 后续系统不改 resolve/effective/width/recovery(spec §9 已列接入路径)✓

- [ ] **Step 6: 更新 HANDOFF.md**

在 `docs/HANDOFF.md` 里程碑表追加 modifier 层, 在代码结构加 `combat/modifier.rs`。

- [ ] **Step 7: 提交**

```bash
git add tests/integration.rs docs/HANDOFF.md
git commit -m "feat(modifier): 端到端测试 + 验收(country_modifier影响战斗/_factor推导/空栈还原现状)"
```

---

## 实现顺序提示

严格 Task 1→8。**关键依赖与注意点**:

- **Task 2 与 Task 3 的顺序**: CombatContext 的测试用了 `Division.modifiers`(Task 3 才加)。建议 Task 1 → Task 3(加字段, 暂不跑 CombatContext 测试) → Task 2(CombatContext 代码 + 测试, 此时字段已就位)。或保持 1→2→3 但 Task 2 的测试延迟到 Task 3 后跑。
- **Task 3 后 cargo build 会失败**: effective_* 调用点(resolve.rs)未更新。这是预期, Task 4 修复。若想每个 commit 可编译, 合并 Task 3+4 为一次。
- **Task 4 是回归风险集中点**: 改 resolve_hour 签名影响所有 resolve 测试。空 ctx 必须精确还原现状(multiplier=1.0)。
- **Task 5 的 can_join_frontline 调用点散落**: commands.rs 里 join_as_attacker/start_battle 都调, 要全改(grep 找全)。
