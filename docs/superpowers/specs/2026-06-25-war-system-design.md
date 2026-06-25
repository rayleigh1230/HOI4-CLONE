# 战争状态系统(War 关系 + 敌人判定) 设计文档

> 日期: 2026-06-25
> 状态: 已批准(头脑风暴),待实现
> 关联: `docs/design-principles.md`(原则1: 原版设计是首要参考)
> 关联: `docs/HANDOFF.md`(当前全员敌对: owner_tag != owner)

---

## 0. 背景与目标

### 现状问题

当前判敌人靠 `owner_tag != owner`——**"不同 tag 就是敌人"**, 全员敌对。原版不是这样: 只有"处于同一场 War 的两国"才互为敌人, 没宣战的两国即使 tag 不同也是中立的。

这个差距导致:
- 无法表达"中立国"(任何不同 tag 都开打)
- 阵营、宣战、加入战争等外交机制无处挂载
- 国策/事件的 `at_war_with = X` trigger 无法实现

### 目标

引入 War 实体(多对多战争关系) + `are_at_war` 判定。把 5 处 `owner_tag != owner` 的敌人判定改成战争关系查询。为阵营、宣战、和谈、投降预留接入点。

### 范围(本次做)

- **War 实体**: 一场战争含攻守两侧(各多个 tag)
- **are_at_war 判定**: 查询两个 tag 是否处于战争状态
- **declare_war / add_to_war / white_peace 命令**: 宣战/加入战争/白和
- **敌人判定改造**: 5 处 `owner_tag != owner` → `are_at_war`
- **Country.faction 字段**: 阵营归属(宣战时自动拉入阵营成员)

### 非目标(本次不做)

- **wargoal(战争目标/正当化)**: 外交系统子机制(需世界紧张度等)
- **和平会议**: 独立复杂系统(割地/傀儡/赔款)
- **投降判定**: 依赖胜利点(State 还没 victory_points)
- **阵营创建/管理**: 只存 faction 字段 + 简单 create_faction/join_faction 命令
- **停战期**: 依赖日期系统(已做), 但和谈结束才记录停战, 和谈没做所以本次也不做

---

## 1. 核心设计决策

| # | 决策 | 选择 |
|---|---|---|
| 1 | 战争数据模型 | War 实体(含 attackers/defenders 两个 tag 集合), 而非 Country 存 enemies 列表 |
| 2 | 为什么用 War 实体 | 一场战争有攻守两侧 + 多国(阵营); 停战/和谈按"战争"为单位; 三国战争归属关系不散乱 |
| 3 | 敌人判定 | `are_at_war(a, b)` 查询(遍历 wars 检查是否分属对立侧); 替换全员敌对 |
| 4 | 阵营 | Country.faction: Option<String>; 宣战时自动拉入同阵营成员 |
| 5 | 默认行为 | 无战争 = 中立(不交战); 现有测试需显式 declare_war 才开打 |

---

## 2. 数据模型

```rust
use std::collections::HashSet;

/// 一场战争(多个参与方, 分攻守两侧)
/// 阵营成员在宣战时自动加入对应侧
#[derive(Debug, Clone)]
pub struct War {
    pub id: u64,
    /// 攻方阵营(tag 集合)
    pub attackers: HashSet<String>,
    /// 守方阵营(tag 集合)
    pub defenders: HashSet<String>,
}
```

Country 加 faction 字段:

```rust
pub struct Country {
    // ... 现有字段 ...
    /// 阵营名(None = 不在阵营; 宣战时同阵营成员自动加入)
    pub faction: Option<String>,
}
```

World 加 wars 存储:

```rust
pub struct World {
    // ... 现有字段 ...
    pub wars: Vec<War>,  // 注意: 这和 battles(战斗)不同——wars 是外交级战争状态
    pub next_war_id: u64,
}
```

> **命名澄清**: `battles` 是战术级(一个省份的交战), `wars` 是战略级(两国间的战争状态)。一个 war 包含多个 battle。

### are_at_war 判定

```rust
impl World {
    /// 判定两个 tag 是否处于战争状态(分属某场 war 的对立两侧)
    pub fn are_at_war(&self, a: &str, b: &str) -> bool {
        self.wars.iter().any(|w| {
            (w.attackers.contains(a) && w.defenders.contains(b))
                || (w.defenders.contains(a) && w.attackers.contains(b))
        })
    }

    /// 取某 tag 的所有交战国(在任一 war 的对立侧)
    pub fn enemies_of(&self, tag: &str) -> Vec<String> {
        let mut enemies = HashSet::new();
        for w in &self.wars {
            if w.attackers.contains(tag) {
                enemies.extend(w.defenders.iter().cloned());
            } else if w.defenders.contains(tag) {
                enemies.extend(w.attackers.iter().cloned());
            }
        }
        enemies.into_iter().collect()
    }
}
```

### 宣战(自动拉入阵营成员)

```rust
impl World {
    /// 宣战: 建立一场新战争, 双方阵营成员自动加入
    pub fn declare_war(&mut self, attacker: &str, defender: &str) -> u64 {
        let id = self.next_war_id;
        self.next_war_id += 1;
        let mut atk = HashSet::new();
        atk.insert(attacker.into());
        atk.extend(self.faction_members(attacker));
        let mut def = HashSet::new();
        def.insert(defender.into());
        def.extend(self.faction_members(defender));
        self.wars.push(War { id, attackers: atk, defenders: def });
        id
    }

    /// 取某 tag 的同阵营成员(含自己)
    fn faction_members(&self, tag: &str) -> Vec<String> {
        let faction = self.countries.get(tag).and_then(|c| c.faction.as_ref());
        match faction {
            None => vec![],
            Some(f) => self.countries.iter()
                .filter(|(_, c)| c.faction.as_deref() == Some(f.as_str()))
                .map(|(t, _)| t.clone())
                .collect(),
        }
    }
}
```

---

## 3. 敌人判定改造(5 处)

把 5 处 `owner_tag != owner` 改成 `are_at_war`:

| 文件:行 | 改造前 | 改造后 |
|---|---|---|
| `commands.rs:387` | `d.owner_tag != owner` | `w.are_at_war(&d.owner_tag, &owner)` |
| `commands.rs:512` | `d.owner_tag != owner` | `w.are_at_war(&d.owner_tag, &owner)` |
| `movement.rs:50` | `od.owner_tag != owner` | `world.are_at_war(&od.owner_tag, &owner)` |
| `movement.rs:188` | `od.owner_tag != owner` | `world.are_at_war(&od.owner_tag, &owner)` |
| `movement.rs:291` | `od.owner_tag != owner` | `world.are_at_war(&od.owner_tag, &owner)` |

hostile 判定(commands.rs:376):
```rust
// 改造前:
let is_hostile = first_controller != owner;
// 改造后(有战争关系才算进军):
let is_hostile = w.are_at_war(first_controller, &owner);
```

**注意借用**: 这些判定在闭包里(world.divisions.values().filter(...))。`are_at_war` 接 `&self`, 闭包捕获 world 不可变借用的同时 filter 借 divisions。需把 `are_at_war` 结果预先算出(快照 enemies 列表), 或用内联查询避免冲突。实现时按具体上下文处理。

---

## 4. 命令接口

```hoi4
# 宣战(建立战争, 阵营自动拉入)
declare_war = { attacker = GER defender = FRA }

# 加入已有战争(某 tag 加入某侧, 对抗某 target 所在的战争)
add_to_war = { tag = ITA side = attacker war_target = FRA }

# 白和(无条件停火, 结束两国间所有战争)
white_peace = { a = GER b = FRA }

# 阵营(简单版)
create_faction = { leader = GER name = "Axis" }
join_faction = { tag = ITA name = "Axis" }
```

命令实现(commands.rs):

```rust
reg.register("declare_war", |w, p| {
    let attacker = np(p, "declare_war", "attacker")?.as_str()?;
    let defender = np(p, "declare_war", "defender")?.as_str()?;
    w.declare_war(attacker, defender);
    Ok(())
});

reg.register("white_peace", |w, p| {
    let a = np(p, "white_peace", "a")?.as_str()?;
    let b = np(p, "white_peace", "b")?.as_str()?;
    // 移除 a 和 b 之间的所有战争
    w.wars.retain(|war| {
        !(war.attackers.contains(a) && war.defenders.contains(b)
            || war.defenders.contains(a) && war.attackers.contains(b))
    });
    Ok(())
});
```

---

## 5. 现有测试迁移

**核心影响**: 现有测试用 `owner_tag != owner` 隐式开战(两国放一起就打)。改造后需显式 `declare_war`。

测试迁移模式:
```hoi4
# 改造前(两国放一起自动开打):
create_division = { owner = GER location = 1 ... }
create_division = { owner = FRA location = 1 ... }
start_battle = { attacker = GER defender = FRA province = 1 }

# 改造后(需先 declare_war 才是敌人):
declare_war = { attacker = GER defender = FRA }
create_division = { owner = GER location = 1 ... }
create_division = { owner = FRA location = 1 ... }
start_battle = { attacker = GER defender = FRA province = 1 }
```

或者: `start_battle` 命令内部自动 declare_war(若两国未在战争)。这样现有测试不破——`start_battle` 时自动建立战争关系。

**推荐**: `start_battle` 自动 declare_war。理由:
- 现有测试零改动(不需每处加 declare_war)
- start_battle 语义上就隐含"开战"
- 只在脚本想表达"中立国并存"时不调 start_battle 即可

---

## 6. 文件组织

```
src/runtime/
├── entities.rs    ← 改: War 结构 + Country 加 faction 字段
└── world.rs       ← 改: 加 wars 存储 + are_at_war/enemies_of/declare_war/faction_members
src/combat/
├── commands.rs    ← 改: declare_war/add_to_war/white_peace/create_faction/join_faction 命令; start_battle 自动宣战; 敌人判定改造
└── movement.rs    ← 改: 3 处敌人判定改 are_at_war
```

### 改动清单

| 文件 | 改动 |
|---|---|
| `runtime/entities.rs` | War 结构 + Country 加 faction |
| `runtime/world.rs` | wars 存储 + are_at_war/enemies_of/declare_war/faction_members |
| `combat/commands.rs` | 5 个战争命令 + start_battle 自动宣战 + 2 处敌人判定改 are_at_war |
| `combat/movement.rs` | 3 处敌人判定改 are_at_war |

---

## 7. 测试策略

| 测试 | 验证 |
|---|---|
| are_at_war 基础 | declare_war 后两国互为敌人; 无战争时中立 |
| 阵营自动拉入 | A 宣战 B, A 的阵营成员 C 自动在攻方 |
| enemies_of | 三国战争(A+B vs C+D)的敌人列表正确 |
| white_peace | 结束战争后 are_at_war = false |
| 现有战斗测试不破 | start_battle 自动宣战, 现有测试零改动 |
| move_division 不打中立国 | 未宣战的两军在同省不开打 |

---

## 8. 后续扩展(本次预留, 不实现)

| 系统 | 如何接入 |
|---|---|
| wargoal/正当化 | declare_war 前需 wargoal(本次直接允许宣战) |
| 和平会议 | white_peace 扩展为有条件和平(割地/傀儡) |
| 投降 | 战争的某方全部 victory_points 丢失 → 自动 white_peace |
| 停战期 | white_peace 时记录 truce_end_hour(用日期系统) |
| 战争参与度 | War 加 per-tag 参与度(损失/占领), 影响和谈话语权 |

---

## 9. 验收标准

1. `cargo test` 全绿(现有 177 + 新增战争测试)
2. `declare_war` 后 `are_at_war(GER, FRA)` = true
3. 无战争时两国军在同省不开打(中立)
4. `start_battle` 自动宣战(现有测试零改动)
5. 阵营成员自动加入战争
6. `white_peace` 结束战争后 are_at_war = false
7. **后续国策/事件的 at_war_with trigger 能用**(读 are_at_war)
