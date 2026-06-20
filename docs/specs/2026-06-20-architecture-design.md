# HOI4 完整复刻 — 架构设计文档

> **项目代号**: hoi4-clone
> **创建日期**: 2026-06-20
> **状态**: 设计已定稿,待 M1 实现
> **技术栈**: Rust + WASM + TypeScript 前端

---

## 0. 项目目标

完整复刻《钢铁雄心 4》(Hearts of Iron IV) 的**机制层**(战斗/经济/科技/生产/补给/AI/外交/国策/事件),并承载**原版内容**(8631 国策 / 7821 事件 / 13375 省份 / 329 国家),最终以**跨平台应用**(PC + 安卓)形态发布。

**法律边界**: 不复刻 HOI4 的任何素材(贴图/音乐/品牌),只复刻**机制**和**数据结构**(数值与游戏机制不受版权保护)。原版脚本经转译后以自有格式存储。

---

## 1. 设计约束(从实测数据倒推)

| # | 约束 | 依据 |
|---|---|---|
| C1 | **内容与引擎彻底分离** | 原版 130 万行脚本 vs 3 万行机制,内容持续增长 |
| C2 | **必须复刻 effect/trigger DSL** | 8631 个国策无法用硬编码表达 |
| C3 | **事件驱动 + 固定主循环** | 原版靠 on_actions 钩子扩展逻辑 |
| C4 | **支撑 13375 省 × 数千单位实时结算** | 后期不能卡 |
| C5 | **跨平台**(PC/浏览器/安卓) | 最终目标含 APP |

---

## 2. 核心认知:复刻 = 做脚本引擎

```
HOI4 = Clausewitz 脚本引擎 + 130万行用该语言写的内容
完整复刻 = 做两件事:
  (1) 做一个 Clausewitz 风格的脚本运行时
  (2) 把原版 130 万行脚本转译成我们的格式
```

**因此核心交付物是"脚本运行时",不是"游戏系统"。** 战斗/经济等系统都建立在该运行时之上。

---

## 3. 技术栈

| 层 | 技术 | 理由 |
|---|---|---|
| 核心引擎 | **Rust** | 性能跑满 13375 省; 内存安全; AI 写得快 |
| 跨平台编译 | **WASM (wasm-bindgen)** | 同一份代码跑 PC/浏览器/安卓 |
| 桌面壳 | **Tauri** | 比 Electron 轻 10 倍 |
| 移动壳 | **Capacitor** | 把 WASM 包成 APK |
| Web 前端 | **TypeScript + React** | UI 开发快 |
| 地图渲染 | **Canvas/WebGL** | 省份多时需要 GPU |
| 数据格式 | **JSON + MessagePack** | JSON 开发友好, MessagePack 存档紧凑 |

---

## 4. 五层架构

```
第五层: 客户端 (Web/Tauri/Android, 可替换)
第四层: 引擎 API (Rust→WASM: tick/query/command)
第三层: 游戏系统 (combat/production/research/supply/ai/diplomacy...)
第二层: 脚本运行时 (★ 核心: DSL解释器 + EventBus + 主循环 + ECS + 存档)
第一层: 数据层 (HOI4 脚本转译出的 JSON)
```

### 4.1 第一层:数据层(转译产物)

```
game_data/
├── defines.json          ← common/defines/00_defines.lua
├── equipment.json        ← common/units/equipment/*.txt (49文件)
├── technologies.json     ← common/technologies/*.txt (17文件)
├── units.json            ← common/units/*.txt (283文件)
├── buildings.json        ← common/buildings/
├── terrain.json          ← common/terrain/
├── ideas.json            ← common/ideas/*.txt (209文件)
├── focuses/              ← common/national_focus/ (65文件→8631国策)
├── events/               ← events/ (114文件→7821事件)
├── decisions/            ← common/decisions/ (124文件)
├── on_actions.json       ← common/on_actions/ (15文件)
├── history/              ← history/ (1924文件,1936开局)
├── map/
│   ├── provinces.bin     ← map/definition.csv (13375省)
│   └── adjacency.bin     ← map/adjacencies.txt
└── localization/         ← localisation/ (11万词条)
```

转译由 Rust 解析器自动完成(见 5.2)。

### 4.2 第二层:脚本运行时(★ 项目成败关键)

#### 4.2.1 游戏主循环

```rust
fn tick(&mut world) {
    world.hour += 1;
    event_bus.fire("on_hourly", &mut world);
    combat::resolve(&mut world);           // 战斗(每小时)
    movement::update(&mut world);          // 移动
    production::produce(&mut world);       // 工厂产出
    if world.hour % 24 == 0 {
        event_bus.fire("on_daily", &mut world);
        event_bus.fire(&format!("on_daily_{}", world.player.tag), &mut world);
        ai::run_daily(&mut world);
    }
    if world.hour % (24*7) == 0 { event_bus.fire("on_weekly", &mut world); }
    if world.hour % (24*30) == 0 { event_bus.fire("on_monthly", &mut world); }
}
```

#### 4.2.2 DSL 解释器(最关键组件)

支持 HOI4 的嵌套 effect/trigger 块:

```rust
enum Effect {
    Command { name: String, args: Vec<Value> },      // add_stability, add_pp...
    If { cond: Trigger, then: Vec<Effect>, else_: Vec<Effect> },
    ForEach { scope: Scope, filter: Trigger, body: Vec<Effect> },
    Random { table: Vec<(f64, Vec<Effect>)> },       // random_events
    SetVar { scope: Scope, key: String, val: Value },
    ScriptedEffect(String),                          // 调用 scripted_effects
}

enum Trigger {
    HasFlag(String),
    Compare { lhs: Value, op: Op, rhs: Value },
    AnyEntity { kind: EntityKind, filter: Box<Trigger> },
    And(Vec<Trigger>), Or(Vec<Trigger>), Not(Box<Trigger>),
    ScriptedTrigger(String),
}
```

**命令清单**(分批实现):
- M1: 50 个核心命令(变量/条件/算术/scope 跳转)
- M2: +80 个战斗/生产命令
- M3: +100 个国策/事件/决策命令
- M4-M5: +270 个杂项,总计约 500 个(对齐原版 effect+trigger 数量)

#### 4.2.3 Event Bus + on_actions

复刻原版钩子分发:

```rust
bus.on("on_startup", handler);
bus.on("on_daily_GER", handler);          // 按国家分发的钩子
bus.on("on_war_declared", handler);
bus.on("on_focus_completed", handler);
```

#### 4.2.4 实体存储(ECS 式)

```rust
struct World {
    hour: u64,
    provinces: Vec<Province>,                  // 13375, 固定
    states: Vec<State>,
    countries: HashMap<Tag, Country>,
    divisions: HashMap<u64, Division>,
    battles: Vec<Battle>,                      // 仅当前战斗
    production_lines: HashMap<Tag, Vec<Line>>,
    // ... flags/arrays/modifiers
}
```

战斗结算只遍历 `battles`(活跃战斗),不扫全图 → 性能可控。

#### 4.2.5 存档

`World` 整体 `serde` 序列化为 MessagePack。因结构稳定,版本兼容用 schema migration 处理。

### 4.3 第三层:游戏系统

每个系统 = 一组 effect/trigger 命令 + 主循环钩子注册:

```rust
fn register_combat(reg: &mut Registry) {
    reg.effect("add_division_attack", |w, a| { ... });
    reg.trigger("is_in_combat", |w, a| { ... });
    reg.on_hourly(|w| combat::resolve(w));
}
```

**已推导的核心公式**(实现时直接用):
- 战斗: 见 `docs/formulas/land-combat.md` (本会话已推导)
- 生产: BASE_FACTORY_SPEED_MIL=4.5, 效率曲线, 资源惩罚 -5%/缺
- 补给: 首都=5+民工×0.3+军工×0.6+船坞×0.4

### 4.4 第四层:引擎 API

WASM 导出的接口:

```rust
#[wasm_bindgen]
impl Engine {
    pub fn new() -> Engine;
    pub fn load_game(&mut self, data: &str) -> Result<()>;
    pub fn tick(&mut self, hours: u32) -> Result<()>;     // 推进N小时
    pub fn query(&self, q: &str) -> JsValue;              // 查询状态
    pub fn command(&mut self, cmd: &str) -> Result<()>;   // 玩家指令
    pub fn save(&self) -> String;                          // 导出存档
}
```

### 4.5 第五层:客户端

UI 与引擎解耦,通过 query/command 通信。三端共用一份前端代码,只是壳不同。

---

## 5. 关键机制实现规范(基于本会话推导)

### 5.1 陆战(完整公式见 docs/formulas/land-combat.md)

每小时每命中掷骰:
- 组织度: `1d4 × 0.053`
- 强度: `1d2 × 0.060`
- 命中率: 有防御剩 10% / 防御耗尽 40%
- 装甲碾压(我装甲>敌穿甲): +6 组织骰/+2 强度骰
- 穿甲系数表: [1.0, 0.8, 0.65, 0.5]
- 多师分摊: 首要目标 35%, 其余 65% 均分
- 堆叠惩罚: >5 师 -2%/师
- 宽度: 默认 70, 超宽 -1%/%(上限 -33%)

### 5.2 生产

- 基础产出: 工厂 × BASE_FACTORY_SPEED × 效率 × 加成
- 效率: 起始 10%, 上限 50%, 每日 +1%
- 切换产品保留: variant 90%/family 70%/archetype 20%
- 资源: 每缺 1 资源/厂 -5%

### 5.3 战斗目标选择

- 宽度分配: 按宽度入场, 超宽惩罚
- 首要目标: 装甲权重 ×1.2, 软攻击 ×1.0
- DAMAGE_SPLIT_ON_FIRST_TARGET = 0.35

---

## 6. 里程碑路线(6 阶段, 每阶段可测)

| M | 目标 | 周期 | 验收标准 |
|---|---|---|---|
| **M1** | 脚本引擎骨架 | 1-2周 | 解析 1 个国策文件并执行, 打印结果 |
| **M2** | 核心机制 | 2-3周 | 2 师打仗, 工厂产装备, 科技加载 |
| **M3** | 内容转译 | 2-3周 | 1936 开局能启动 |
| **M4** | 国策+事件 | 2-3周 | 德国历史路线走通 |
| **M5** | AI+外交+补给 | 2-3周 | AI 国家能自己打仗 |
| **M6** | 海空战+完善+平衡 | 2-3周 | 后期不卡, 体验完整 |

**M1 是关键风险点** — M1 成功证明整个方案可行。

---

## 7. 风险登记

| 风险 | 概率 | 影响 | 对策 |
|---|---|---|---|
| DSL 需实现 ~500 命令 | 必然 | 高 | 按使用频率排序, 80/20 |
| HOI4 脚本边角语法多 | 高 | 中 | 容错解析, 跳过+日志 |
| 国策/事件复杂依赖 | 中 | 高 | M4 验证主路线 |
| 性能(后期) | 中 | 高 | Rust 兜底, profiling 优化 |
| AI 太蠢 | 高 | 中 | 规则式+难度补偿 |
| 内容转译语义偏差 | 高 | 中 | 用户 playtest 对比 |
| 上下文窗口限制 | 必然 | 中 | 严格分模块分会话 |

---

## 8. 模块独立性原则(便于跨会话开发)

每个模块满足:
- **单一职责**: 一个文件/模块只做一件事
- **明确接口**: 通过注册的 effect/trigger 或主循环钩子通信
- **可独立测试**: 不依赖其他模块即可测
- **可独立描述**: 不读内部即可理解

这保证我能跨会话增量开发,不会因上下文丢失而失控。

---

## 9. 与原版对比

| 维度 | 原版 | 本设计 | 评价 |
|---|---|---|---|
| 内容/引擎分离 | ✅ | ✅ | 一致 |
| 事件钩子 | ✅ on_actions | ✅ event bus | 一致 |
| defines 配置 | ✅ lua | ✅ json | 一致 |
| 主循环 | ✅ hourly | ✅ tick() | 一致 |
| DSL | ✅ effect/trigger | ✅ AST 解释器 | 一致 |
| 实体存储 | C++ 内部 | ECS 式 | 现代化 |
| 跨平台 | ❌ 仅PC | ✅ WASM | **超越** |

---

## 10. 下一步

本 spec 审阅通过后, 进入 writing-plans 流程, 为 **M1(脚本引擎骨架)** 写详细实现计划。
