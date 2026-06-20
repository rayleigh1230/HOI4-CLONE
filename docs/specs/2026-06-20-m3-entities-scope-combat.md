# M3 — 实体存储 + 作用域框架 + 陆战引擎

> **创建日期**: 2026-06-20
> **状态**: 设计已定稿,待实现
> **依据**: `docs/specs/2026-06-20-architecture-design.md` §4.2.4(实体存储) + §5.1(陆战公式) + M2 review 的"M3 阻塞"
> **前置**: M2 (`m2-complete` tag)

---

## 0. 目标

M2 让运行时足够健壮,但 World 仍是扁平 vars/flags,`ForEach` 是占位单次执行。
M3 实现"两个师能打仗":

1. **实体存储** —— Province/Country/Division 的领域结构
2. **作用域框架** —— 枚举栈 + ForEach 真实枚举(M2 review 的核心阻塞)
3. **陆战引擎** —— 用 `docs/formulas/land-combat.md` 的公式实现战斗结算

M3 完成后,能加载两个师配置 → 让它们战斗 → 看到组织度/强度按公式变化。

---

## 1. 实体存储(手写领域结构)

```rust
// src/runtime/entities.rs
pub struct Province { pub id: u32, pub owner: String, pub controller: String, /* terrain 等 */ }
pub struct Country { pub tag: String, pub owned_states: Vec<u32>, pub capital_state: u32, /* ... */ }
pub struct Division {
    pub id: u64,
    pub owner_tag: String,
    pub location_province: u32,
    pub soft_attack: f64, pub hard_attack: f64,
    pub defense: f64, pub breakthrough: f64,
    pub armor: f64, pub piercing: f64,
    pub hardness: f64,
    pub max_org: f64, pub org: f64,
    pub max_strength: f64, pub strength: f64,
    pub combat_width: f64,
}
pub struct Battle {
    pub id: u64,
    pub province: u32,
    pub attackers: Vec<u64>,  // division ids
    pub defenders: Vec<u64>,
}
```

World 扩展(M2 的 vars/flags/event_bus 保留):
```rust
pub struct World {
    // M2 字段...
    pub provinces: HashMap<u32, Province>,
    pub countries: HashMap<String, Country>,
    pub divisions: HashMap<u64, Division>,
    pub battles: Vec<Battle>,
    pub scope_stack: Vec<Scope>,
    pub next_division_id: u64,
}
```

---

## 2. 作用域框架(枚举栈)

```rust
#[derive(Debug, Clone)]
pub enum Scope {
    Root,                       // 顶层
    Country(String),            // tag
    Province(u32),              // province id
    Division(u64),              // division id
    Battle(u64),                // battle id
}
```

**作用域遍历(M3 实现真实枚举)**:
- `every_owned_state` / `all_owned_state`: 遍历当前国家(栈顶 Country)的 owned_states,每省压入 Province scope
- `every_country` / `all_country`: 遍历所有国家
- `random_country`: 随机选一国
- `all_army` / `every_army`: 遍历当前国家的所有师
- `all_enemy_country`: 战时遍历敌国

Interpreter 的 `ForEach` 分支根据 scope 名分发到对应的枚举器,对每个实体:压栈 → 求值 filter(可选) → 执行 body → 出栈。

filter 里的 trigger 可访问栈顶实体(如 `is_owned_and_controlled_by` 读栈顶 Province 的 controller)。

---

## 3. 陆战引擎(`docs/formulas/land-combat.md`)

每小时每场战斗结算,核心公式(已在 docs/formulas 存档):

### 攻击点数
```
软攻击点 = 攻方软攻击 × (1 - 守方硬度)
硬攻击点 = 攻方硬攻击 × 守方硬度
总攻击点 = (软+硬) × Σ修正(地形/计划/将领/补给)
```

### 命中(防御池机制)
```
守方有 defense(守方)/breakthrough(攻方) 池
每发攻击消耗 1 点池:
  池未空: 命中率 10%
  池空:   命中率 40%
```

### 装甲碾压
```
若 攻方装甲 > 守方穿甲: +6 组织度骰, +2 强度骰, 守方对攻方伤害 ×0.5
穿甲系数: 我穿甲/敌装甲 查表 [1.0, 0.8, 0.65, 0.5]
```

### 掷骰伤害
```
组织度伤害/命中 = 1d4 × 0.053
强度伤害/命中   = 1d2 × 0.060
```

### 多师分摊(spec §5.3)
```
首要目标承受 35% 总伤害, 其余 65% 均分其他目标
```

实现为 `combat::resolve(world)`,挂在主循环 `on_hourly`。

---

## 4. 范围边界(YAGNI)

**M3 做**:
- Province/Country/Division/Battle 实体结构 + World 存储
- Scope 枚举 + 作用域栈 + ForEach 真实枚举(every_owned_state/every_country/all_army/random_country)
- 战斗引擎(攻击点/防御池/装甲/掷骰/多师分摊)
- 战斗相关命令(create_division/start_battle/add_soft_attack 等)
- 战斗相关 trigger(is_in_combat/has_more_org_than 等)
- 端到端测试:两师打仗,验证 org/str 按公式变化

**M3 不做**(M4+):
- 完整科技树/装备设计器(M3 用硬编码数值建师)
- 地图文件解析(M3 手动建几个省)
- 移动/战略部署(M3 师固定位置)
- 生产系统(M4)
- AI 决策(M5)
- 补给系统(M5)

---

## 5. 验收标准

- [ ] 能创建省/国家/师,存入 World
- [ ] `every_owned_state` 真实遍历国家拥有的省(测试验证计数)
- [ ] `all_army` 遍历国家的师
- [ ] 两师战斗:每小时按公式扣 org/str,数值符合预期
- [ ] 装甲碾压:装甲>穿甲时伤害显著更高(回归测试)
- [ ] 防御池:守方 defense 高时承伤明显低
- [ ] clippy 0 警告
- [ ] demo:展示两师打仗的完整流程

---

## 6. 下一步

spec 审阅通过 → writing-plans 出实现计划。
