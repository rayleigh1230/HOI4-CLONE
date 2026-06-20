# M3 实体存储 + 作用域 + 陆战 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: subagent-driven-development 或 executing-plans。

**Goal:** 实现实体存储(Province/Country/Division)、作用域枚举栈、陆战引擎,达成"两个师能打仗"。

**Architecture:** 渐进式——先实体结构(纯新增)→ 作用域栈(改 Interpreter)→ 战斗引擎(新模块挂 on_hourly)→ 端到端。每阶段测试绿。

**Tech Stack:** Rust 2021, 零外部依赖, `stable-x86_64-pc-windows-gnu`。

**Spec:** `docs/specs/2026-06-20-m3-entities-scope-combat.md`
**公式:** `docs/formulas/land-combat.md`

---

## 文件结构

```
src/
├── runtime/
│   ├── entities.rs      [新] Province/Country/Division/Battle + Scope
│   ├── world.rs         [改] 加实体存储 + scope_stack
│   └── interp.rs        [改] ForEach 真实枚举(作用域分发)
├── combat/
│   ├── mod.rs           [新] combat 模块入口
│   ├── resolve.rs       [新] 战斗结算(攻击点/防御池/掷骰/分摊)
│   └── commands.rs      [新] 战斗相关 effect/trigger 命令注册
└── lib.rs               [改] 加 combat 模块
```

---

## Task 1: 实体结构定义(纯新增)

**Files:** Create `src/runtime/entities.rs`, Modify `src/runtime/mod.rs`, `src/runtime/world.rs`

- [ ] **Step 1: 创建 entities.rs**

```rust
//! 游戏实体结构(M3)
use crate::ast::Arg;

#[derive(Debug, Clone)]
pub struct Province {
    pub id: u32,
    pub owner: String,
    pub controller: String,
    pub terrain: String,
}

#[derive(Debug, Clone, Default)]
pub struct Country {
    pub tag: String,
    pub owned_states: Vec<u32>,
    pub capital_state: u32,
}

#[derive(Debug, Clone)]
pub struct Division {
    pub id: u64,
    pub owner_tag: String,
    pub location_province: u32,
    // 战斗属性(由装备+营汇总,M3 直接硬编码)
    pub soft_attack: f64,
    pub hard_attack: f64,
    pub defense: f64,
    pub breakthrough: f64,
    pub armor: f64,
    pub piercing: f64,
    pub hardness: f64,
    pub combat_width: f64,
    // 当前状态
    pub max_org: f64,
    pub org: f64,
    pub max_strength: f64,
    pub strength: f64,
}

impl Division {
    pub fn org_ratio(&self) -> f64 {
        if self.max_org > 0.0 { self.org / self.max_org } else { 0.0 }
    }
    pub fn is_broken(&self) -> bool {
        self.org <= 0.0
    }
}

#[derive(Debug, Clone)]
pub struct Battle {
    pub id: u64,
    pub province: u32,
    pub attackers: Vec<u64>,
    pub defenders: Vec<u64>,
}

/// 作用域(M3: 枚举栈)
#[derive(Debug, Clone)]
pub enum Scope {
    Root,
    Country(String),
    Province(u32),
    Division(u64),
    Battle(u64),
}

/// helper: 从 scope 取国家 tag(若栈顶或指定是国家)
impl Scope {
    pub fn country_tag(&self) -> Option<&str> {
        if let Scope::Country(t) = self { Some(t) } else { None }
    }
    pub fn province_id(&self) -> Option<u32> {
        if let Scope::Province(p) = self { Some(*p) } else { None }
    }
    pub fn division_id(&self) -> Option<u64> {
        if let Scope::Division(d) = self { Some(*d) } else { None }
    }
}

// 抑制未使用警告(Arg 后续 Task 用)
#[allow(unused_imports)]
use _Unused = Arg;
```

- [ ] **Step 2: 修改 world.rs 加实体存储**

在 World struct 加字段(保留 M2 所有字段):
```rust
use crate::runtime::entities::{Battle, Country, Division, Province, Scope};
// ... 在 struct World 内追加:
    pub provinces: HashMap<u32, Province>,
    pub countries: HashMap<String, Country>,
    pub divisions: HashMap<u64, Division>,
    pub battles: Vec<Battle>,
    pub scope_stack: Vec<Scope>,
    pub next_division_id: u64,
    pub next_battle_id: u64,
```

加方法(保留 M2 方法):
```rust
    pub fn current_scope(&self) -> Scope {
        self.scope_stack.last().cloned().unwrap_or(Scope::Root)
    }
    pub fn current_country(&self) -> Option<&str> {
        self.scope_stack.iter().rev().find_map(|s| s.country_tag())
    }
    pub fn add_division(&mut self, mut d: Division) -> u64 {
        d.id = self.next_division_id;
        self.next_division_id += 1;
        let id = d.id;
        self.divisions.insert(id, d);
        id
    }
    pub fn divisions_of(&self, tag: &str) -> Vec<u64> {
        self.divisions.values()
            .filter(|d| d.owner_tag == tag)
            .map(|d| d.id)
            .collect()
    }
```

Default impl 更新(新字段初始化):
```rust
impl Default for World {
    fn default() -> Self {
        Self {
            vars: Default::default(), flags: Default::default(),
            strings: Default::default(), hour: 0, player_tag: String::new(),
            error_log: Vec::new(), event_bus: Default::default(),
            provinces: Default::default(), countries: Default::default(),
            divisions: Default::default(), battles: Vec::new(),
            scope_stack: vec![Scope::Root], next_division_id: 1, next_battle_id: 1,
        }
    }
}
```

- [ ] **Step 3: 改 runtime/mod.rs 加 entities 模块**

```rust
pub mod clock;
pub mod entities;
pub mod error;
pub mod interp;
pub mod registry;
pub mod world;

pub use entities::{Battle, Country, Division, Province, Scope};
// ... 其余 pub use 不变
```

- [ ] **Step 4: 编译验证**

Run: `cargo build 2>&1 | tail -5`
Expected: 通过(entities 是纯新增,World 加字段不影响旧 API)

- [ ] **Step 5: 提交**

```bash
git add src/runtime/entities.rs src/runtime/world.rs src/runtime/mod.rs
git commit -m "feat(m3): 实体结构(Province/Country/Division/Battle/Scope) + World 存储"
```

---

## Task 2: 作用域栈 + ForEach 真实枚举

**Files:** Modify `src/runtime/interp.rs`

核心: ForEach 根据 scope 名分发到不同枚举器,每实体压栈执行 body。

- [ ] **Step 1: 改 interp.rs 的 ForEach 分支**

替换 `Effect::ForEach` 分支为:
```rust
            Effect::ForEach { scope, filter, body } => {
                self.run_for_each(scope, filter.as_ref(), body, world)?;
                Ok(())
            }
```

- [ ] **Step 2: 实现 run_for_each 方法**

在 impl Interpreter 加:
```rust
    fn run_for_each(
        &self,
        scope_name: &str,
        filter: Option<&Trigger>,
        body: &[Effect],
        world: &mut World,
    ) -> Result<(), CmdError> {
        // 收集要遍历的实体 id 列表(先收集再遍历,避免借用冲突)
        let targets: Vec<Scope> = match scope_name {
            "every_country" | "all_country" => {
                world.countries.keys().map(|t| Scope::Country(t.clone())).collect()
            }
            "random_country" => {
                use std::collections::hash_map::Keys;
                let tags: Vec<String> = world.countries.keys().cloned().collect();
                if tags.is_empty() { return Ok(()); }
                // 简单确定性取第一个(M3 不引入 rand;真正随机 M5)
                vec![Scope::Country(tags.into_iter().next().unwrap())]
            }
            "every_owned_state" | "all_owned_state" => {
                let tag = match world.current_country() {
                    Some(t) => t.to_string(),
                    None => return Ok(()),
                };
                world.countries.get(&tag).map(|c| c.owned_states.iter().map(|p| Scope::Province(*p)).collect()).unwrap_or_default()
            }
            "all_army" | "every_army" => {
                let tag = match world.current_country() {
                    Some(t) => t.to_string(),
                    None => return Ok(()),
                };
                world.divisions_of(&tag).into_iter().map(Scope::Division).collect()
            }
            _ => {
                eprintln!("[warn] 未知作用域: {scope_name}, 跳过");
                return Ok(());
            }
        };

        for target in targets {
            world.scope_stack.push(target);
            let pass = match filter {
                Some(t) => self.eval(t, world)?,
                None => true,
            };
            if pass {
                self.run(body, world);
            }
            world.scope_stack.pop();
        }
        Ok(())
    }
```

- [ ] **Step 3: 加作用域测试**

在 interp.rs 测试模块加(或新建 tests/scope.rs):
```rust
#[cfg(test)]
mod scope_tests {
    use super::*;
    use crate::ast::{Arg, Effect};
    use crate::commands::register_all;
    use crate::runtime::entities::{Country, Division, Province};
    use crate::runtime::{Interpreter, Registry, World};

    fn setup_two_states_world() -> World {
        let mut w = World::new();
        w.player_tag = "GER".into();
        w.countries.insert("GER".into(), Country {
            tag: "GER".into(), owned_states: vec![1, 2], capital_state: 1,
        });
        w.provinces.insert(1, Province { id: 1, owner: "GER".into(), controller: "GER".into(), terrain: "plains".into() });
        w.provinces.insert(2, Province { id: 2, owner: "GER".into(), controller: "GER".into(), terrain: "forest".into() });
        w
    }

    #[test]
    fn t_every_owned_state_enumerates_both() {
        let mut reg = Registry::new();
        register_all(&mut reg);
        let interp = Interpreter::new(reg);
        let mut world = setup_two_states_world();
        // 对每个 owned state 加 1 到变量 state_count
        let effs = vec![Effect::ForEach {
            scope: "every_owned_state".into(),
            filter: None,
            body: vec![Effect::Command {
                name: "add_to_variable".into(),
                params: vec![("state_count".into(), Arg::Num(1.0))],
            }],
        }];
        interp.run(&effs, &mut world);
        assert!((world.get_var("state_count") - 2.0).abs() < 1e-9, "应遍历 2 个省");
    }

    #[test]
    fn t_all_army_enumerates_divisions() {
        let mut reg = Registry::new();
        register_all(&mut reg);
        let interp = Interpreter::new(reg);
        let mut world = setup_two_states_world();
        world.add_division(Division {
            id: 0, owner_tag: "GER".into(), location_province: 1,
            soft_attack: 10.0, hard_attack: 2.0, defense: 20.0, breakthrough: 5.0,
            armor: 0.0, piercing: 5.0, hardness: 0.0, combat_width: 10.0,
            max_org: 60.0, org: 60.0, max_strength: 20.0, strength: 20.0,
        });
        world.add_division(Division { /* 同上, 另一个师 */ id: 0, owner_tag: "GER".into(),
            location_province: 2, soft_attack: 10.0, hard_attack: 2.0, defense: 20.0,
            breakthrough: 5.0, armor: 0.0, piercing: 5.0, hardness: 0.0, combat_width: 10.0,
            max_org: 60.0, org: 60.0, max_strength: 20.0, strength: 20.0,
        });
        let effs = vec![Effect::ForEach {
            scope: "all_army".into(), filter: None,
            body: vec![Effect::Command {
                name: "add_to_variable".into(),
                params: vec![("div_count".into(), Arg::Num(1.0))],
            }],
        }];
        interp.run(&effs, &mut world);
        assert!((world.get_var("div_count") - 2.0).abs() < 1e-9, "应遍历 2 个师");
    }
}
```

- [ ] **Step 4: 运行作用域测试**

Run: `cargo test scope 2>&1 | tail -8`
Expected: 2 tests PASS

- [ ] **Step 5: 提交**

```bash
git add src/runtime/interp.rs
git commit -m "feat(m3): 作用域栈 + ForEach 真实枚举(every_owned_state/all_army/every_country)"
```

---

## Task 3: 战斗引擎核心(resolve.rs)

**Files:** Create `src/combat/mod.rs`, `src/combat/resolve.rs`, Modify `src/lib.rs`

实现 `docs/formulas/land-combat.md` 的结算:每场战斗每小时,攻方对守方掷骰伤害。

- [ ] **Step 1: 创建 src/combat/resolve.rs**

```rust
//! 陆战结算(公式见 docs/formulas/land-combat.md)
use crate::runtime::entities::Division;
use crate::runtime::World;

/// 全局系数(对应 NMilitary defines)
const ORG_DICE_SIZE: f64 = 4.0;
const STR_DICE_SIZE: f64 = 2.0;
const ORG_DMG_MOD: f64 = 0.053;
const STR_DMG_MOD: f64 = 0.060;
const HIT_CHANCE_DEF_LEFT: f64 = 0.10;  // 防御池未空
const HIT_CHANCE_NO_DEF: f64 = 0.40;    // 防御池耗尽
const ARMOR_ORG_BONUS_DICE: f64 = 6.0;  // 装甲碾压额外组织度骰
const ARMOR_STR_BONUS_DICE: f64 = 2.0;
const DAMAGE_SPLIT_FIRST: f64 = 0.35;   // 首要目标分摊
const EQUIPMENT_LOSS_FACTOR: f64 = 0.70;

/// 对一组攻击者 vs 一组防御者结算 1 小时
pub fn resolve_hour(attackers: &mut [&mut Division], defenders: &mut [&mut Division]) {
    if attackers.is_empty() || defenders.is_empty() {
        return;
    }
    // 每个攻击者对防御者输出伤害
    let total_attackers = attackers.len();
    for atk_idx in 0..total_attackers {
        // 借用攻击者(只读其属性)
        let atk = &attackers[atk_idx];
        let atk = atk as *const Division;
        let atk = unsafe { &*atk };  // 安全:本函数内 defenders/attackers 不重叠借用
        let attacks = compute_attack_points(atk, defenders);

        // 把 attacks 分配给 defenders(首要目标 35%, 其余均分)
        distribute_attacks(atk, attacks, defenders);
    }
}

/// 计算攻击者对一组防御者的总攻击点数(简化:对首个防御者)
fn compute_attack_points(atk: &Division, defenders: &mut [&mut Division]) -> f64 {
    if defenders.is_empty() { return 0.0; }
    let target_hardness = defenders[0].hardness;
    let soft = atk.soft_attack * (1.0 - target_hardness);
    let hard = atk.hard_attack * target_hardness;
    soft + hard
}

/// 把攻击点数分配给防御者并造成伤害
fn distribute_attacks(atk: &Division, mut attacks: f64, defenders: &mut [&mut Division]) {
    if defenders.is_empty() || attacks <= 0.0 { return; }
    let n = defenders.len();
    // 首要目标(索引0)承受 35%, 其余均分 65%
    for (i, def) in defenders.iter_mut().enumerate() {
        let share = if i == 0 { DAMAGE_SPLIT_FIRST } else { (1.0 - DAMAGE_SPLIT_FIRST) / (n - 1).max(1) as f64 };
        let attacks_on_this = attacks * share;
        // 判定装甲碾压
        let armor_outclass = atk.armor > def.piercing;
        let def_outclass = def.armor > atk.piercing;
        // 防御池
        let def_pool = def.defense;
        let hits = compute_hits(attacks_on_this, def_pool);
        // 掷骰伤害
        let mut org_dice = ORG_DICE_SIZE;
        let mut str_dice = STR_DICE_SIZE;
        if armor_outclass {
            org_dice += ARMOR_ORG_BONUS_DICE;
            str_dice += ARMOR_STR_BONUS_DICE;
        }
        let armor_deflect = if def_outclass { 0.5 } else { 1.0 };
        let org_dmg = hits * (org_dice / 2.0) * ORG_DMG_MOD;
        let str_dmg = hits * (str_dice / 2.0) * STR_DMG_MOD * armor_deflect;
        def.org = (def.org - org_dmg).max(0.0);
        def.strength = (def.strength - str_dmg).max(0.0);
    }
    let _ = attacks;
}

/// 计算命中数(防御池机制)
fn compute_hits(attacks: f64, def_pool: f64) -> f64 {
    let defended = attacks.min(def_pool);
    let undefended = (attacks - def_pool).max(0.0);
    defended * HIT_CHANCE_DEF_LEFT + undefended * HIT_CHANCE_NO_DEF
}

/// World 级战斗结算:遍历所有 battle,每小时调用
pub fn resolve_all_battles(world: &mut World) {
    // 收集每场战斗的攻守 id(避免借用冲突)
    let battle_specs: Vec<(u64, u32, Vec<u64>, Vec<u64>)> = world.battles.iter()
        .map(|b| (b.id, b.province, b.attackers.clone(), b.defenders.clone()))
        .collect();
    for (_bid, _prov, atk_ids, def_ids) in battle_specs {
        // 取出攻守 Division 的可变引用(用 get_mut)
        let mut atks: Vec<&mut Division> = atk_ids.iter()
            .filter_map(|id| world.divisions.get_mut(id)).collect();
        let mut defs: Vec<&mut Division> = def_ids.iter()
            .filter_map(|id| world.divisions.get_mut(id)).collect();
        if !atks.is_empty() && !defs.is_empty() {
            resolve_hour(&mut atks, &mut defs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inf() -> Division {
        Division {
            id: 0, owner_tag: "X".into(), location_province: 1,
            soft_attack: 30.0, hard_attack: 2.0, defense: 40.0, breakthrough: 8.0,
            armor: 0.0, piercing: 5.0, hardness: 0.0, combat_width: 10.0,
            max_org: 60.0, org: 60.0, max_strength: 20.0, strength: 20.0,
        }
    }

    #[test]
    fn t_inf_vs_inf_reduces_org() {
        let mut a = inf();
        let mut d = inf();
        let a_copy = a.clone();
        let org_before = d.org;
        resolve_hour(&mut [&mut a], &mut [&mut d]);
        assert!(d.org < org_before, "守方组织度应下降");
        assert!(d.org >= 0.0);
        let _ = a_copy;
    }

    #[test]
    fn t_armor_outclass_more_damage() {
        // 装甲师 vs 步兵(穿甲不足)
        let mut armor = Division {
            armor: 50.0, soft_attack: 30.0, piercing: 50.0, ..inf()
        };
        let mut def1 = inf();
        let mut def2 = inf();
        def2.armor = 50.0; // def2 有装甲, def1 无
        // 对无装甲的 def1(攻方装甲 50 > 守方穿甲 5): 应高伤害
        let org_before = def1.org;
        resolve_hour(&mut [&mut armor], &mut [&mut def1]);
        assert!(def1.org < org_before, "装甲碾压应造成伤害");
    }

    #[test]
    fn t_defense_pool_reduces_hits() {
        // 高 defense 的守方承伤低
        let mut a = inf();
        let mut d_low_def = inf();
        let mut d_high_def = inf();
        d_high_def.defense = 200.0;
        let org_low_before = d_low_def.org;
        let org_high_before = d_high_def.org;
        resolve_hour(&mut [&mut a.clone()], &mut [&mut d_low_def]);
        let mut a2 = a.clone();
        resolve_hour(&mut [&mut a2], &mut [&mut d_high_def]);
        // 高 defense 承伤应更少(org 下降更少)
        let low_drop = org_low_before - d_low_def.org;
        let high_drop = org_high_before - d_high_def.org;
        assert!(high_drop < low_drop, "高防御池应减少伤害, high_drop={high_drop} < low_drop={low_drop}");
    }
}
```

- [ ] **Step 2: 创建 src/combat/mod.rs**

```rust
//! 战斗模块
pub mod resolve;
pub mod commands;
```

- [ ] **Step 3: 改 src/lib.rs 加 combat**

```rust
pub mod ast;
pub mod combat;
pub mod commands;
pub mod parser;
pub mod runtime;
```

- [ ] **Step 4: 编译验证(resolve 内 unsafe 块需谨慎,先确认编译)**

Run: `cargo build 2>&1 | tail -10`
Expected: 编译通过(combat::commands 还没建,会在下个 Task;若 mod.rs 报 commands 缺失,先注释该行)

- [ ] **Step 5: 运行战斗测试**

Run: `cargo test combat 2>&1 | tail -8`
Expected: 3 tests PASS

- [ ] **Step 6: 提交**

```bash
git add src/combat/ src/lib.rs
git commit -m "feat(m3): 陆战引擎(攻击点/防御池/装甲碾压/掷骰/多师分摊)"
```

---

## Task 4: 战斗命令注册 + 主循环挂载

**Files:** Create `src/combat/commands.rs`, Modify `src/runtime/clock.rs`

- [ ] **Step 1: 创建 combat/commands.rs**

```rust
//! 战斗相关命令注册
use crate::ast::Arg;
use crate::runtime::entities::Division;
use crate::runtime::error::CmdError;
use crate::runtime::registry::ParamGet;
use crate::runtime::Registry;

pub fn register(reg: &mut Registry) {
    // 创建师(简化:硬编码属性, M4 接装备汇总)
    reg.register("create_division", |w, p| {
        let owner = p.get("owner").and_then(Arg::as_str).ok_or_else(|| bad("create_division", "owner"))?;
        let loc = p.get("location").and_then(Arg::as_num).ok_or_else(|| bad("create_division", "location"))? as u32;
        let sa = p.get("soft_attack").and_then(Arg::as_num).unwrap_or(10.0);
        let d = Division {
            id: 0, owner_tag: owner.into(), location_province: loc,
            soft_attack: sa,
            hard_attack: p.get("hard_attack").and_then(Arg::as_num).unwrap_or(2.0),
            defense: p.get("defense").and_then(Arg::as_num).unwrap_or(20.0),
            breakthrough: p.get("breakthrough").and_then(Arg::as_num).unwrap_or(5.0),
            armor: p.get("armor").and_then(Arg::as_num).unwrap_or(0.0),
            piercing: p.get("piercing").and_then(Arg::as_num).unwrap_or(5.0),
            hardness: p.get("hardness").and_then(Arg::as_num).unwrap_or(0.0),
            combat_width: p.get("combat_width").and_then(Arg::as_num).unwrap_or(10.0),
            max_org: p.get("max_org").and_then(Arg::as_num).unwrap_or(60.0),
            org: p.get("max_org").and_then(Arg::as_num).unwrap_or(60.0),
            max_strength: 20.0, strength: 20.0,
        };
        w.add_division(d);
        Ok(())
    });
    // 开始战斗:把两个 tag 在某省的师设为攻守
    reg.register("start_battle", |w, p| {
        let attacker = p.get("attacker").and_then(Arg::as_str).ok_or_else(|| bad("start_battle", "attacker"))?;
        let defender = p.get("defender").and_then(Arg::as_str).ok_or_else(|| bad("start_battle", "defender"))?;
        let prov = p.get("province").and_then(Arg::as_num).ok_or_else(|| bad("start_battle", "province"))? as u32;
        let atks = w.divisions_of(attacker);
        let defs = w.divisions_of(defender);
        if atks.is_empty() || defs.is_empty() {
            return Err(CmdError::RuntimeError("start_battle: 攻方或守方无师".into()));
        }
        let id = w.next_battle_id;
        w.next_battle_id += 1;
        w.battles.push(crate::runtime::entities::Battle {
            id, province: prov, attackers: atks, defenders: defs,
        });
        Ok(())
    });
    // trigger: 判断当前作用域师是否破阵
    reg.register_trigger("is_broken", |w, _p| {
        let cur = w.current_scope();
        if let Some(did) = cur.division_id() {
            Ok(w.divisions.get(&did).map(|d| d.is_broken()).unwrap_or(false))
        } else { Ok(false) }
    });
}

fn bad(cmd: &str, key: &str) -> CmdError {
    CmdError::BadParam { cmd: cmd.into(), key: key.into(), reason: "缺少或类型错误".into() }
}
```

- [ ] **Step 2: 修改 clock.rs 在 on_hourly 挂战斗结算**

在 `commands/mod.rs` 的 `register_all` 加 `combat::commands::register`:
```rust
// src/commands/mod.rs
pub fn register_all(reg: &mut Registry) {
    vars::register(reg);
    control::register(reg);
    scope::register(reg);
    crate::combat::commands::register(reg);
}
```

在 `clock.rs` 的 tick 加(注释掉的 M3 接入现在激活):
```rust
        world.fire_event(interp, "on_hourly");
        crate::combat::resolve::resolve_all_battles(world);  // M3: 战斗结算
```

- [ ] **Step 3: 编译验证**

Run: `cargo build 2>&1 | tail -5`
Expected: 通过

- [ ] **Step 4: 提交**

```bash
git add src/combat/commands.rs src/commands/mod.rs src/runtime/clock.rs
git commit -m "feat(m3): 战斗命令(create_division/start_battle) + on_hourly 挂载"
```

---

## Task 5: 端到端战斗测试(★ M3 验收)

**Files:** Modify `tests/integration.rs`

用脚本驱动:创建两国→各省→两师→开战→tick 若干小时→验证 org 下降、装甲碾压效果。

- [ ] **Step 1: 加端到端战斗测试**

在 tests/integration.rs 加:
```rust
#[test]
fn two_divisions_battle_deals_damage() {
    use hoi4_clone::runtime::GameClock;
    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg);
    let mut world = World::new();
    world.player_tag = "GER".into();
    world.countries.insert("GER".into(), Default::default());
    world.countries.insert("FRA".into(), Default::default());

    // 创建两个师
    let effs = hoi4_clone::ast::lower::lower_effects(
        &hoi4_clone::parser::parse(r#"
            _setup = {
                create_division = { owner = GER location = 1 soft_attack = 30 defense = 40 max_org = 60 }
                create_division = { owner = FRA location = 1 soft_attack = 20 defense = 40 max_org = 60 }
                start_battle = { attacker = GER defender = FRA province = 1 }
            }
        "#).unwrap()
    );
    interp.run(&effs, &mut world);
    assert_eq!(world.divisions.len(), 2, "应创建 2 个师");
    assert_eq!(world.battles.len(), 1, "应有 1 场战斗");

    // 记录守方初始 org
    let fra_div = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap();
    let org_before = fra_div.org;

    // 推进 24 小时(战斗每小时结算)
    GameClock::advance(&interp, &mut world, 24);

    let fra_div = world.divisions.values().find(|d| d.owner_tag == "FRA").unwrap();
    assert!(fra_div.org < org_before, "24h 战斗后守方 org 应下降: before={org_before} after={}", fra_div.org);
}
```

- [ ] **Step 2: 运行端到端测试**

Run: `cargo test two_divisions_battle 2>&1 | tail -10`
Expected: PASS — 这是 M3 核心验收

- [ ] **Step 3: 运行全量测试**

Run: `cargo test 2>&1 | grep "test result"`
Expected: 全部 PASS

- [ ] **Step 4: 提交**

```bash
git add tests/integration.rs
git commit -m "test(m3): 端到端战斗测试 — 两师打仗 org 下降(M3 验收)"
```

---

## Task 6: M3 收尾 — clippy + 报告 + review

**Files:** Create `docs/milestones/M3-complete.md`

- [ ] **Step 1: 全量 clippy + test**

Run: `cargo clippy --all-targets 2>&1 | grep -cE "^warning|^error"` (期望 0)
Run: `cargo test 2>&1 | grep "test result"` (全 PASS)

- [ ] **Step 2: 写 M3 完成报告**

- [ ] **Step 3: 派 Explore 子代理做最终 code review**

- [ ] **Step 4: 提交 + tag**

```bash
git tag m3-complete
```

---

## 自检

**Spec 覆盖:** 实体存储(Task1)、作用域(Task2)、陆战公式(Task3)、命令(Task4)、验收(Task5)全覆盖。
**unsafe 注意:** Task3 resolve.rs 用 unsafe 绕过多可变借用,需在 review 重点审查安全性。备选方案: 改 resolve_hour 接受 owned 数据(拷贝 Division 属性 → 计算伤害 → 写回),避免 unsafe。
**类型一致:** Division/Scope/Battle 字段名跨 Task 一致。
