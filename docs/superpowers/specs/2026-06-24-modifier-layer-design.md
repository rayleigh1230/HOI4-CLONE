# Modifier 层(陆战结算统一修正接口) 设计文档

> 日期: 2026-06-24
> 状态: 已批准(头脑风暴),待实现
> 关联: `docs/design-principles.md`(原则1: 原版设计是首要参考)
> 关联: `docs/formulas/land-combat.md`(战斗修正表)
> 关联: `docs/HANDOFF.md §5`(未实现交战规则, 大多走 modifier)

---

## 0. 背景与目标

### 现状问题

当前陆战结算的 `effective_*` 方法只乘了补给系数, 没有 modifier 层:

```rust
// 现在(resolve.rs / entities.rs):
pub fn effective_soft_attack(&self) -> f64 {
    self.soft_attack * self.supply_ratio()   // 只有一个系数
}
```

后续每个系统(科技/国策/将领/堑壕/地形/补给/精神)都要影响陆战结算。若没有统一接口, 每加一个系统就要改 `effective_soft_attack` 等 4 个方法, modifier 之间如何叠加会越改越乱。**这是"现在不做、后面每个系统都重构结算层"的唯一地基缺口。**

### 目标

引入 Modifier 层作为陆战结算的统一修正接口。后续所有系统通过"往 ModifierStack 塞 modifier"影响结算, 不再各自改结算代码。

### 范围(本次做)

- **覆盖三个结算点**: 战斗属性(soft/hard/defense/breakthrough/armor/piercing)、战斗宽度上限、org 恢复率
- **数据模型**: Modifier / ModifierStat / ModifierOp / ModifierStack + multiplier 查询
- **作用域汇总**: CombatContext 把国家/省份/师三层 modifier 汇总到结算点
- **命令接口**: add_country_modifier / add_division_modifier(运行时动态加)
- **属性名解析**: `_factor` 后缀推导 op(对齐原版)

### 非目标(本次不做)

- 移动速度/增援率口子(等补给系统/卡车时再加, 贴合需求)
- 具体 modifier 内容(科技/国策/堑壕/地形的数值)——等对应系统实现时再往接口塞
- ideas/technologies 文件加载——loader 接 parse_modifier_token, 但具体加载留待对应系统
- 战术系统(combat tactics)——独立乘层, 留待战术系统

---

## 1. 核心设计决策(头脑风暴结论)

| # | 决策 | 选择 |
|---|---|---|
| 1 | 覆盖范围 | 战斗属性 + 宽度 + org恢复(折中) |
| 2 | 叠加公式 | 同类 Add 相加 + 异类 Multiply 相乘: `(1+ΣAdd) × Π(1+Multiply)` |
| 3 | op 来源 | 属性名后缀推导(对齐原版 Paradox 约定): 无后缀=Add, `_factor`=Multiply |
| 4 | 作用域汇总 | CombatContext 结算前快照, 汇总国家+省份+师三层 |
| 5 | modifier 存储 | Country/Division 各存各的 ModifierStack; 省份层查地形表(静态) |
| 6 | 借用安全 | CombatContext 只读快照, 避免结算时同时读 modifier 和写 division |
| 7 | 零破坏保证 | 空 ModifierStack multiplier 返回 1.0, 默认状态精确还原现状 |

---

## 2. 数据模型

### 2.1 Modifier / ModifierStat / ModifierOp

```rust
/// 单个 modifier: 作用在某属性上的一个修正
#[derive(Debug, Clone)]
pub struct Modifier {
    pub stat: ModifierStat,
    pub value: f64,           // 0.05 = +5%
    pub op: ModifierOp,       // 由属性名后缀推导, 构造时填好
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModifierOp {
    /// 无后缀(soft_attack): 加进 add 池, 同类相加
    Add,
    /// _factor 后缀(soft_attack_factor): 独立乘一层
    Multiply,
}

/// 可被修正的属性(本次覆盖战斗属性+宽度+org恢复)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierStat {
    // 战斗属性(effective_* 的 6 个)
    SoftAttack, HardAttack, Defense, Breakthrough, Armor, Piercing,
    // 战斗宽度上限
    CombatWidth,
    // 组织度恢复率
    OrgRegain,
}
```

### 2.2 属性名后缀推导规则(对齐原版)

原版 Paradox 脚本约定: 每个属性有两个变体名, op 由后缀决定:

| 属性 | Add 类(无后缀, 直接加) | Multiply 类(`_factor` 后缀) |
|---|---|---|
| 软攻击 | `soft_attack` | `soft_attack_factor` |
| 硬攻击 | `hard_attack` | `hard_attack_factor` |
| 防御 | `defense` / `defence` | `defense_factor` |
| 突破 | `breakthrough` | `breakthrough_factor` |
| 装甲 | `armor` / `armor_value` | `armor_factor` |
| 穿甲 | `piercing` / `ap_attack` | `ap_attack_factor` |
| 宽度 | `combat_width` | `combat_width_factor` |
| org恢复 | `org_regain` / `local_org_regain` | `org_regain_factor` |

**一个 idea 可同时写两种**(语义都合法且共存):
```hoi4
modifier = {
    soft_attack = 10           # 加 10 点(add)
    soft_attack_factor = 0.05  # 再乘 +5%(multiply)
}
```

### 2.3 统一解析函数

所有来源(脚本命令/数据文件/所有 loader)走同一个解析:

```rust
/// 字符串属性名 → (stat, op)
/// _factor 后缀 → Multiply; 无后缀 → Add
/// 未知属性 → None(静默跳过)
fn parse_modifier_token(s: &str) -> Option<(ModifierStat, ModifierOp)> {
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
```

### 2.4 ModifierStack + multiplier

```rust
/// 一组 modifier 的集合, 按 stat 查询最终乘数
#[derive(Debug, Clone, Default)]
pub struct ModifierStack {
    mods: Vec<Modifier>,
}

impl ModifierStack {
    pub fn new() -> Self { Self { mods: vec![] } }

    /// 推入一个 modifier
    pub fn push(&mut self, m: Modifier) { self.mods.push(m); }

    /// 合并另一个 stack(用于三层汇总: 国家+省份+师)
    pub fn merge(&mut self, other: &ModifierStack) {
        self.mods.extend(other.mods.iter().cloned());
    }

    /// 算某属性的总系数(面板值 × 这个 = 最终值)
    /// 公式: (1 + Σ Add类) × Π(1 + Multiply类)
    /// - Add 类(科技/精神/将领的百分比加成): 同类相加
    /// - Multiply 类(地形/战术/堑壕/计划/designer): 异层相乘
    /// 空栈返回 1.0(默认无修正, 精确还原现状)
    pub fn multiplier(&self, stat: ModifierStat) -> f64 {
        let add_sum: f64 = self.mods.iter()
            .filter(|m| m.stat == stat && m.op == ModifierOp::Add)
            .map(|m| m.value).sum();
        let mult_prod = self.mods.iter()
            .filter(|m| m.stat == stat && m.op == ModifierOp::Multiply)
            .fold(1.0, |acc, m| acc * (1.0 + m.value));
        (1.0 + add_sum) * mult_prod
    }
}
```

**为什么这个公式安全**:
- 负向修正(地形-15%、夜间-50%、堆叠-2%/师)在原版几乎全是 Multiply 类 → 逐个相乘, 永不负
- Add 类几乎都是正向增益(科技/精神/将领) → 相加不会跌破有意义范围
- 极端情况(Add 之和 < -1)极少见, 且原版数据不会出现

---

## 3. 作用域与结算时汇总

### 3.1 三层 modifier 来源

一个师的最终属性, 汇总三层 modifier:

```
国家层:  科技 +10% 软攻、精神 +5%、战争支持度...   (Country.modifiers)
省份层:  地形 -15%(森林)、要塞、渡河...            (查地形表, 静态)
师自身:  堑壕 +2%/级、计划加成、经验...             (Division.modifiers)
```

### 3.2 CombatContext — 结算前快照

引入轻量结构, 结算前一次性算好"每个参战师的 modifier 汇总":

```rust
/// 一场战斗的结算上下文(结算前算好, 结算中只读)
pub struct CombatContext {
    /// 每个参战师的 modifier 汇总(按 division_id 索引)
    /// = 国家modifier + 该师所在省modifier + 师自身modifier
    pub stacks: HashMap<u64, ModifierStack>,
}

impl CombatContext {
    /// 结算前构造: 遍历 battle 攻守双方, 为每个师算 modifier 汇总
    pub fn build(world: &World, battle: &Battle) -> CombatContext {
        let mut stacks = HashMap::new();
        for div_id in battle.attackers.iter().chain(&battle.defenders)
            .chain(&battle.reserve_attackers).chain(&battle.reserve_defenders) {
            let Some(d) = world.divisions.get(div_id) else { continue };
            let mut stack = ModifierStack::new();
            // 国家层
            if let Some(c) = world.countries.get(&d.owner_tag) {
                stack.merge(&c.modifiers);
            }
            // 省份层(地形表, 静态查)
            if let Some(p) = world.provinces.get(&battle.province) {
                stack.merge(&terrain_modifiers(&p.terrain));
            }
            // 师自身
            stack.merge(&d.modifiers);
            stacks.insert(*div_id, stack);
        }
        CombatContext { stacks }
    }

    /// 取某师的 modifier 汇总(找不到则空栈)
    pub fn get(&self, div_id: u64) -> &ModifierStack {
        self.stacks.get(&div_id).unwrap_or_else(|| &EMPTY)
    }
}
```

### 3.3 关键设计要点

1. **CombatContext 是只读快照**。结算前算好, 结算中只读 ctx.stacks + 写 division 血量, 不碰 world——避开借用冲突。

2. **三层各存各的**:
   - `Country.modifiers: ModifierStack`(科技/精神/ideas 加的)
   - `Division.modifiers: ModifierStack`(堑壕/计划/经验 加的)
   - 省份层不存(地形是静态查表 `terrain_modifiers(terrain)`, 地形不改)
   - 每个系统只管往自己负责的实体加 modifier, 互不干扰。

3. **terrain_modifiers 是占位**。本次提供一个返回空栈的版本(无地形数据), 后续地形系统实现时填真实地形修正。

### 3.4 快照设计支持动态 modifier(昼夜/天气/季节)

CombatContext 的快照设计**天然支持随时间变化的 modifier**——不只是静态的地形, 昼夜、天气、季节这类动态修正也能冻结进快照。这是快照优于"结算时实时读 world"的关键场景。

**数据流(以昼夜为例)**:

```
主循环每小时(clock.rs tick):
  1. hour += 1
  2. 算当前小时各省的 darkness(依赖纬度+日期) → 存 world   ← 主循环算
  3. resolve_all_battles:
       CombatContext::build(world, battle):
         遍历参战师:
           省份层: stack.merge( terrain_modifiers(p.terrain)
                              + night_modifier(world.darkness[p.id]) )
                                                    ↑ build 时读一次, 冻结进快照
         → 快照已含该小时的夜间修正
       resolve_hour(ctx): 用快照结算, 不再读 world
```

**为什么快照适合动态 modifier**:
- **同一小时用同一个 darkness**: 一场小时级战斗内 darkness 不变, 快照保证这一点。
- **无借用冲突**: darkness 在 build 前算好存 world, build 时只读, 结算时只读快照+写 division——不碰 world。
- **可调试**: 快照可序列化复现某小时的完整修正状态。

**原版昼夜机制(查证自 defines + wiki)**:
- `BASE_NIGHT_ATTACK_PENALTY = -0.5`: 夜间攻击惩罚基础值
- `night` 是 province modifier, `# Multiplied by amount of darkness`
- darkness ∈ [0.0, 1.0]: 0=白天, 1=全黑, 之间是黎明/黄昏过渡
- 实际惩罚 = -0.5 × darkness(全黑 ×0.5 攻击, 半黑 ×0.75)
- darkness 由**省份纬度 + 当前日期**算出(北欧冬夜长, 赤道全年均分)
- 科技(夜视仪)和 `night_attack` 属性可抵消此惩罚

**本次处理**: modifier 层的快照已就位(支持任何动态 modifier)。但 darkness 数据用占位:
- `night_modifier(darkness)` 占位函数, 默认 darkness=0(白天, 无惩罚)
- 现有测试不破(默认无夜间修正)

**昼夜系统的数据前提(后续独立做, 不改 modifier 层)**:
1. `Province` 加纬度(或简化的纬度带)
2. `World` 加日期(年月日, 不只是 hour)
3. 主循环每小时算 darkness 存 `World.darkness: HashMap<省份id, f64>`
4. `CombatContext::build` 省份层加一行 `night_modifier(world.darkness[prov])`

**核心**: 昼夜系统后续做时, 只往 CombatContext::build 的省份层 push 一个动态 modifier, **不改 resolve.rs、不改 effective_*、不改 ModifierStack**。快照设计让动态 modifier 和静态 modifier 走同一条路。

---

## 4. 结算点改造(接口口子)

### 4.1 战斗属性(resolve.rs)

```rust
// 改造前(只乘补给):
impl Division {
    pub fn effective_soft_attack(&self) -> f64 {
        self.soft_attack * self.supply_ratio()
    }
}

// 改造后(补给 × modifier):
impl Division {
    pub fn effective_soft_attack(&self, mods: &ModifierStack) -> f64 {
        self.soft_attack * self.supply_ratio()
            * mods.multiplier(ModifierStat::SoftAttack)
    }
    // hard_attack/defense/breakthrough 同理
}

// AtkStats::from 接 mods:
impl AtkStats {
    fn from(d: &Division, mods: &ModifierStack) -> Self {
        Self {
            soft_attack: d.effective_soft_attack(mods),
            hard_attack: d.effective_hard_attack(mods),
            armor: d.armor * mods.multiplier(ModifierStat::Armor),
            piercing: d.piercing * mods.multiplier(ModifierStat::Piercing),
        }
    }
}
```

`resolve_all_battles` 在结算循环里为每场 battle 构造 CombatContext, 传给 resolve_hour:
```rust
for (atk_ids, def_ids) in &battle_specs {
    let battle = /* 找到对应 battle */;
    let ctx = CombatContext::build(world, battle);  // 结算前快照
    // resolve_hour 内部按 division_id 从 ctx 取 stack
}
```

### 4.2 战斗宽度(width.rs)

```rust
// 改造前:
pub const BASE_COMBAT_WIDTH: f64 = 70.0;
pub fn can_join_frontline(world, frontline, new_div_width) -> bool {
    used + new_div_width <= BASE_COMBAT_WIDTH
}

// 改造后: 宽度上限乘 modifier
pub fn can_join_frontline(world, frontline, new_div_width, mods: &ModifierStack) -> bool {
    let cap = BASE_COMBAT_WIDTH * mods.multiplier(ModifierStat::CombatWidth);
    used + new_div_width <= cap
}
```
地形(森林 60)、科技可改宽度 → 走这个口子。

### 4.3 组织度恢复(recovery.rs)

```rust
// 改造前:
let recovery = hourly * (0.5 + 0.5 * div.supply_ratio());

// 改造后: 乘 org_regain modifier
let recovery = hourly * (0.5 + 0.5 * div.supply_ratio())
    * mods.multiplier(ModifierStat::OrgRegain);
```
精神/补给/地形影响恢复 → 走这个口子。

---

## 5. 命令接口(运行时动态加 modifier)

### 5.1 国家级 modifier

```hoi4
# 国策完成奖励: 给德国加 +10% 软攻(无后缀 → Add)
add_country_modifier = {
    tag = GER
    stat = soft_attack
    value = 0.10
}

# 地形修正(事件给): defense_factor = -0.15(_factor → Multiply)
add_country_modifier = {
    tag = GER
    stat = defense_factor
    value = -0.15
}
```

命令实现(commands.rs):
```rust
reg.register("add_country_modifier", |w, p| {
    let tag = np(p, "add_country_modifier", "tag")?.as_str()...;
    let token = np(p, "add_country_modifier", "stat")?.as_str()...;
    let (stat, op) = parse_modifier_token(token)
        .ok_or_else(|| CmdError::RuntimeError(format!("未知属性: {token}")))?;
    let value = num_of(np(p, "add_country_modifier", "value")?)?;
    let country = w.countries.entry(tag.into()).or_default();
    country.modifiers.push(Modifier { stat, value, op });
    Ok(())
});
```

**注意: 作者不传 op, 传 stat 名(带或不带 _factor)**。解析函数从后缀推导 op, 和原版完全一致。

### 5.2 师级 modifier

```hoi4
# 给某师加堑壕 modifier
add_division_modifier = {
    division = 5
    stat = defense_factor
    value = 0.10
}
```

实现类似, 往 `division.modifiers` push。

---

## 6. 错误处理

1. **默认状态精确还原现状**: `ModifierStack::new()` 空, `multiplier()` 返回 1.0, 等价于无修正。现有测试不破。

2. **未知属性静默跳过**: `parse_modifier_token` 返回 None 时跳过(stability_factor/ace_effectiveness_factor 等本次不处理的), 不报错, 和 loader 既有策略一致。

3. **CombatContext 容错**: 构造时某 division_id 找不到(已歼灭但仍在 battle 列表), 跳过而非 panic。

4. **空栈兜底**: `CombatContext::get(div_id)` 找不到时返回空栈引用(不 panic)。

---

## 7. 文件组织与改动清单

### 7.1 新增文件

```
src/combat/modifier.rs   ← 新增: Modifier/ModifierStat/ModifierOp/ModifierStack
                              + parse_modifier_token + CombatContext + terrain_modifiers(占位)
```

放在 `combat/` 下(与陆战结算紧密相关), 不放 `data/`(不是数据定义, 是运行时计算)。

### 7.2 改动清单

| 文件 | 改动 | 性质 |
|---|---|---|
| `src/combat/modifier.rs` | 全新模块 | 新增 |
| `src/combat/mod.rs` | 声明 modifier 子模块 + re-export | 小改 |
| `src/runtime/entities.rs` | Division 加 `modifiers: ModifierStack`; Country 加 `modifiers`; `effective_*` 加 `mods` 参数 | 改 |
| `src/combat/resolve.rs` | AtkStats::from/pool_value 接 mods; resolve_all_battles 构造 CombatContext | 改 |
| `src/combat/width.rs` | can_join_frontline 宽度上限乘 multiplier | 改 |
| `src/combat/recovery.rs` | org 恢复量乘 multiplier | 改 |
| `src/combat/commands.rs` | 注册 add_country_modifier/add_division_modifier | 改 |
| `src/combat/movement.rs` | resolve_hour 调用点传 ctx(若有) | 小改 |
| parser/ast/data | 不动 | 零改动 |

---

## 8. 测试策略

| 测试组 | 验证内容 | 关键断言 |
|---|---|---|
| multiplier 空栈 | 无 modifier 返回 1.0 | `new().multiplier(SoftAttack) == 1.0` |
| multiplier 纯 Add | 多个 Add 相加 | 0.05+0.10 → 1.15 |
| multiplier 纯 Multiply | 多个 Multiply 相乘 | 0.05 和 0.10 → 1.05×1.10=1.155 |
| multiplier 混合 | Add 后乘 Multiply | (1+0.05)×(1+0.10)=1.155 |
| parse_modifier_token 后缀 | 无后缀=Add, _factor=Multiply | soft_attack→Add, soft_attack_factor→Multiply |
| parse_modifier_token 未知 | 返回 None | stability_factor→None |
| 现有 resolve 回归 | 空 modifier 等价现状 | t_inf_vs_inf_reduces_org 等全绿 |
| effective_* 带 modifier | 正向增益提升攻击 | +50% Add → ×1.5 |
| width 带 modifier | 地形缩宽度 | CombatWidth×0.85 → 上限 59.5 |
| 端到端 | add_country_modifier 影响战斗 | +100% soft 后伤害翻倍 |

---

## 9. 后续扩展(本次预留, 不实现)

| 后续系统 | 如何接入(不改结算层) |
|---|---|
| 科技 | 完成 → add_country_modifier(stat=soft_attack value=0.05) |
| 国策 | completion_reward → add_country_modifier |
| 将领特质 | add_division_modifier / 将领 scope |
| 堑壕 | 战斗系统每小时 dig_in++, 转 add_division_modifier |
| 地形 | terrain_modifiers 函数填真实地形修正(替换占位空栈) |
| **昼夜** | Province 加纬度 + World 加日期; 主循环算 darkness; CombatContext::build 省份层加 night_modifier(darkness)。动态 modifier, 快照天然支持(见 §3.4) |
| 战术 | 战斗时往 CombatContext 加战术层(push 一个 Multiply) |
| 补给不足 | add_division_modifier(stat=soft_attack_factor value=-0.25) |
| ideas 文件 | loader 读 idea 块的 modifier, 走 parse_modifier_token |
| 移动速度口子 | (本次不做)需要时加 ModifierStat::MovementSpeed + movement.rs 口子 |

**核心: 所有后续来源都只往 ModifierStack push 数据, 不改 effective_*/resolve.rs/width.rs/recovery.rs。**

---

## 10. 验收标准

1. `cargo test` 全绿(现有 147 + 新增 modifier 测试, 零回归)
2. 空 ModifierStack 时, `effective_soft_attack` 等返回值与改造前**逐位相同**(现有测试不破)
3. `add_country_modifier = { tag=GER stat=soft_attack value=0.5 }` 后, GER 师 effective_soft_attack 提升 50%
4. `soft_attack_factor` 解析为 Multiply, `soft_attack` 解析为 Add(单元测试)
5. 战斗宽度可通过 modifier 改变(CombatWidth×0.85, 前线容纳变少)
6. **后续加科技/地形/将领系统时, 不改 resolve.rs / effective_* / width.rs / recovery.rs**——只往 ModifierStack push 数据

第 6 条是核心验收: 这是"做一次接口、后续不重构"的兑现标准。
