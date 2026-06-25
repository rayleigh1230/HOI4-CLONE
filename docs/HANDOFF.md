# hoi4-clone 项目交接文档

> **用途**: 在新会话中继续开发。读本文件 + 列出的关键文件即可接上。
> **更新**: 2026-06-25(P0-2 地形修正接通 — 攻方惩罚 + 地形宽度; **208 测试** = 143 内联 + 48 battle + 13 integration + 3 scope + 1 teleport; 见下方"P0-2 地形修正接通"小节)

---

## 0. 项目概况

完整复刻 HOI4 核心机制的自制游戏, 分享给朋友玩。
- **技术栈**: Rust → WASM + 单文件 HTML/JS 前端
- **位置**: `G:\projects\hoi4-clone\`
- **运行**: `cargo test`(测试) / `cargo run --bin hoi4_demo`(CLI) / 浏览器 `http://127.0.0.1:8765`(UI demo)
- **工具链**: `stable-x86_64-pc-windows-gnu`(rustup override 绑定)
- **规模**: ~8100 行 Rust + UI(30+ JS 文件) + 原版数据, **208 Rust 测试 = 143 内联 + 48 battle + 13 integration + 3 scope + 1 teleport; 22 Playwright 验证**
- **分支**: `feat/data-driven-engine`(本轮 25 个提交: 地图视觉 + 游戏逻辑完善)
- **验证**: `node tests/web_demo.mjs`(Playwright, 用系统 Chrome channel:'chrome', 22 项端到端)

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
| **地图视觉&战斗可视化改造** | 见下方"地图视觉改造"小节(13 Task + Playwright 17/17) | 122+UI |
| **游戏逻辑层完善** | 见下方"游戏逻辑完善"小节(占领省份级/国家视角/移动距离+速度/外交/拖拽下令; 126 测试 + 22 验证) | 126+UI |
| **P0-1 国家资源重构** | 见下方"P0-1 国家资源重构"小节(三件套国家级 + modifier 打通 + create_country; 202 测试) | 202 |
| **P0-2 地形修正接通** | 见下方"P0-2 地形修正接通"小节(攻方地形惩罚 + 地形宽度(含反击修正); 208 测试) | 208 |

### P0-2 地形修正接通(2026-06-25, 地基层最后一块拼图)

把 `terrain_modifiers()` 占位(返回空栈)换成原版真实值。地基层(modifier/State/date/War)此前都完整, 唯独地形对战斗的影响是空的——所有战斗数值都偏。本次接通后战斗贴合原版。维度②攻方惩罚 + ③地形宽度(移动成本①此前已实现)。

| 改造 | 内容 | 对齐/来源 |
|---|---|---|
| **攻方地形惩罚** | `terrain_attacker_penalty(terrain)`: plains/desert1.0 / forest·jungle0.80 / hills0.70 / marsh·urban0.60 / mountain0.40; 攻方 soft/hard_attack 乘此系数 | 原版 terrain.txt(攻方惩罚, 守方不受影响) |
| **地形宽度** | `terrain_combat_width(terrain)`: plains·hills·desert70 / forest·jungle60 / marsh54 / mountain50 / urban80; `can_join_frontline` 按省份地形查表(原固定70) | 原版 terrain.txt combat_width |
| **攻守区分注入** | CombatContext 加 `attacker_terrain_penalty` 字段(build 按省份地形填); 原版**只罚攻方身份**(攻守整场固定): 正向罚攻方 attack + 反击罚攻方 breakthrough(攻方挨反击池被削); 守方 attack/defense 都不罚(享受地形优势) | 原版 Land battle wiki: attacker receives attack and breakthrough penalties |

**关键设计决策**(用户确认):
- 范围 = 攻方惩罚 + 地形宽度(维度②③); 排除兵种地形加成(维度④, 需改营数据, 留装备层)
- 数据硬编码查表(对齐 `terrain_movement_cost` 风格, 不引 terrain.txt 数据文件加载)
- 攻方惩罚**不进 CombatContext 通用 stack**(避免误伤守方), 改为攻方专用注入
- 多方向加宽(每多一进攻方向 +base/2)**本次不做**(YAGNI, demo 单方向), 留 TODO

**踩坑**:
- `resolve_all_battles` 有两处**内联**调 `AtkStats::from`(正向+反向反击), 绕过 `resolve_hour`; 改签名时两处都要更新(不只 resolve_hour)。
- **地形惩罚攻守归属(2026-06-26 修正)**: 初版凭直觉"谁开火罚谁", 反击也罚守方。查证原版后发现错——原版只罚攻方身份(攻守整场固定, 不随反击翻转), 且罚 attack**和 breakthrough**两个。修正: 守方反击不罚其 attack; 攻方挨反击的 breakthrough 池被罚(更脆)。教训记入 CLAUDE.md 红线1。
- integration 偶发 flaky(TEST_BLOCKED 泄漏, 既有); 单线程 `--test-threads=1` 稳定 206/206。

### P0-1 国家资源模型重构(2026-06-25, 对齐 spec 2026-06-25-country-resources-design)

把政治点/稳定度/战争支持度(三件套)从 `World.vars` **全局变量**改成 `Country` **具名字段**, 打通 modifier 接口。为后续国策/科技/政治系统铺地基(P0-1 是它们的前置)。spec → plan → 9 Task TDD 实现。

| 改造 | 内容 | 对齐/来源 |
|---|---|---|
| **Country 加资源字段** | `political_power`/`stability`/`war_support` 具名字段(存 base); 手写 Default(stability/war_support=0.5 对齐原版 BASE_*) | 原版 defines NDefines.NCountry |
| **modifier 接口打通** | `ModifierStat` 加 Stability/WarSupport/PoliticalPower; `parse_modifier_token` 接受 stability(+_factor) 等 token; 复用现有 ModifierStack(零新结构) | 原版: 稳定度走统一 modifier 框架, 非独立加法池(调研推翻"加法池"直觉) |
| **effective 读取** | `effective_stability()`=clamp(base×mult,0,1) + buffer 保留超额; trigger/UI 读 effective, 命令改 base | 原版 Government wiki |
| **资源命令国家级** | add/set_stability、add/set_political_power 从全局→当前作用域国家; 无国家时报错(非静默) | 决策5 |
| **create_country 命令** | 建国家实体+设资源初值(字段可选); 重复 tag 覆盖 | 原版 history/countries 加载语义 |
| **trigger Compare 作用域化** | `political_power >= 150` 读当前国家 effective; 无国家返回 0(trigger 不报错, 与命令不对称是刻意) | interp.rs |
| **序列化** | get_state 新增 countries 数组(含 effective 资源字段), 供顶栏 UI(UI 本身后做) | — |

**关键设计决策**(用户确认 + 原版调研):
- 资源存 Country 具名字段(非 HashMap) — 三件套固定, 与现有具名字段风格一致
- **复用 ModifierStack**(调研关键发现): 原版稳定度就是普通 modifier 属性走统一框架, `stability`(Add)+`stability_factor`(Multiply), 公式与战斗属性一致; 我一度以为是"独立加法池", 查证后纠正
- 作用域栈优先回退 player_tag; 无国家时命令报错/trigger返回0(刻意不对称)
- 不兼容全量迁移测试(不留双写债); 排除 PP默认增长/资源效果/fuel/UI(YAGNI, 留对应系统)

**踩坑/记录**:
- `ParamGet::get(p, key)` 要全限定调用 — slice 的 inherent `get(usize)` 会遮蔽 trait method(plan 初稿写成 `p.get("tag")` 编译失败, 修正)。
- `current_country()` 返回 `Option<&str>` 借自 &World, 命令要 `&mut Country` → 用 `current_country_tag()`(owned String)快照后 `get_mut`(避借用冲突)。
- integration 偶发 flaky(TEST_BLOCKED thread-local 跨测试泄漏, **既有问题非本次引入**); 单线程 `--test-threads=1` 稳定 13/13。

### 测试基线修复(2026-06-25, 本轮)

接手时发现 `cargo test` 编译即失败(并非 HANDOFF 旧版声称的"126 测试全绿")。根因:前序两轮改造加了字段/改了公式, 但**漏改了 `tests/` 集成测试**, 导致 3 个集成测试目标无法编译, 自那以后一直没跑过。修复后暴露并修好 8 个真实回归。这是"文档与实际脱节"的教训——下次改 struct 字段/核心公式后, 必须跑**全量** `cargo test`(含 `tests/` 集成目标), 不能只看 `src/` 内联测试。

| # | 根因 | 影响范围 | 修复 |
|---|---|---|---|
| **A 编译失败** | commit 20cedf4 给 `Province` 加 `controller` 字段, 更新了 `src/` 4 处构造点, 但**漏掉 `tests/battle.rs` `tests/scope.rs` `tests/teleport_bug.rs`** 的 `Province{}` 构造 | 3 个集成测试目标编译失败, 全量 `cargo test` build failure | 3 处补 `..Default::default()` |
| **B 移动计时回归** | commit c3a3f92 把移动公式从恒定 `MOVE_RATE=0.05`(20h 到达) 改为 `max_speed/(距离km×地形成本)`, 但**没更新 battle.rs 里硬编码的 `advance(小时数)`** | 5 个测试: move_to_empty/march_into/frontline_route/t_multihop/t_queue_move — advance(21/90/100) 在新公式下不够到达 | 把 advance 加大到新公式下足够到达(80/250/800 等), 每处加注释说明公式与距离 |
| **C 国家视角回归** | commit 0b65327 加了 `player_controls` 校验(player_tag 非空时只能下令自己国家的师), 但**战斗编排测试靠双向下令 GER+FRA 师造场景**, 在 `player_tag="GER"` 下 FRA 命令被静默拒绝 | 3 个测试: stop_keeps_passive_defense/defender_move_to_friendly/defender_move_to_enemy — 下 FRA 命令被拒, 战斗未创建 | 这 3 个测试清空 `player_tag=""`(CLI 模式, 绕过单国家控制; 它们测引擎规则非玩家权限) |

**验证**: `cargo test` 全绿 — 126 内联 + 48 battle + 13 integration + 3 scope + 1 teleport = **191 测试**; `cargo build --target wasm32-unknown-unknown --lib --release` 0 警告。

**遗留观察(非 bug, 记录用)**:
- 旧建师路径(`create_division` 用 `battalions=`)的 `max_speed` 硬编码 4.0(`commands.rs:259`), **忽略 equipment 参数的速度**——medium_tank 装备并不给 8 的速度。这是设计债(equipment_data.rs 同源), 不在本轮范围。battle.rs 测试据此校准小时数。
- `tests/integration.rs` 偶发单测失败(TEST_BLOCKED thread-local 跨测试泄漏? 重跑即过)。若再现, 查 pathfinding.rs:80 `set_test_blocked`/`clear_test_blocked` 配对。

### 游戏逻辑层完善(2026-06-25)

地图视觉做完后, 实战暴露一批游戏逻辑缺陷 + 交互体验问题, 系统性修复并完善。每项对齐原版, 标注调研来源。

| 改造 | 内容 | 对齐/来源 |
|---|---|---|
| **占领省份级** | Province 加 `controller: Option<String>`(None 从 State 派生, Some 省份级覆盖); `set_province_controller` 只改该省不蔓延; `province_controller` 优先读省份级。**根因**: 原 `set_state_controller` 改 State 级, 占领一省→整个 State 易主 | HOI4 省份级占领 |
| **国家视角权限** | 4 个下令命令(move/support/queue/stop)加 `player_controls` 校验(owner!=player_tag 静默拒绝); player_tag 空时放行(CLI/测试兼容); create_division 不校验(setup 建双方合理); TDD `t_player_cannot_order_foreign_division` | HOI4 单国家控制(wiki/observe mode 调研) |
| **省间距离 + 师速度** | SubUnitDef/DivisionStats/Division 加 `max_speed`(取最慢营); 营文件 light_armor=12/medium_armor=8; `province_position/distance`(重心欧氏距离=km); advance_movement 公式: `每小时推进度 = max_speed/(距离km×地形成本)`; 地形成本(terrain.txt): plains1.0/forest·hills1.5/mountain·marsh2.0/urban1.2; 战斗中×0.33; TDD `t_armor_moves_faster_than_infantry`(装甲3倍步兵) | HOI4 defines + terrain.txt + Land units wiki |
| **外交系统** | diplomacyPanel 重写: 实时展示当前战争(GER⚔FRA)+阵营归属; 国家选择(A→B)+宣战/白和/创建阵营/加入阵营; 替代旧写死按钮 | — |
| **部署区分国家** | deployPanel owner 锁定 player(只部署自己); _deployTemplate(owner,tmpl); 修复 deployTemplate 缺 owner 参数的 bug(原 owner=省id/loc=模板名) | — |
| **拖拽下令交互** | 取消框选; 拖兵牌拉箭头指向目标省(原版核心交互); 拖动中鼠标悬停省实时金色高亮; 松开弹命令菜单; 左键拖空白/右键拖=平移; 战斗气泡点击→左侧出详情框(landcombatview, 不跳路由); 所有面板带关闭按钮+ESC | HOI4 拖拽下令 |
| **切视角** | 顶栏"切控制权"(上帝模式)→"切视角"(弹 GER/FRA 选择 setPlayer 切 player_tag); 顶栏显示当前视角(👁 GER); 删除上帝模式改省归属 | HOI4 控制台 tag 切换 |

**关键设计决策**(用户确认):
- 占领=省份级 controller(非 demo 改 1省1State 取巧, 修引擎根因)
- 国家视角=单国+可切视角(玩家只控制 player_tag, 切视角按钮切国家)
- 距离=重心欧氏距离当 km(世界坐标 1000×700 抽象, 与前端 layout 网格一致)

**踩坑记录**:
- `deployTemplate(owner,loc,template)` 3 参, 部署 bug 是只传 2 参(owner 当了省id)。web_demo 固化"部署师数+1"回归测试。
- Province 加 controller 字段后, 4 处测试构造点要补 `..Default::default()`(commands/modifier/movement/resolve.rs)。
- 移动公式改后, 旧测试用 max_speed=0 走 MOVE_RATE 回退分支故仍过(兼容); 新公式靠 `t_armor_moves_faster` 专项验证。
- 国家视角: 敌师兵牌拖拽不进 dragOrder(canCommand=false), 但点击可查看信息(onHit 兜底显示"非己方")。
- 战斗触发条件: `check_engagements` 只对 Moving/Pending 师+目标省有敌军开战; demo setup 加进攻命令才有战斗。

### 地图视觉&战斗可视化改造(2026-06-25, 对齐 map-visual-overhaul spec)

把地图从"抽象圆点+全黑底"改成"多边形拼图+地形底色+完整 NATO 牌+战斗小圆+详情面板"。头脑风暴 4 节确认 → spec → 13 Task 实现, Playwright 17/17 验证。每项标注 spec 条目, 便于追溯。

| Task | 内容 | 对齐 spec |
|---|---|---|
| 1 | get_state 序列化补 soft/hard/defense/breakthrough/armor/piercing/combat_width(战斗面板用) | §6.1 |
| 2 | layout.js 重写: 固定世界坐标系(1000×700) + 手写 10 省多边形(5列×2排网格无缝) + 地形 + pointInPolygon | §2 |
| 3 | canvas.js 相机 fitToWorld(世界居中可见) + resize 同步 | §2.4 |
| 4 | layerTerrain: 多边形填充地形色 + offscreen 噪点纹理(替代全黑底) | §3.1 |
| 5 | layerProvince: 多边形描边 controller 色 + 淡填充(地形底色透出) + 省号 | §3.2 |
| 6 | layerOverlay: 选中沿多边形金色描边 + 前线脉冲(controller 不同的相邻省) | §3.3 |
| 7 | layerOrder: 改用 provinceCentroid 世界坐标(适配新布局) | §5.2 |
| 8 | layerUnit: 完整 NATO 76×24 牌(兵种+org/str竖条+数量+国旗色边框+牌堆合并+战斗描红) | §4 |
| 9 | layerCombat: 带进度数字小圆(可点击) + combatIcons 导出; rAF 动画循环(前线/战斗脉冲) | §5.1 |
| 10 | combatPanel: 升级 landcombatview 风格(攻守双方+师soft/hard/defense+进度+宽度+预备队) | §5.3 |
| 11 | main.js: 命中改 pointInPolygon + 战斗图标点击优先开战斗面板 + rAF | §3.4/§5.4 |
| 12 | demo setup 加 GER 进攻省7(战斗可视化内容) + 扩展 web_demo 17/17 | §5 |
| 13 | HANDOFF 更新 + 最终回归(122 测试/wasm 0 警告/17 验证) | — |

**关键决策**(头脑风暴确认): 省份=自定义多边形(非真实地图, 不突破 spec 非目标); 多师=牌堆合并(对齐原版 unit_counter); 战斗图标=带进度数字小圆(对齐 land_combat_mapicon, 进军箭头归 layerOrder); 战斗详情=点击图标弹独立面板(landcombatview 风格); 命中优先级=战斗图标 > 省份多边形。

**验证**: `tests/web_demo.mjs` 17 项全过 — loading/game/无错误/canvas非黑(16000点)/get_state字段/顶栏/底栏/多边形命中弹抽屉/战斗图标点击开面板/地形多边形渲染(44001绿色调采样)/战斗属性字段/tick。截图 `tests/demo-final.png`。

**踩坑记录**:
- 战斗触发窗口短: `check_engagements` 只对 Moving/Pending 师 + 目标省有敌军开战; 师到达后转 Idle 不再开战。原 demo 无进攻命令故无战斗(非 bug)。加进攻命令后战斗正常触发, layerCombat 小圆/面板有内容。
- provinceCentroid 返回 `{x,y}` 对象(非数组), 各图层消费用 `.x/.y`。
- rAF 全量重画下, spec §4.4 的"layerUnit 订阅 divisions 脏标记"优化冗余(rAF 已保证牌子实时), 不额外订阅(避免过度设计)。

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
├── index.html              # 入口: 地图全屏 + 顶栏/底栏/抽屉/面板宿主/命令菜单
├── css/app.css             # 移动优先全屏布局 + 组件样式(NATO牌/战斗面板/外交面板)
└── js/                     # ES Modules, 无构建
    ├── main.js             # 启动: 拖拽下令交互 + 国家视角 + 完整 setup(GER 进攻省7)
    ├── engine/             # WASM 封装(wasm/state/commands)
    ├── core/               # 通用框架(store 路径级脏标记/bind/router/canvas/input/el)
    ├── map/                # 6 图层(layout 多边形+地形+pointInPolygon/terrain/province/unit/order/combat/overlay)
    ├── ui/                 # 复用组件(topbar 切视角/bottombar 时间/drawer/orderMenu/statbar)
    └── views/              # 面板内容(deploy 锁player/diplo 战争+阵营/unit/combat landcombatview)
tests/
├── web_demo.mjs            # ★Playwright 端到端验证(22 项, 系统 Chrome channel:'chrome')
└── demo-final.png          # 验证截图存证
docs/
├── design-principles.md  ★复刻设计原则(原版是首要参考)
├── formulas/land-combat.md  陆战公式(四量模型/防御池/装甲/宽度)
└── superpowers/      specs + plans(地图视觉改造 + 游戏逻辑完善均含 spec/plan)
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
| **地形** | **已实现**: terrain_attacker_penalty(攻方惩罚) + terrain_combat_width(地形宽度); 见"P0-2 地形修正接通"小节。剩余: 兵种地形加成(维度④, 留装备层) + 多方向加宽(TODO) |
| **昼夜** | State纬度 + World.date().day_of_year() → darkness; CombatContext省份层加 night_modifier |
| **补给** | 读 State.buildings(infrastructure); supply flow 沿 State 计算 |
| **生产** ★下一阶段 | 读 State.buildings(industrial_complex/arms_factory); 国家仓库 Country.equipment_stockpile(待加); reinforce 改从仓库扣减(不再从虚空) |
| **剧本切换** | World初始化后运行 transfer_state 命令改 owner/controller |
| **宣战/阵营** | declare_war / create_faction / join_faction(**已实现**) |
| **移动速度** | **已实现**: max_speed/(距离km×地形成本); Province.controller 省份级占领(已实现); 见"游戏逻辑层完善"小节 |

**核心**: 后续系统只"往接口塞数据", 不改 resolve.rs / effective_* / width.rs / recovery.rs / State结构 / War结构。

---

## 4. 下阶段方向: 生产 + 装备系统(下一阶段重点)

**目标**: 实现生产系统(工厂造装备) + 装备补充流(装备从仓库到师), 让"师的损耗→工厂补给→再战"闭环跑通。

### 当前 demo 状态(已完善)

- **地图**: 5列×2排多边形拼图 + 地形底色 + 固定世界坐标系(1000×700) + pan/zoom
- **部队牌**: 完整 NATO 76×24(兵种+org/str竖条+数量+国旗边框+牌堆合并)
- **战斗**: 带进度数字小圆(可点击)+landcombatview 风格详情面板; 省份级占领不蔓延
- **交互**: 拖兵牌下令(原版)+战斗气泡出详情框+面板关闭+ESC
- **国家视角**: 单国控制权限(player_controls)+切视角按钮+部署锁 player
- **移动**: 省间距离(重心)+师速度(max_speed)+地形移动成本公式
- **外交**: 战争/阵营状态展示+宣战/白和/阵营操作; 默认 GER⚔FRA 交战
- **引擎**: 数据驱动建师(template)+change_template+declare_war+省份级controller

### 下一阶段: 生产 + 装备系统

**为什么**: 现在师的装备是建师时满编(`engine_supply` 一次性补满), 战斗损耗后**没有补充流**——师打废了就废了。生产系统让工厂持续造装备入仓库, 增援从仓库补充到师, 形成"损耗→生产→补给→再战"闭环。这是 HOI4 经济与军事的核心纽带。

**依赖的地基(都已就位)**:
- `State.buildings`(已有, 含 industrial_complex/arms_factory 等建筑字段)
- `Division.equipment_need/equipment_held`(M4a 装备库存/消耗已有)
- `reinforce_all`(每日增援已有, 但只从"虚空"补, 不消耗仓库)
- `GameData.equipment`(装备定义已有: chassis/module/整件装备模型)

**预计要做的**:
1. **国家仓库**(Country.equipment_stockpile): 存各装备类型的库存量
2. **生产**(clock 每日): 工厂(industrial_complex/arms_factory)按建筑数×效率产装备入仓库
3. **增援改造**(reinforce.rs): 师缺装备时从国家仓库扣减补充(不再从虚空); 仓库不足则缺编
4. **UI**: 仓库面板(各装备库存)+生产概览(工厂产出); 师牌子显示装备满编度(eq_ratio 已有序列化)
5. **建筑**(可选, 本次或后续): State.buildings 升级(建/拆工厂)

**调研来源**(下一阶段实施时查):
- 原版 `common/defines/00_defines.lua`: `BASE_FACTORY_SPEED`(5)/`BASE_FACTORY_SPEED_MIL`(4.5)、生产线相关
- 原版 `documentation/effects_documentation.md`: `add_equipment`/`transfer_equipment`/`add_factory` 等
- 原版 `common/buildings/`: 工厂建筑定义(industrial_complex/arms_factory 等)
- HANDOFF §3 "基础构造层接口总结" 的"生产/补给"接入方式行

### 其他未实现系统(优先级排序)

| 系统 | 依赖 | 复杂度 | 备注 |
|---|---|---|---|
| **生产+装备补充** | State.buildings + equipment库存 | 中高 | **下一阶段重点** |
| 补给系统 | State(buildings) + date + 距离 | 高 | HOI4 最复杂, 生产之后 |
| 国策系统 | modifier + date + war | 中 | trigger/effect 已就位 |
| 科技系统 | modifier + GameData | 中 | 解锁装备+加 modifier |
| 建筑系统 | State(buildings 升级) | 中 | 可与生产一起做 |
| 投降/和平会议 | war + State(vp) | 高 | 需 victory_points |

---

## 5. 新会话怎么接上

1. 在 `G:\projects\hoi4-clone\` 开新对话
2. 读本文件了解全局; 读 `docs/design-principles.md` 了解设计原则(原版是首要参考)
3. `git checkout feat/data-driven-engine`(若不在)
4. 跑 `cargo test` 确认基线(**208 测试 = 143 内联 + 65 集成/battle/scope/teleport**; integration 偶发 flaky 用 `--test-threads=1` 稳定)
5. (可选)跑 `node tests/web_demo.mjs` 确认 UI(需先 `cd web && python -m http.server 8765`)
6. 看 §4 选下一步(**生产+装备系统**是推荐重点)

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
- **省份归属**: province_controller/province_owner 派生查询。Province 现有 controller 字段(省份级占领, None 时回退 State)
- **recovery 借用**: 遍历 divisions.values_mut 时查 controller 必须内联字段访问(provinces/states分离借用)
- **国家视角**: 下令命令(move/support/queue/stop)有 player_controls 校验; player_tag 空(CLI/测试)才放行
- **移动公式**: 每小时推进度 = max_speed/(距离×地形成本); max_speed=0 时回退 MOVE_RATE(兼容旧测试); 距离用 province_distance(重心欧氏)
- **部署参数**: deployTemplate(owner, loc, template) 3 参, 缺 owner 会导致 owner=省id 的隐蔽 bug
- **Province 加字段**: struct 加字段后, 显式构造点(commands/modifier/movement/resolve.rs 测试)要补 `..Default::default()`
- **JS 调试钩子**: main.js 的 refresh 挂 `window._store` 供 Playwright 验证读 state(非生产代码但无害, web_demo 依赖它)
