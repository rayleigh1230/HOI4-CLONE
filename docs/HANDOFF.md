# hoi4-clone 项目交接文档

> **用途**: 在新会话中继续开发。读本文件 + 列出的关键文件即可接上。
> **更新**: 2026-06-25(demo 改造后修复 — canvas 脏标记崩溃/路径级绑定/底栏时间控制/overlay 分层/装备补给修正; 引入 Playwright 真机验证 13/13)

---

## 0. 项目概况

完整复刻 HOI4 核心机制的自制游戏, 分享给朋友玩。
- **技术栈**: Rust → WASM + 单文件 HTML/JS 前端
- **位置**: `G:\projects\hoi4-clone\`
- **运行**: `cargo test`(测试) / `cargo run --bin hoi4_demo`(CLI) / 浏览器 `http://127.0.0.1:8765`(UI demo)
- **工具链**: `stable-x86_64-pc-windows-gnu`(rustup override 绑定)
- **规模**: ~7400 行 Rust + UI + 原版数据, **180 测试**
- **分支**: `feat/data-driven-engine`(37 个提交, 5 个基础构造里程碑)

---

## 1. 已完成的里程碑

| M | 内容 | 测试 |
|---|---|---|
| M1 | 脚本引擎(parser/AST/interpreter) | 19 |
| M2 | 运行时重构(Registry/Result/主循环) | 25 |
| M3 | 实体+作用域+陆战伤害 | 38 |
| M4a | 装备系统(库存/消耗/增援) | 42 |
| 陆战循环 | 四量模型+撤退+歼灭+行军+包围+宽度+预备队+带溃 | 73 |
| 多战场+指令 | 多战场伤害累积+支援攻击+停止+防守撤退+预备队时机 | 99 |
| 状态机重构 | OrderState enum 替代 7 扁平字段 + 4 条规则对齐 + 瞬移根治 | 101 |
| 多段路径行军 | 自动寻路 + 航点规划 + 支援攻击邻接收敛 + 路径失效应对 | 118 |
| **数据驱动引擎** | GameData 只读层 + 模块化装备 + 营→师汇总 + 原版数据加载 | 147 |
| **modifier 层** | 陆战结算统一修正接口(CombatContext快照 + _factor推导op + 三结算点口子) | 164 |
| **State 概念** | 省份上级容器(归属从State派生 + 法理vs控制 + state_loader) | 168 |
| **日期系统** | GameDate 精确公历(闰年/月份天数) + World.date()派生 + clock月切换修正 | 177 |
| **战争状态** | War关系(are_at_war判定) + 阵营自动拉入 + 敌人判定改造 | 180 |
| **demo 彻底改造** | 地图全屏+浮层/绑定式数据流/触屏/模板引用/ change_template/ 6图层Canvas / ES Module 四层架构 | 122+UI |
| **demo 改造后修复+验证** | 见下方"demo 改造后修复"小节(9 项修复 + Playwright 真机验证 13/13) | 122+UI |

### demo 改造后修复(2026-06-25, 对齐 demo-overhaul spec/plan)

改造完成后 demo 一启动即崩(主因 #1), 经系统性排查定位 9 个问题, 全部修复并以 Playwright 真机验证 13/13 通过。每项均标注对应的 spec/plan 条目, 便于新故障往回追溯。

| # | 问题(根因) | 修复 | 对齐 spec/plan |
|---|---|---|---|
| 1 | `canvas.js` 的 `fullRedraw` 变量被使用但**从未声明**(3 处), ES module 严格模式下 `init()→resize()` 抛 `ReferenceError`, 中断整个 `main()` → 空白无交互(此前 6 个 fix commit 的真根因) | 加 `let fullRedraw = true;`; resize 防御性 `Math.max(1, ...)` | spec §6.2(render 用 `layer.dirty \|\| fullRedraw`) |
| 2 | 坐标系: `provincePos` 用视口 W×H 当世界坐标, 命中检测用 `innerWidth/Height` | 确认 canvas `inset:0` 全屏, 两者当前相等; Playwright 验证点击命中正常(暂无需改, 已留注) | spec §6.2(相机统一坐标) |
| 3 | `#bottombar` HTML/CSS 存在但**无 JS 填充**=死元素; 时间按钮被塞进 topbar 违反 spec | 新增 `ui/bottombar.js`, 时间控制移入底栏; topbar 只留日期+系统按钮+切控制权 | spec §7.1(时间放底部命令栏)/plan Task 13 |
| 4 | `store.js` 是**全量通知**(spec 声称路径级脏标记但没实现), 每 tick 所有订阅者全跑 | 重写 store: `setState` 做 `diffKeys`, `subscribeKeys([key])` 仅声明 key 变化才通知 | spec §3.3(路径级脏标记) |
| 5 | `bindList` 每 tick 全量重建列表 → 用户正选的 `<select>` 选中态被刷掉 | `bindList`/`bindText` 改用 `subscribeKeys`, 只在数据真变时重建 | spec §3.3 + §3.4(换模板数据流) |
| 6 | `layerOverlay.js` 是空占位(`// xxx`), 选中高亮被塞在 layerProvince 里违反分层 | 选中高亮逻辑移到 layerOverlay(金色环+标签); layerProvince 只画基础省份 | spec §6.1(overlay 负责选中/tooltip) |
| 7 | `index.html` 无 `#log` 元素 → `main.js:log()`/orderMenu 静默无反馈 | index.html 加 `#log`; CSS 加右上角浮层样式 | plan Task 13(log 调试用) |
| 8 | `engine_supply` 误补 `medium_tank`(无营引用), 装甲师 light_armor 营真正 need 的 `light_tank_chassis` 没补 → 装甲师 eq_ratio 偏低 | 改补 `infantry_equipment`/`light_tank_chassis`/`artillery_equipment`(对齐营 need) | spec §8.2(装甲对比)/数据正确性 |
| 9 | `parser/block.rs:183` `other =>` 不可达分支(key_token 已兜底) → unreachable_patterns 警告 | 删除 dead code, 注释说明 | 工程整洁 |

**验证**: `tests/web_demo.mjs`(Playwright, 用系统 Chrome `channel:'chrome'`) 13 项全过: loading 隐藏/game 显示/无 console.error/无 pageerror/canvas 非零尺寸/canvas 画出内容(724 非黑采样点, 含 GER红`#e94560`+FRA绿)/get_state 含 date/wars/factions/顶栏日期+系统按钮/底栏时间控制/点击弹抽屉/tick 推进/截图。截图: `tests/demo-final.png`。

**运行验证脚本**:
```bash
cd web && python -m http.server 8765 &   # 另开终端
npm install playwright-chromium            # 一次性
node tests/web_demo.mjs                    # 13/13 应全过
```

### 基础构造层(本阶段, 2026-06-24~25)

**目标**: 搭建"多系统耦合地基", 免得后续加系统时返工。审计原版 28 个 defines 子系统, 识别出 5 个真地基, 全部完成。

#### 1. 数据驱动引擎(GameData)

把"硬编码游戏"变成"数据驱动引擎"。师从"硬编码 create_division"变成"由模板+营+装备数据汇总计算"。

- **`src/data/`** 层(parser 的第二个消费者, 与 runtime 平行):
  - `equipment.rs`: ChassisDef/ModuleDef/EquipmentDef + compute_equipment_stats(加法后乘法)
  - `subunit.rs`: SubUnitDef + combat_stats(营属性=need装备×件数/100)
  - `template.rs`: DivisionTemplate + to_division_stats(营→师汇总: 求和/加权混合60-40/按width加权)
  - `loader.rs`: load_all 统一入口 + 两遍扫描解继承 + 装备别名注册
- **数据来源**: 原版文件编译期嵌入 `src/data_raw/`(include_str!)
- **统一装备模型**: 所有装备 = 底盘+模块组合。整件装备(步兵)是 slots 为空的底盘
- **GameData 进 World**(Arc 共享只读, OnceLock 缓存)
- **create_division 加 template 路径**(数据驱动汇总, 旧 battalions 路径隔离保留)

#### 2. modifier 层(陆战结算统一修正接口)

后续所有系统(科技/国策/将领/堑壕/地形/昼夜)通过往 ModifierStack 塞数据影响结算, 不再各自改结算代码。

- **`src/combat/modifier.rs`**: Modifier/ModifierStack/CombatContext
- **op 由属性名后缀推导**(对齐原版 Paradox 约定): `_factor`=Multiply, 无后缀=Add
- **叠加公式**: `(1+ΣAdd) × Π(1+Multiply)`
- **CombatContext**: 结算前快照, 汇总 国家+省份+师 三层 modifier(避借用冲突; 支持昼夜等动态 modifier)
- **三结算点口子**: effective_*(mods) / can_join_frontline(mods) / recovery(mods)
- **空栈默认返回 1.0**(现有测试零破坏)

#### 3. State 概念(省份上级容器)

- `State { id, owner, controller, manpower, category, cores, buildings, provinces }`
- `Province` 删 owner/controller, 加 state_id; 归属从 State 派生
- **占领只改 controller, 不改 owner**(法理归属 vs 实际控制)
- **State 进 World**(可变运行时, 不进 GameData。依据: State 可变 + 剧本切换需改归属)
- `src/data/state_loader.rs`: 读 history/states/*.txt

#### 4. 日期系统(精确公历)

- `src/runtime/date.rs`: GameDate + from_hours/to_hours(Howard Hinnant 绝对天数算法)/day_of_year
- `World.date()` 从 hour 派生(保留 hour 不动, 现有测试零破坏)
- clock 月切换改月份比对(不再 % 30, 月份天数不固定)
- 闰年正确(1936.2.29 存在)

#### 5. 战争状态(War 关系)

- `War { attackers, defenders }`(攻守两侧 tag 集合)
- `are_at_war(a, b)` / `enemies_of(tag)` / `declare_war(attacker, defender)`
- `Country.faction` 阵营字段; 宣战时同阵营成员自动加入
- 5 处 `owner_tag != owner`(全员敌对) → `are_at_war`(战争关系判定)
- `start_battle` 自动宣战(现有测试零改动兼容)
- 命令: declare_war/white_peace/create_faction/join_faction

#### 实施中解决的真实数据加载问题

| 问题 | 根因 | 修正 |
|---|---|---|
| lexer BOM | 原版文件 Windows BOM | lex 跳过 \u{feff} |
| 非法数字 | 日期 1939.1.1 多段点号 | parse 失败回退为字符串 token |
| 命名空间限定 | `mio:GER_xxx` 冒号 | ident 字符集含 `:` |
| 裸 ident 列表 | `type={infantry}` | parser lookahead: ident 后非=则列表 |
| archetype 别名 | 营 need 引用 archetype 名, 装备按型号存 | loader 注册最新型号别名 |
| 数字 key | buildings 块 `3838={naval_base=0}` | parse_block 支持 Num 作 key |

#### 设计原则沉淀

`docs/design-principles.md` — **原版设计是首要参考对象**。每次做新系统先查原版数据文件/defines/wiki, 不凭直觉设计。教训: modifier 的 op 最初设计了"双模式"(脚本显式标记 + loader 猜文件类型), 查证后发现原版用属性名后缀(`_factor`)自动推导, 根本不需要标记。

---

## 2. 当前代码结构

```
src/
├── parser/          lexer(含BOM跳过/日期容错/冒号ident/裸ident列表/Num作key) + block
├── ast/             effect/trigger/lower(Block→Effect/Trigger)
├── data/            ★数据驱动层(parser的第二个消费者, 与runtime平行)
│   ├── mod.rs          GameData(只读数据库) + EquipStats(add/multiply) + OnceLock缓存
│   ├── equipment.rs    ChassisDef/ModuleDef/EquipmentDef/SlotDef + compute_equipment_stats
│   ├── subunit.rs      SubUnitDef + combat_stats(营=need装备×件数/100) + battalion_mult
│   ├── template.rs     DivisionTemplate + to_division_stats(营→师汇总公式)
│   ├── loader.rs       load_all统一入口 + 两遍扫描解继承 + 装备别名注册
│   └── state_loader.rs load_states读history/states/*.txt
├── data_raw/        ★原版数据文件拷贝(编译期include_str!嵌入)
├── runtime/
│   ├── entities.rs   Province(state_id) / State / Country(faction) / War / Division / Battle / Scope
│   ├── world.rs      World状态 + wars/are_at_war/declare_war + states派生查询 + date()派生
│   ├── date.rs       ★GameDate 精确公历(闰年/monthDay) + from_hours/to_hours
│   ├── clock.rs      GameClock::tick(主循环, 月切换用月份比对)
│   ├── interp.rs     Interpreter(run/eval + run_for_each作用域枚举)
│   ├── registry.rs   Registry(effects/triggers) + ParamGet
│   └── error.rs      CmdError
├── combat/
│   ├── modifier.rs   ★陆战结算统一修正接口(Modifier/ModifierStack/CombatContext + _factor推导op)
│   ├── resolve.rs    陆战结算(注入CombatContext; atk_stats/pool_value接mods)
│   ├── movement.rs   check_engagements(are_at_war判定) + advance_movement(set_state_controller占领)
│   ├── width.rs      战斗宽度(乘CombatWidth modifier) + reinforce_reserves
│   ├── recovery.rs   组织度恢复(内联字段访问避借用冲突; 乘OrgRegain modifier)
│   ├── reinforce.rs  装备+人力增援(每日, 排除 Moving/Retreating)
│   ├── commands.rs   create_state/province/division(template/battalions/手填) + 战争命令 +
│   │                 move_division/support_attack/stop_order + start_battle(自动宣战)
│   ├── pathfinding.rs BFS寻路 + is_passable插槽
│   └── equipment_data.rs 5种1936装备硬编码表(旧路径用)
├── commands/         vars/control/scope命令注册
├── wasm_api.rs       WASM桥接(序列化省份controller/owner从State派生读)
└── lib.rs / main.rs
web/
├── index.html              # 入口: 地图全屏 + 顶栏/底栏/抽屉/面板宿主
├── css/app.css             # 移动优先全屏布局 + 组件样式
└── js/                     # 25 文件 1166 行, ES Modules, 无构建
    ├── main.js             # 启动: 装配 + 两段式交互 + 完整 setup
    ├── engine/             # WASM 封装(wasm/state/commands)
    ├── core/               # 通用框架(store/bind/router/canvas/input/el)
    ├── map/                # 6 图层(layout+terrain/province/unit/order/combat/overlay)
    ├── ui/                 # 复用组件(topbar/drawer/orderMenu/statbar)
    └── views/              # 面板内容(deploy/diplo/unit/combat)
docs/
├── design-principles.md  ★复刻设计原则(原版是首要参考)
├── formulas/land-combat.md  陆战公式(四量模型/防御池/装甲/宽度)
└── superpowers/      specs(6篇设计文档) + plans(7篇实现计划)
```

### 主循环顺序(clock.rs 每小时)
```
1. prev_month = date().month; hour += 1; started = true
2. fire_event(on_hourly)
3. check_engagements        — Moving/Pending师遇敌→开战(are_at_war判定, 过滤Retreating)
4. cancel_finished_supports — 支援目标省战斗结束→清Supporting
5. resolve_all_battles      — 战斗结算(CombatContext注入; 伤害累积+撤退+歼灭+带溃+占地)
6. reinforce_reserves       — 预备队补位(2%/h)
7. advance_movement         — 进度推进 + 到达判定(set_state_controller占领)
8. recover_org              — org恢复/损耗(读OrderState + OrgRegain modifier)
9. hour%24==0 → on_daily + reinforce_all(每日增援)
10. hour%(24*7)==0 → on_weekly
11. date().month != prev_month → on_monthly(精确月份边界)
```

---

## 3. 基础构造层的接口总结(后续系统怎么接入)

| 后续系统 | 接入方式(不改基础构造层) |
|---|---|
| **国策** | 完成奖励 → add_country_modifier; trigger 读 are_at_war / date(); 花费 date() 算天数 |
| **科技** | 完成 → add_country_modifier(stat=soft_attack value=0.05); 解锁装备(GameData) |
| **将领** | add_division_modifier; 技能影响 modifier |
| **堑壕** | 战斗每小时 dig_in++, 转 add_division_modifier(stat=defense_factor) |
| **地形** | terrain_modifiers 函数填真实值(替换占位空栈) |
| **昼夜** | State纬度 + World.date().day_of_year() → darkness; CombatContext省份层加 night_modifier |
| **补给** | 读 State.buildings(infrastructure); supply flow 沿 State 计算 |
| **生产** | 读 State.buildings(industrial_complex/arms_factory); State.manpower(征兵) |
| **剧本切换** | World初始化后运行 transfer_state 命令改 owner/controller |
| **宣战/阵营** | declare_war / create_faction / join_faction |
| **移动速度口子** | (modifier层未覆盖, 需要时加 MovementSpeed + movement.rs 口子) |

**核心**: 后续系统只"往接口塞数据", 不改 resolve.rs / effective_* / width.rs / recovery.rs / State结构 / War结构。

---

## 4. 下阶段方向: 完善 demo 做实际测试

**目标**: 把基础构造层接入 UI demo, 做端到端实际测试, 暴露架构问题。

### 当前 demo 状态

- **web/index.html**: 单文件 UI(Canvas节点图 + 交战面板 + 弹菜单 + 进度箭头)
- **10省对垒地图**: 上排 GER / 下排 FRA
- **已有**: 师部署 + 移动/进军/支援攻击/停止命令 + 战斗可视化
- **缺口**: demo 还用旧脚本(create_division battalions, 未接 create_state/template/declare_war); 数据驱动/State/战争的新能力未在 UI 体现

### 未实现系统(按优先级, 供后续选择)

| 系统 | 依赖的地基(都已就位) | 复杂度 |

| 系统 | 依赖的地基(都已就位) | 复杂度 |
|---|---|---|
| 国策系统 | modifier + date + war(trigger) | 中 |
| 科技系统 | modifier + GameData(解锁装备) | 中 |
| 生产系统 | State(buildings/manpower) | 中高 |
| 补给系统 | State(buildings) + date | 高(HOI4最复杂) |
| 外交系统 | war + faction | 中 |
| 建筑系统 | State(buildings升级) | 中 |
| 投降/和平会议 | war + State(victory_points待加) | 高 |

---

## 5. 新会话怎么接上

1. 在 `G:\projects\hoi4-clone\` 开新对话
2. 读本文件了解全局; 读 `docs/design-principles.md` 了解设计原则
3. `git checkout feat/data-driven-engine`(若不在)
4. 跑 `cargo test` 确认基线(**180测试**)
5. 看 §4 选下一步(demo 完善 / 新系统)

### 运行UI demo
```bash
cd G:\projects\hoi4-clone\web
python -m http.server 8765
# 浏览器开 http://127.0.0.1:8765
```

### 重新编译WASM
```bash
cd G:\projects\hoi4-clone
cargo build --target wasm32-unknown-unknown --lib --release
cp target/wasm32-unknown-unknown/release/hoi4_clone.wasm web/
```

### 关键约束(踩过的坑)
- 工具链: 必须用 `stable-x86_64-pc-windows-gnu`(无MSVC链接器)
- WASM FFI: u64参数在JS侧要BigInt, 用u32避免
- WASM更新后: fetch加 `?v=Date.now()` 防缓存
- engine_tick: 必须用 GameClock::advance(完整主循环), 不能内联
- **借用冲突**: get_mut 持有借用时不能再 world.divisions.values(), 用快照→计算→写回模式
- **敌人判定**: 用 are_at_war/enemies_of, 不能用 owner_tag != owner(那是旧的全员敌对)
- **省份归属**: 用 province_controller/province_owner 派生查询, 不能直接读 Province(已无 owner/controller 字段)
- **recovery 借用**: 遍历 divisions.values_mut 时查 controller 必须内联字段访问(provinces/states分离借用)
