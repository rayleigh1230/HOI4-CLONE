# 数据驱动引擎架构改造 设计文档

> 日期: 2026-06-24
> 状态: 已批准(头脑风暴),待实现
> 关联: `docs/formulas/land-combat.md`(营→师汇总公式,本次代码化)
> 关联: `docs/HANDOFF.md`(项目现状,本次在其基础上新增 data 层)

---

## 0. 背景与目标

### 现状

当前项目是一个**硬编码游戏**而非**数据驱动引擎**:

- `create_division` 命令的师属性要么手填(`soft_attack=21`),要么从 `equipment_data.rs` 里 5 条硬编码 `static` 查表。两者都不是从原版数据文件加载的。
- 虽然 parser 能解析任意 HOI4 脚本(Block 树),但 `ast/lower.rs` 把所有 `key=value` 都当"命令"处理。它不区分"数据定义"(如 `sub_units = { infantry = {...} }`)和"命令执行"(如 `add_stability = 0.05`)。

原版 HOI4 是数据驱动引擎:数据文件定义一切,引擎读取并执行。我们的 parser 已经具备解析能力,缺的是"把数据文件解读成静态定义表"的消费者。

### 目标

引入一个**只读静态定义数据库 `GameData`**,让师从"硬编码 create_division"变成"由模板+营+装备数据汇总计算"。

具体打通这条数据链:

```
原版数据文件 → parser(Block树) → data::loader → GameData(只读定义表)
                                                    ↓
                              create_division 查 GameData → 营汇总公式 → Division
```

### 范围(本次做)

- **装备**:底盘(chassis)+ 模块(module)+ 设计变体的数据结构与加载
- **营**:sub_units 定义的结构与加载;营的战斗属性从 need 装备计算
- **师模板**:division_template 的结构与加载;营→师汇总公式代码化
- **支援连**:与战斗营一起做(结构 + 汇总 + `battalion_mult` 修正)

### 非目标(本次不做)

- **模块化设计师 UI**:玩家在游戏内组合模块设计装备的交互界面(数据结构预留,UI 后续)
- **生产系统**:工厂→IC→装备的真实生产链(装备靠库存/补给,生产系统是后续系统)
- **科技系统**:科技树解锁装备(本次所有 year≤1936 的装备都视为可用)
- **堑壕等后续战斗规则**:支援连的 `entrenchment` 等专属属性暂不实现战斗效果(在 HANDOFF §5 未实现规则表里)
- **空军/海军装备**:飞机模块、船模块、船体(本次只做陆战装备)

---

## 1. 核心设计决策(头脑风暴结论)

| # | 决策 | 选择 |
|---|---|---|
| 1 | 架构方案 | 新增独立 `src/data/` 层(parser 的第二个消费者),与 runtime 平行;combat/runtime 零改动 |
| 2 | Division 结构 | **完全不动**——汇总产出填入现有字段值,战斗系统(resolve/movement/width/recovery/reinforce)零改动 |
| 3 | 装备模型 | 统一模型:所有装备 = 底盘 + 模块组合;历史预置型号 = archetype 的 default_modules;不分裂"直读 vs 算模块"两条路 |
| 4 | 属性汇总公式 | 加法后乘法:`stat = (base + Σ add_stats) × Π(1 + multiply_stats)` |
| 5 | 营→师汇总 | 严格对齐 `docs/formulas/land-combat.md` 第2节:求和类/加权混合(60%平均+40%最高)/按width加权/按权重加权 |
| 6 | 支援连 | 一起做:combat_width=0 走独立 support_width 池;battalion_mult 按 category 扫战斗营应用修正 |
| 7 | 数据来源 | 编译期嵌入(`include_str!`),原版文件拷到 `src/data_raw/`;不引用 steam 路径(可移植) |
| 8 | 加载顺序 | 两遍扫描:第一遍注册名字+原始Block,第二遍解析 parent/archetype 继承链 |
| 9 | 错误处理 | 未知字段静默忽略;引用断裂(模板引用不存在的营)记录警告并跳过该条目,不 panic |
| 10 | GameData 归属 | 进 World(`Arc<GameData>` 共享只读);`World::new()` 自动加载完整 GameData |
| 11 | 旧 create_division | 兼容回退保留:有 `template` 走新路径(数据驱动),有 `battalions` 走旧路径(查表)。新路径不碰旧表,互不干扰 |
| 12 | GameData 缓存 | `std::sync::OnceLock` 缓存加载结果(零外部依赖),所有测试共享一份,只算一次 |

---

## 2. 整体架构与数据流

### 2.1 两阶段:加载阶段 + 运行时阶段

```
┌─────────────────────────────────────────────────────────────┐
│  引擎启动阶段(加载一次, 产出只读 GameData)                  │
│                                                              │
│  src/data_raw/*.txt      现有 parser           data::loader  │
│  (编译期嵌入)    →    (Block 树, 通用)   →   解读为数据定义  │
│  tank_chassis.txt                                   ↓        │
│  infantry.txt                              装进 GameData     │
│  00_tank_modules.txt                       (Arc 包裹)        │
│  history_ger.txt                                             │
│                          (parser 不动)        (新增 src/data/)│
└──────────────────────────────────┬──────────────────────────┘
                                   │ Arc 共享只读
                                   ▼
┌─────────────────────────────────────────────────────────────┐
│  运行时(现有 runtime, 改动小)                               │
│                                                              │
│  历史脚本 → parser → ast → runtime::interp                   │
│    命令执行时:                                              │
│    create_division { owner=GER template="7_inf" loc=1 }     │
│      └→ 从 w.data (Arc<GameData>) 查 template 定义           │
│      └→ 按营+装备汇总公式算出 Division 的 soft_attack 等     │
│      └→ 写入 world.divisions(战斗系统零改动地继续工作)      │
│                                                              │
│  clock.rs 主循环 → combat/* (现有, 零改动)                   │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 三个关键边界

**边界 1:`GameData` 是只读快照**。启动时从文件加载一次,运行时绝不修改。所有战斗/移动/生产逻辑从它"查"定义,不"写"。这避免了可变借用冲突(HANDOFF 反复强调的痛点)。

**边界 2:loader 和 interpreter 是 parser 的两个消费者**。parser 保持通用(它已经能解析任意 Block),但:
- `data::loader` 把 Block 当**数据定义**读(`infantry = { max_strength=25 }` → 注册一条定义)
- `runtime::interp` 把 Block 当**命令**执行(现有逻辑不变)

这是本方案的核心:同一棵 Block 树,两种解读。

**边界 3:`Division` 结构不变**。战斗系统全部依赖 `Division` 的现有字段。改造后 `Division` 的字段一个不少,只是**填充方式变了**:从"硬编码 create_division 手填"变成"查 GameData + 营汇总公式计算"。战斗代码零改动。

### 2.3 数据流示例:一个 7 步师是怎么诞生的

```
加载阶段:
  infantry_equipment 数据 → GameData.equipment["infantry_equipment"]
  infantry 营定义      → GameData.sub_units["infantry"]

运行时建师:
  脚本: create_division = { owner=GER template="7_inf_div" location=1 }
    1. 查 w.data.templates["7_inf_div"] → 营列表 [infantry×7]
    2. 每营查 w.data.sub_units["infantry"] → need=infantry_equipment×100
    3. 聚合: 总 need = 7×100 infantry_equipment
    4. 汇总公式(land-combat.md 第2节):
       soft_attack = Σ(营 soft_attack) = 7 × (装备soft × 100/100) = 7×3 = 21
       defense     = Σ(营 defense)     = 7 × 20 = 140
       armor       = 60%平均 + 40%最高(营里都无装甲 → 0)
       ...
    5. 填入 Division(字段和现在完全一样), 加入 world.divisions
    6. 战斗系统照常工作
```

---

## 3. 模块化装备数据模型

### 3.1 原版装备的三层继承体系

调研原版 `units/equipment/tank_chassis.txt` 确认:

```
① archetype (抽象底盘, is_archetype=yes, 不可生产)
   light_tank_chassis = {
     module_slots = { ... }          ← 定义"有哪些槽位、每个槽位允许哪些模块类别"
     default_modules = { ... }       ← 默认模块组合(预设设计模板)
   }
   ↓ 定义槽位结构 + 预设模块组合

② 具体型号 (继承 archetype, module_slots=inherit, 可生产)
   light_tank_chassis_1 = {
     archetype = light_tank_chassis  ← 指向①
     parent = light_tank_chassis_0   ← 继承链
     module_slots = inherit          ← 槽位结构来自①
     year=1934  armor_value=15  build_cost_ic=2.35   ← 预填数值(缓存)
     derived_variant_name = light_tank_equipment_1   ← 装备真名
     upgrades = { tank_nsb_engine_upgrade ... }      ← 标记哪些模块可换
   }

③ 装备变体 (营 need 里引用的名字)
   light_tank_equipment_1 = ② 的产出
```

### 3.2 统一视角:历史型号与模块化设计是一回事

**关键洞察**:历史预置型号和模块化设计**不是两种东西,而是同一条流水线的不同入口**。

- **archetype** 的 `default_modules` = 一个"预设设计模板"
- **历史型号** = 拿 archetype 的默认模块组合(可能覆盖几个模块)→ 算出最终数值。预填的 `armor_value=15` 只是缓存,逻辑上等价于"用某组模块算出来的值"
- **玩家自建设计**(未来)= 同样的底盘,玩家手动换模块 → 同样算出最终数值

三者统一为:**装备变体 = 底盘 + {槽位→模块} 的映射**。

```
装备变体 = 底盘 + {槽位 → 模块} 的映射
  ├─ 历史型号: 用 archetype 的 default_modules(可经 upgrades 覆盖)→ 预填数值是缓存
  └─ 玩家设计: 玩家选的模块 → 实时算
```

因此代码只有**一条汇总路径**,不分裂"直读数值 vs 算模块"两套逻辑。历史型号也走统一公式(用 default_modules)。

### 3.3 属性汇总公式(精确对齐原版)

原版模块用两种方式改属性,顺序固定**先加后乘**:

```
第1步 — 加法汇总(底盘基础 + 各槽位模块的 add_stats):
  raw_stat = chassis_base_stat + Σ module.add_stats[stat]

第2步 — 乘法修正(所有 multiply_stats 叠乘):
  final_stat = raw_stat × Π (1 + module.multiply_stats[stat])
```

**例**:轻型坦克 armor_value
- 底盘基础 armor_value = 10
- 装甲模块 welded_armor: multiply_stats armor_value = +0.3
- 炮塔模块 light_turret_1: multiply_stats armor_value = +0.1
- raw = 10 + 0 + 0 = 10
- final = 10 × (1+0.3) × (1+0.1) = 10 × 1.3 × 1.1 = 14.3

### 3.4 Rust 数据结构

```rust
// src/data/equipment.rs

/// 装备属性集合(战斗相关字段, 从 add_stats/multiply_stats 提取)
#[derive(Debug, Clone, Default)]
pub struct EquipStats {
    pub soft_attack: f64,
    pub hard_attack: f64,
    pub defense: f64,
    pub breakthrough: f64,
    pub armor: f64,        // 原版 armor_value
    pub piercing: f64,     // 原版 ap_attack
    pub hardness: f64,
    pub build_cost_ic: f64,
    pub maximum_speed: f64,
    pub reliability: f64,
}

/// 底盘定义(archetype):槽位结构 + 默认模块
/// 同时承载"整件装备"(slots 为空时, 如 infantry_equipment)和"模块化底盘"(有槽位, 如坦克)
#[derive(Debug, Clone)]
pub struct ChassisDef {
    pub name: String,              // "light_tank_chassis"
    pub equip_type: String,        // "armor" / "infantry" / "artillery"
    pub year: u32,
    pub is_archetype: bool,        // archetype 不可生产
    pub base_stats: EquipStats,    // 底盘自带基础属性(整件装备的主要来源)
    pub slots: Vec<SlotDef>,       // 槽位定义(有序; 整件装备为空)
    pub default_modules: HashMap<String, String>, // slot_name → module_name(预设组合)
}

#[derive(Debug, Clone)]
pub struct SlotDef {
    pub name: String,                        // "turret_type_slot"
    pub required: bool,
    pub allowed_categories: Vec<String>,     // ["tank_light_turret_type"]
}

/// 模块定义(原版 00_tank_modules.txt 里的每个条目)
#[derive(Debug, Clone)]
pub struct ModuleDef {
    pub name: String,            // "tank_welded_armor"
    pub category: String,        // "tank_armor_type"
    pub add_stats: EquipStats,
    pub multiply_stats: EquipStats,
}

/// 可生产装备 = 底盘 + 各槽位选定的模块(历史型号和玩家设计统一)
/// 一件挂在营 need 里的装备
#[derive(Debug, Clone)]
pub struct EquipmentDef {
    pub name: String,              // "light_tank_equipment_1"(derived_variant_name)
    pub chassis: String,           // 指向 ChassisDef.name("light_tank_chassis")
    pub year: u32,
    pub equip_type: String,        // "armor" / "infantry" / "artillery"
    pub stats: EquipStats,         // 最终属性(按统一公式算, 加载时缓存)
}
```

### 3.5 关键设计要点

1. **整件装备和模块化装备走同一套模型**。`ChassisDef` 的 `slots` 为空就退化成整件装备(如 `infantry_equipment`)。一套代码同时处理两者。
2. **`stats` 在加载时算好缓存**。设计变体的属性在启动加载时按公式算好,建师时直接读,不重算。因为设计是静态的(一旦定好,模块组合不变)。
3. **archetype 标记排除**。`is_archetype=true` 的底盘不可生产,加载时打标,营的 need 不会引用它。

---

## 4. 营 → 师的汇总(模板系统)

### 4.1 完整数据链

```
① EquipmentDef      装备(底盘+模块组合, 算好 stats)
       ↑ need 引用
② SubUnitDef        营定义(units/*.txt)
     │                结构属性: hp/org/width/manpower
     │                战斗属性: 来自它 need 的装备 × 件数比例
       ↑ regiments/support 引用
③ DivisionTemplate  师模板(history/countries/*.txt)
     │                regiments: [战斗营]   support: [支援连]
       ↓ 汇总公式(land-combat.md 第2节)
④ Division          (现有结构, 字段完全匹配, 战斗系统零改动)
```

### 4.2 营的属性来源(两处)

- **sub_unit 定义**(`units/*.txt`):HP/org/width/manpower 等结构属性 + `need`(消耗哪些装备)
- **装备定义**:soft_attack/defense/armor 等战斗属性。营的战斗力 = 它 need 的装备贡献

一个营的 `soft_attack` = 它 `need` 的装备的 `soft_attack` × (需求件数 / 100):

```
# need = { infantry_equipment = 100 } 表示满编要 100 件
battalion.soft_attack = Σ_eq (equip[eq].soft_attack × need_qty[eq] / 100.0)
# 例: infantry 营 need infantry_equipment×100, 装备 soft=3 → 营 soft = 3×100/100 = 3
#     7 营 → 师 soft = 7×3 = 21  ✓(与现有 equipment_data.rs 注释一致)
```

### 4.3 营 → 师汇总公式(严格对齐 land-combat.md 第2节)

每种属性有不同的汇总规则:

```
// 求和类(直接相加):
soft_attack   = Σ battalion.soft_attack
hard_attack   = Σ battalion.hard_attack
defense       = Σ battalion.defense
breakthrough  = Σ battalion.breakthrough
combat_width  = Σ battalion.combat_width
max_strength  = Σ battalion.max_strength     // HP
manpower      = Σ battalion.manpower

// 加权混合(60%平均 + 40%最高) — 装甲/穿甲特殊:
armor    = 0.6 × (Σ armor / 营数) + 0.4 × max(各营 armor)
piercing = 0.6 × (Σ piercing / 营数) + 0.4 × max(各营 piercing)

// 加权平均(按 combat_width) — 硬度:
hardness = Σ(battalion.hardness × cw) / Σ(cw)

// 加权平均(按权重, 支援连权重=1) — 组织度:
org      = Σ(battalion.org × w) / Σ(w)
```

### 4.4 支援连的特殊处理

调研 `units/engineer.txt` 等,确认支援连与战斗营的差异及处理方式:

| 支援连特性 | 原版来源 | 本次处理方式(师属性汇总层面) |
|---|---|---|
| `combat_width = 0` | 不占主力宽度 | 求和进师 combat_width 时贡献 0(不增加师的总宽度) |
| `max_strength` 小(2 vs 25) | 结构定义 | 正常求和进师 HP |
| 自身战斗属性(soft_attack 等) | need 装备 | 正常求和(同战斗营) |
| `battalion_mult` | 给其它营加成 | 汇总后,按 category 扫战斗营应用修正(见下) |
| `entrenchment` 等专属属性 | 结构定义 | 本次战斗效果不实现(堑壕系统在 HANDOFF §5 未实现规则表) |

> **范围说明**:原版还有 `combat_support_width`(每场战斗的支援连宽度上限,来自地形定义,如森林=30)。这是**战斗级**机制(决定一场战斗能容纳多少支援连),需要地形数据。本次 spec 只做**师属性汇总**——在师层面,支援连的 `combat_width=0` 不影响师的总宽度;战斗级的 support_width 上限留待地形系统(后续)。

**`battalion_mult` 机制**(工兵/维修/宪兵等):

```hoi4
# 原版: 工兵连给"轻步兵类营"加成
battalion_mult = {
    category = category_light_infantry   ← 作用于哪些营(按 category 匹配)
    entrenchment = 0.20                   ← 加多少
    add = yes                             ← yes=加法, 缺省=乘法
}
```

处理流程:汇总战斗营属性后,遍历每个支援连的 `battalion_mult`,按 `category` 匹配战斗营,用 `add=yes`(加法)或乘法(`default`)修正它们的属性。

```rust
// 汇总流程(伪代码)
let mut stats = aggregate_regiments(&regiments);  // 战斗营汇总(§4.3)
for support in &template.support {
    // 支援连自身属性求和进 stats
    add_to_stats(&mut stats, support);
    // battalion_mult: 按 category 扫战斗营应用修正
    for mult in &support.battalion_mults {
        apply_battalion_mult(&mut stats, mult, &regiments);
    }
}
```

### 4.5 Rust 数据结构

```rust
// src/data/subunit.rs

/// 营定义(原版 sub_units 里的一个条目)
#[derive(Debug, Clone)]
pub struct SubUnitDef {
    pub name: String,           // "infantry" / "medium_armor" / "engineer"
    pub group: String,          // "infantry" / "armor" / "support"(分类)
    pub categories: Vec<String>,// ["category_light_infantry", ...](battalion_mult 匹配用)
    pub combat_width: f64,
    pub max_strength: f64,      // HP
    pub max_organisation: f64,
    pub default_morale: f64,    // org 恢复率
    pub manpower: f64,
    /// 满编需求: equipment_name → 件数
    pub need: HashMap<String, f64>,  // {"infantry_equipment": 100}
    /// (可选)支援连对其它营的修正
    pub battalion_mults: Vec<BattalionMult>,
}

/// 支援连的 battalion_mult(给匹配 category 的营加成)
#[derive(Debug, Clone)]
pub struct BattalionMult {
    pub category: String,   // "category_light_infantry"(匹配营的 categories)
    pub stat: String,       // "entrenchment" / "max_strength" / ...
    pub value: f64,         // 0.20
    pub add: bool,          // true=加法, false=乘法
}

impl SubUnitDef {
    /// 营的战斗属性(从它 need 的装备算)
    ///
    /// 注意区分两类属性的计算方式:
    /// - 攻/防/突(soft_attack/hard_attack/defense/breakthrough):按件数比例
    ///   `Σ_eq (equip.stats[stat] × need_qty / 100)`。件数越多威力越大。
    /// - 装甲/穿甲/硬度(armor/piercing/hardness):取装备值,不×件数。
    ///   因为这三者是"营的固有等级",不因装备数量叠加。
    ///   依据:原版 defines 第1052行 `ARMOR_VS_AVERAGE=0.4`,注释"weight in highest armor & pen
    ///   vs the division average"——确认 armor 和 piercing 都在师层做 60%平均+40%最高加权混合(§4.3)。
    ///   hardness 同理按 combat_width 加权平均。
    pub fn combat_stats(&self, lookup: &dyn Fn(&str) -> Option<&EquipmentDef>) -> EquipStats {
        let mut s = EquipStats::default();
        for (eq_name, qty) in &self.need {
            if let Some(eq) = lookup(eq_name) {
                let factor = qty / 100.0;
                // 按件数比例的属性(攻/防/突)
                s.soft_attack  += eq.stats.soft_attack  * factor;
                s.hard_attack  += eq.stats.hard_attack  * factor;
                s.defense      += eq.stats.defense      * factor;
                s.breakthrough += eq.stats.breakthrough * factor;
                // 不×件数的属性(营固有等级, 师层加权混合)
                s.armor        += eq.stats.armor;
                s.piercing     += eq.stats.piercing;
                s.hardness     += eq.stats.hardness;
            }
        }
        s
    }
}
```

```rust
// src/data/template.rs

/// 师模板(原版 division_template)
#[derive(Debug, Clone)]
pub struct DivisionTemplate {
    pub name: String,                  // "Pashtun Levy"
    pub regiments: Vec<RegimentEntry>, // 战斗营列表(有序)
    pub support: Vec<RegimentEntry>,   // 支援连列表
}

#[derive(Debug, Clone)]
pub struct RegimentEntry {
    pub sub_unit: String,  // "infantry"(指向 SubUnitDef)
    pub x: u32,            // 槽位坐标(UI 用, 战斗不读)
    pub y: u32,
}

impl DivisionTemplate {
    /// 汇总成 Division 所需的属性(填入现有 Division 字段)
    pub fn to_division_stats(&self, data: &GameData) -> DivisionStats {
        // 步骤1: 查每个战斗营的 SubUnitDef + combat_stats(need 装备算出)
        let regiments: Vec<(&SubUnitDef, EquipStats)> = self.regiments.iter()
            .filter_map(|r| data.sub_units.get(&r.sub_unit)
                .map(|su| (su, su.combat_stats(&|n| data.equipment.get(n)))))
            .collect();
        // 步骤2: 战斗营按 §4.3 公式汇总(求和/加权混合60-40/按width加权/按权重加权)
        let mut stats = aggregate_regiments(&regiments);
        // 步骤3: 支援连逐个处理 —— 自身属性求和 + battalion_mult 按 category 应用
        for support_entry in &self.support {
            if let Some(su) = data.sub_units.get(&support_entry.sub_unit) {
                let sup_stats = su.combat_stats(&|n| data.equipment.get(n));
                add_support_to_stats(&mut stats, su, &sup_stats);
                for mult in &su.battalion_mults {
                    apply_battalion_mult(&mut stats, mult, &regiments);
                }
            }
        }
        // 步骤4: 聚合 equipment_need / manpower_need(Σ各营 need + manpower)
        stats.equipment_need = aggregate_equipment_need(&self.regiments, &self.support, data);
        stats.manpower_need = aggregate_manpower(&self.regiments, &self.support, data);
        stats
    }
}

/// 汇总产出的中间结构(字段与现有 Division 的属性字段一一对应)
/// create_division 命令拿它填充 Division
pub struct DivisionStats {
    pub soft_attack: f64,
    pub hard_attack: f64,
    pub defense: f64,
    pub breakthrough: f64,
    pub armor: f64,
    pub piercing: f64,
    pub hardness: f64,
    pub combat_width: f64,
    pub max_org: f64,
    pub max_strength: f64,
    pub manpower_need: f64,
    pub equipment_need: HashMap<String, f64>,
}
```

### 4.6 关键设计要点

1. **`Division` 结构完全不动**。汇总产出 `DivisionStats` 中间结构,`create_division` 命令拿它填充 `Division`。战斗系统零改动。
2. **支援连和战斗营都实现**。支援连的 `combat_width=0`、`battalion_mult`、自身属性求和全部处理。唯一延后的是 `entrenchment` 等专属属性的战斗效果(后续堑壕系统)。
3. **`land-combat.md` 公式代码化**。第2节(加权混合 60/40、按 width 加权等)直接落地,不是新设计,是把已验证的公式代码化。

---

## 5. Loader 与 GameData 组装

### 5.1 数据加载顺序(依赖链)

```
模块(modules)     ← 无依赖, 最先加载
  ↓
底盘(chassis)     ← 依赖模块(default_modules 引用模块名)
  ↓
装备变体(equipment) ← 依赖底盘(derived_variant_name 指向 chassis 型号)
  ↓
营(sub_units)      ← 依赖装备(need 引用装备名)
  ↓
模板(template)     ← 依赖营(regiments 引用营名)
```

乱序加载会导致引用断裂(如先加载模板再加载营,模板引用的营名找不到)。loader 必须分阶段、按依赖顺序。

### 5.2 加载入口(编译期嵌入)

```rust
// src/data/loader.rs

/// 引擎启动时调用一次, 产出只读 GameData
pub fn load_all() -> GameData {
    let mut data = GameData::default();

    // 阶段1: 模块(无依赖)
    load_modules(&mut data, include_str!("../../data_raw/modules/00_tank_modules.txt"));

    // 阶段2: 底盘(依赖模块)
    load_chassis(&mut data, include_str!("../../data_raw/tank_chassis.txt"));
    // (其它底盘文件视 data_raw 实际拷贝情况加载)

    // 阶段3: 装备变体(在阶段2解析 chassis 型号时产出, 这里做索引整理)
    index_equipment(&mut data);

    // 阶段4: 营定义(依赖装备)
    load_sub_units(&mut data, include_str!("../../data_raw/units_infantry.txt"));
    // (其它营文件)

    // 阶段5: 模板(依赖营)
    load_templates(&mut data, include_str!("../../data_raw/history_ger.txt"));

    data
}
```

### 5.3 两遍扫描解决继承

原版数据有继承链(`chassis_1` 的 `parent = chassis_0`、`archetype = light_tank_chassis`)。两遍扫描:

```rust
fn load_chassis(data: &mut GameData, src: &str) {
    let block = parser::parse(src).expect("chassis 文件解析失败");
    // 第一遍: 注册所有 chassis 名字 + 原始 Block
    let raw: HashMap<String, Block> = extract_named_entries(&block);
    // 第二遍: 按 parent/archetype 链解析继承, 算最终属性 + 缓存 stats
    resolve_inheritance(data, &raw);
}
```

### 5.4 错误处理:鲁棒加载

- **未知字段静默忽略**。原版文件有大量我们暂不处理的字段(DLC 条件、UI 图标、音效),loader 不报错跳过。
- **引用断裂记录警告并跳过该条目**。模板引用了不存在的营 → 警告 + 跳过这个营(模板仍可用,只是少一个营)。不让单个缺失项让整个 GameData 加载失败。
- **不 panic**。加载阶段的问题用警告日志体现,尽力产出可用的 GameData。

```rust
match data.sub_units.get(&regiment.sub_unit) {
    Some(su) => regiments.push(su.clone()),
    None => eprintln!("[data] 警告: 模板 {} 引用未知营 {}", name, regiment.sub_unit),
}
```

### 5.5 GameData 结构与 World 的关系

```rust
// src/data/mod.rs

/// 只读静态定义数据库(启动加载, 运行时不改)
#[derive(Debug, Clone, Default)]
pub struct GameData {
    pub modules: HashMap<String, ModuleDef>,
    pub chassis: HashMap<String, ChassisDef>,
    pub equipment: HashMap<String, EquipmentDef>,
    pub sub_units: HashMap<String, SubUnitDef>,
    pub templates: HashMap<String, DivisionTemplate>,
    /// 开局年份(本次固定 1936; 科技锁定装备的预留)
    pub start_year: u32,
}
```

**World 持有 `Arc<GameData>`(共享只读)**:

```rust
pub struct World {
    pub data: std::sync::Arc<GameData>,
    // ... 现有字段不变
    pub divisions: HashMap<u64, Division>,
    pub provinces: HashMap<u32, Province>,
    pub countries: HashMap<String, Country>,
    pub battles: Vec<Battle>,
    // ...
}

impl World {
    /// 生产用: 挂载完整 GameData
    pub fn new() -> World {
        World {
            data: cached_game_data(),  // OnceLock 缓存, 所有测试共享一份
            // ... 现有字段初始化
        }
    }
}
```

### 5.6 Arc<GameData> 的理由与代价

**为什么 Arc(而非 `&GameData`)**:

1. **避开借用检查冲突**。命令闭包里 `w.data`(`&GameData`)与 `w.divisions.insert`(`&mut`)会冲突——`w.data` 借了 `w`(不可变),`w.divisions` 又要 `&mut w`。Arc 把 data 的所有权独立出来,不借 w。
2. **无生命周期参数**。若用 `&GameData`,`World<'a>` 的 `'a` 会传染到 `Registry<'a>`、`Interpreter<'a>`、`GameClock<'a>`……整个 runtime 层签名都要改。Arc 把这个污染挡死。
3. **clone 廉价**。`Arc::clone` 只增引用计数(纳秒级),契合"只读快照"语义。

**已知代价(本次接受)**:

1. **数据加载完锁死,无法热更新**。Arc 共享所有权,运行时不能替换 GameData。换 mod / 切剧本 / 动态加 DLC——这些原版支持的场景本次不支持(单剧本自用场景无关紧要)。
2. **不能单点热修**。调试时发现某条数据算错,不能单点改了看效果,要么改源文件重新编译,要么整体 `Arc::new` 重建。
3. **clone 语义易混淆**。`Arc::clone`(增计数)和 `struct.clone`(深拷贝)写法一样,语义不同。需约定代码风格避免混淆。
4. **测试构造稍重**。`World::new()` 内部加载 GameData,若每次测试都重算会拖慢。用 `OnceLock` 缓存解决。

### 5.7 测试迁移影响(最小化)

`World::new()` 内部自动加载完整 GameData,无参签名不变:

- **现有 118 个测试零改动**。绝大多数测试不调 `create_division`(测的是 resolve/movement/width/recovery,用硬编码 Division 直接塞进 world)。它们调用 `World::new()`,GameData 被加载但没人用,无影响。
- **少数用 `create_division` 的测试走旧 `battalions` 路径**(兼容回退),不碰 GameData,无改动。
- **新增的数据驱动测试用 `template` 参数**,依赖 GameData 里的真实模板。这些是全新测试,不是迁移。

**GameData 加载缓存(零外部依赖)**:

```rust
use std::sync::OnceLock;

static GAME_DATA: OnceLock<GameData> = OnceLock::new();

fn cached_game_data() -> std::sync::Arc<GameData> {
    std::sync::Arc::new(GAME_DATA.get_or_init(|| crate::data::loader::load_all()).clone())
}
```

所有测试共享同一份 GameData(只算一次),`OnceLock` 是 std 稳定 API(1.70+),不算外部依赖。

---

## 6. create_division 命令改造

### 6.1 双路径分发(旧路径隔离保留)

> 以下伪代码展示分发逻辑骨架。`build_division_from_stats` / `build_legacy_battalions` / `build_hardcoded_defaults` 是提取出来的辅助函数,具体实现细节见实现计划阶段;这里只明确"按参数存在性分发到哪条路径"。

```rust
// create_division 命令的分发(伪代码骨架)
reg.register("create_division", |w, p| {
    let owner = np(p, "create_division", "owner")?.as_str()...;
    let loc = num_of(np(p, "create_division", "location")?)? as u32;

    let division = if let Some(tmpl_name) = ParamGet::get(p, "template").and_then(Arg::as_str) {
        // 新路径: 数据驱动汇总(查 GameData)
        let stats = w.data.templates.get(tmpl_name)
            .ok_or_else(|| CmdError::RuntimeError(format!("未知模板: {tmpl_name}")))?
            .to_division_stats(&w.data);
        build_division_from_stats(owner, loc, stats)
    } else if let Some(bn) = opt_num("battalions") {
        // 旧路径: 查表(equipment_data.rs 硬编码, 原样不动)
        build_legacy_battalions(bn, eq_name, owner, loc)
    } else {
        // 兼容: 手填默认值(原样不动)
        build_hardcoded_defaults(owner, loc)
    };
    w.add_division(division);
    Ok(())
});
```

### 6.2 新旧路径隔离

- **新路径(`build_division_from_stats`)**是纯新增代码,不调用、不依赖 `equipment_data.rs` 硬编码表。
- **旧路径(`build_legacy_battalions`)**原样不动,走 `equipment_data.rs` 查表。
- 两套路径互不干扰:新测试用 `template` 验证数据驱动汇总;旧测试用 `battalions` 走旧路径行为不变。
- `equipment_data.rs` 后续是否清理,等新路径稳定、旧脚本迁移完后另行决定,不是本次范围。

---

## 7. 文件组织

### 7.1 新增模块结构

```
src/
├── data/                          ← 新增: 数据驱动层
│   ├── mod.rs                     GameData 结构 + cached_game_data()
│   ├── equipment.rs               EquipStats / ChassisDef / SlotDef / ModuleDef / EquipmentDef
│   ├── subunit.rs                 SubUnitDef / BattalionMult + combat_stats()
│   ├── template.rs                DivisionTemplate / RegimentEntry / DivisionStats + to_division_stats()
│   └── loader.rs                  load_all() + 各阶段 load_* 函数 + 两遍扫描
├── data_raw/                      ← 新增: 原版数据文件拷贝(编译期嵌入)
│   ├── modules/
│   │   └── 00_tank_modules.txt
│   ├── tank_chassis.txt
│   ├── units_infantry.txt
│   ├── units_artillery.txt
│   ├── ... (1936 开局实际用到的装备/营文件子集)
│   └── history_ger.txt
├── parser/                        ← 不动
├── ast/                           ← 不动
├── runtime/                       ← 改动: World 加 data 字段 + new() 加载 GameData
├── combat/                        ← 不动(零改动)
│   └── commands.rs                ← 改动: create_division 加 template 路径(旧路径隔离保留)
└── ...
```

### 7.2 原版文件拷贝策略

- 原版文件**拷贝**进 `src/data_raw/`,不直接引用 `G:\steam\...`(那个路径只在本机有,不可移植)。
- **拷贝 1936 开局实际用到的子集**(非全量 43+41+329 文件):步兵/炮/轻中重坦克的底盘+模块+装备,常见营定义,几个典型国家的师模板。精简子集保证编译速度和仓库体积。
- 数据文件是 Paradox 数据,自用/分享朋友无版权问题。
- **Git 提交决策**:精简子集(预计几十个文件、单个最大约 1500 行)提交进仓库,保证项目可独立编译、他人 clone 即可构建。`include_str!` 要求文件在编译期存在,故必须随源码一起。

### 7.3 对现有代码的改动清单

| 文件 | 改动 | 性质 |
|---|---|---|
| `src/data/*` | 全新模块 | 新增 |
| `src/data_raw/*` | 全新目录 | 新增 |
| `src/runtime/world.rs` | World 加 `data: Arc<GameData>` 字段 + `new()` 加载 | 小改 |
| `src/runtime/mod.rs` | re-export data 模块 | 小改 |
| `src/combat/commands.rs` | `create_division` 加 template 分支(旧路径保留) | 小改 |
| `src/lib.rs` | 声明 data 模块 | 小改 |
| `src/parser/*` | 不动 | 零改动 |
| `src/ast/*` | 不动 | 零改动 |
| `src/combat/resolve.rs` | 不动 | 零改动 |
| `src/combat/movement.rs` | 不动 | 零改动 |
| `src/combat/width.rs` | 不动 | 零改动 |
| `src/combat/recovery.rs` | 不动 | 零改动 |
| `src/combat/reinforce.rs` | 不动 | 零改动 |
| `src/combat/equipment_data.rs` | 不动(旧路径仍用) | 零改动 |

---

## 8. 测试策略

### 8.1 新增测试(数据驱动层)

| 测试组 | 验证内容 |
|---|---|
| `data::equipment` 模块属性汇总 | `add_stats` + `multiply_stats` 公式正确性;加法后乘法顺序 |
| `data::subunit` 营属性计算 | 营的 combat_stats = need 装备 × 件数/100 |
| `data::template` 营→师汇总 | 各属性汇总公式(求和/加权混合60-40/按width加权/按权重加权) |
| `data::template` 支援连 | combat_width=0;battalion_mult 按 category 应用;支援连自身属性求和 |
| `data::loader` 真实数据加载 | 加载原版文件子集,断言 infantry_equipment/7步师等已知数值 |
| `create_division` template 路径 | 端到端:脚本调 create_division{template=...},产出的 Division 字段正确 |

### 8.2 现有测试不变

现有 118 个测试全部继续通过(详见 §5.7):
- 战斗系统测试(不调 create_division):零改动
- 用 create_division{battalions=...} 的测试:走旧路径,零改动

---

## 9. 后续扩展(本次预留,不实现)

| 扩展 | 本次如何预留 |
|---|---|
| 模块化设计师 UI | 数据结构(ChassisDef/ModuleDef/slots)已建;UI 后续做 |
| 生产系统 | EquipmentDef.build_cost_ic 字段已有;工厂→IC→装备链后续 |
| 科技系统 | EquipmentDef.year 字段已有;科技锁定装备后续 |
| 玩家自建设计 | EquipmentDef = 底盘+module_choices 模型统一;玩家选模块就是改 module_choices |
| buildings/terrain 等 | data 层架构成立,新增 loader 不动现有结构 |

---

## 10. 验收标准

1. `cargo test` 全部通过(现有 118 + 新增数据驱动测试)
2. `create_division { template=... }` 能从 GameData 查模板,产出属性正确的 Division
3. 一个 7 步师的 soft_attack/defense/hp 等数值与原版计算一致(与 equipment_data.rs 注释记录的 21/140/175 等吻合)
4. 支援连(工兵/炮兵支援等)能正确加载并参与汇总
5. 装备模块化数据结构能加载原版 tank_chassis.txt + 00_tank_modules.txt
6. 现有所有战斗测试零改动通过
7. `World::new()` 签名不变(无参),现有调用方零改动
