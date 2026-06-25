# Demo 彻底改造设计文档

> 日期: 2026-06-25
> 状态: 已批准(头脑风暴 6 节全部确认),待实现
> 关联: `docs/design-principles.md`(原则1: 原版是首要参考)
> 关联: `docs/HANDOFF.md` §4(下阶段: 完善 demo 做实际测试)
> 参考来源: 原版客户端 `G:\steam\steamapps\common\Hearts of Iron IV`(122 个 .gui + scripted_guis 文档 + 地图数据)

---

## 0. 背景与目标

### 现状问题

基础构造层(5 地基)已完成,但 demo(`web/index.html` 733 行单文件)还停在旧脚本路径:
- demo 用 `create_division battalions=` + `create_province owner=` + `start_battle`(旧路径),而新引擎签名已改成 `create_province state=` 且需显式 `declare_war` —— **demo 对新引擎很可能跑不通**
- 数据驱动新能力(`template=` 建师、真实 State 派生、`declare_war`)在 UI 上完全没体现
- UI 是桌面鼠标优先:依赖 `dblclick`/hover,命中圆 26px,无平移缩放 —— **手机触屏不可用**

实测暴露的架构 bug(见 §3):
- `to_division_stats` 静默丢弃未知营(Panzer-Division 的 6 个 light_armor 营被丢光,armor 归 0 不报错)
- 模块汇总异常(light_tank_chassis hardness 掉 0、soft_attack=0)
- 缺 light_armor 营定义 + FRA OOB 模板

### 目标

**彻底改造 demo,搭一个面向后续系统(国策/科技/生产/补给)持续扩展的 UI 架构**,同时接通新基础构造、修复暴露的 bug、适配移动端触屏。把"改顺当前 demo"升级为"搭扩展骨架"。

### 范围

- **A. 数据补全**:补 light_armor 营 + FRA OOB 模板(底盘/模块已齐全)
- **B. 引擎层修复 + 补强**:修 `to_division_stats` 静默丢弃、修模块汇总、Division 加模板引用 + `change_template`
- **C. UI 架构重写**:原生 ES Modules + 4 层结构(engine/core/views+map/ui)
- **D. 数据流对齐原版**:声明式绑定 + 引擎推送 changeset
- **E. Canvas 图层化**:拆 6 层 + canvas 管家(相机/坐标转换/脏标记)
- **F. 触屏适配**:PointerEvent 统一 + 手势 + 44px 硬约束
- **G. 接通新基础构造到 demo**:template 建师 / 显式 declare_war / 多模板对战 / 换模板功能

### 非目标

- 不接真实 State 地理数据(`1-France.txt` 省份坐标/地图渲染)—— 抽象 10 省对垒,聚焦测试数据驱动与战争关系
- 不补全所有兵种营(只补 light_armor + FRA 模板用的营;motorized/cavalry/mountaineers 等留后续)
- 不实现后续系统本身(国策/科技/生产只预留架构接入点)

### 验证方法(对齐 design-principles.md 原则1)

调研了原版客户端:
- `interface/*.gui`(122 个)—— UI 布局结构、面板滑入协议、NATO 部队牌子结构
- `common/scripted_guis/_documentation.md` —— 数据绑定模型(visible/triggers/properties/dynamic_lists)
- `common/units/*.txt` + `history/units/*.txt` —— 营定义与 OOB 模板
- `documentation/effects_documentation.md` —— `add_units_to_division_template`/`declare_war_on`/`transfer_state` 语义

---

## 1. 核心设计决策(头脑风暴确认)

| # | 决策 | 选择 |
|---|---|---|
| 1 | 布局 | B 地图全屏 + 浮层(顶栏/底栏/抽屉),桌面手机同构 |
| 2 | 接入深度 | template 数据驱动建师 + 显式 declare_war,不接真实 State 地理 |
| 3 | 兵力配置 | 步兵 + 装甲对比(多模板对战 + 宣战边界场景) |
| 4 | 数据来源 | 补 light_armor 营 + FRA OOB 模板(从原版客户端取) |
| 5 | bug 处理 | 顺手修暴露的 bug(补数据 + 修静默丢弃 + 修模块汇总) |
| 6 | 未知营丢弃 | 日志告警 + 跳过(对齐 Paradox 容错哲学,不 panic) |
| 7 | 改造深度 | 彻底改造(面向后续系统扩展,非只改顺当前) |
| 8 | 技术栈 | 无构建 · 原生 ES Modules(对齐项目"无 npm 工具链"调性) |
| 9 | 数据流 | 纯绑定式(对齐原版 scripted_gui 模型) |
| 10 | 引擎层补强 | Division 加 `template_name` 引用 + `change_template` 命令 |

---

## 2. UI 整体架构(§1 确认)

4 层结构,原生 ES Modules,浏览器 `<script type="module">` 加载,沿用 `python http.server`。

```
web/
├─ index.html              # 入口: 挂载根容器 + 加载 main.js (type=module)
├─ hoi4_clone.wasm         # 引擎产物(不变)
├─ css/
│  └─ app.css              # 移动优先 + 全屏地图布局 + 各组件样式
└─ js/
   ├─ main.js              # 启动: 装配 store/router/canvas, 注册初始系统
   │
   ├─ engine/              # ═══ WASM 封装层(引擎不变的核心契约) ═══
   │  ├─ wasm.js           # loadWasm/alloc/readCString/passStr(沿用现有, 抽出)
   │  ├─ commands.js       # 命令封装: deploy/move/support/declareWar/changeTemplate...
   │  └─ state.js          # getState() 数据视图序列化 + 派生视图模型
   │
   ├─ core/                # ═══ 通用框架层(后续系统都依赖它) ═══
   │  ├─ store.js          # 视图状态容器(接收 changeset 打补丁)
   │  ├─ bind.js           # 声明式绑定框架: bind/bindWhen/bindEnabled/bindList
   │  ├─ router.js         # 面板路由: register(name, panel)/open(name)/close()
   │  ├─ canvas.js         # Canvas 管家: 相机(pan/zoom) + 图层注册 + 渲染循环 + 脏标记
   │  ├─ input.js          # 统一输入: PointerEvent 归一化 + hit-test + 手势识别
   │  └─ el.js             # h(tag,props,children) hyperscript 造 DOM
   │
   ├─ views/               # ═══ 面板内容(每系统一个, 注册到 router) ═══
   │  ├─ deployPanel.js    # 部署面板: 选模板→选省→建师(数据驱动 template 路径)
   │  ├─ unitPanel.js      # 部队列表: 全部队概览
   │  ├─ combatPanel.js    # 交战视窗: 战斗双方 + 预备队
   │  ├─ diplomacyPanel.js # 外交: 宣战/阵营/和谈(显式 declare_war 入口)
   │  └─ # focusPanel.js / techPanel.js  ← 后续系统加在这
   │
   ├─ map/                 # ═══ 地图图层(每层一个函数, 注册到 canvas) ═══
   │  ├─ layout.js         # 省份坐标布局(10省对垒→后续可换真实坐标)
   │  ├─ layerTerrain.js   # 图层1: 地形底色 + 省界
   │  ├─ layerProvince.js  # 图层2: 省份着色(按 controller 政治色) + 选中高亮
   │  ├─ layerUnit.js      # 图层3: NATO 部队牌(兵种符号/org·str条/数量/国旗)
   │  ├─ layerOrder.js     # 图层4: 命令箭头(进攻/行军/支援/航点, 多段折线)
   │  ├─ layerCombat.js    # 图层5: 战斗气泡(交战省进度环 + VS)
   │  └─ # layerSupply.js  ← 后续:补给覆盖层
   │
   └─ ui/                  # ═══ 复用组件 ═══
      ├─ topbar.js         # 顶栏: 国旗/日期/速度/资源 + 系统按钮组 + [切控制权]测试按钮
      ├─ panelHost.js      # 通用面板容器: 滑入/滑出动画 + 标题栏 + 关闭
      ├─ drawer.js         # 底部抽屉(点省弹部队/命令, 移动端主交互)
      ├─ orderMenu.js      # 下令菜单(选师后点省弹: 进军/航点/支援)
      └─ statbar.js        # 状态条组件(org/str/eq/mp, 复用于牌子和面板)
```

### 扩展协议(后续加系统的标准动作)

- **加面板**:写 `views/xxxPanel.js` → `router.register('xxx', panel)` → 顶栏自动多一个按钮
- **加地图层**:写 `map/layerXxx.js` → `canvas.addLayer(name, z, fn)`
- **engine/core 永不动** —— 这就是"彻底改造适应后续系统"的骨架

### 原版映射(对齐依据)

| 我们的层 | 原版对应 |
|---|---|
| topbar + 系统按钮组 | `topbar.gui`(顶栏) + 顶栏右侧 production/tech/diplomacy/... 按钮组 |
| panelHost 滑入面板 | 所有 `country*view`(统一滑入:左侧、550px、300ms decelerated) |
| layerUnit NATO 牌 | `mapicons.gui` 的 `unit_counter`(76×24,兵种/org条/str条/国旗) |
| layerCombat 战斗气泡 | `landcombat.gui` 的 `landcombatview` |
| drawer 底部抽屉 | (原版无,移动端新增) |

---

## 3. 数据流:声明式绑定 + 引擎推送(§2 确认)

对齐原版 `scripted_gui` 的数据绑定模型。**不是** get_state 全量快照拉取,而是 UI 元素声明依赖 → 引擎推送变更。

### 3.1 引擎侧:数据视图序列化 + changeset

每次 tick 后,引擎序列化"数据视图"(按视图模型组织,非裸 dump),diff 上一帧产出 changeset:

```
World (运行态) ─tick─▶ World'
                          │ 序列化为数据视图
                          ▼
                    viewModel JSON
                    { date, player, wars, factions,
                      provinces:[...], divisions:[{id,template,stats,order}...],
                      templates:[...] }
                          │ diff 上一帧
                          ▼
                    changeset
                    { divisions.changed:[id...], wars.changed, date.changed }
                          │
                          ▼ (WASM→JS 单次推送)
```

viewModel 结构(对齐 `engine_get_state`,补字段):
```jsonc
{
  "hour": 0, "date": {"y":1936,"m":1,"d":1},  // 用 World.date() 派生
  "player": "GER",
  "wars": [{"id":1,"atk":["GER"],"def":["FRA"]}],   // 新增
  "factions": {"GER":"Axis","FRA":null},              // 新增
  "divisions": [...],   // 每师新增 template 字段
  "battles": [...],
  "provinces": [...]
}
```

静态数据(模板列表)走独立查询接口 `engine_get_templates()`(启动后不变,调一次缓存),不进每帧 changeset。

### 3.2 UI 侧:core/bind.js 绑定框架

`store.state` 持有完整 viewModel,接收 changeset 打补丁,通知所有订阅了变更路径的绑定。

5 个绑定原语(对齐原版 scripted_gui):

| 原版 | 我们的 JS | 实例 |
|---|---|---|
| `text="[Get...]"` | `bind(path, fn)` | org条数值实时显示 |
| `visible={trigger}` | `bindWhen(path, pred)` | 无命令师显示"⚠️"图标 |
| `triggers(enabled)` | `bindEnabled(path, pred)` | 资源不够则按钮灰掉 |
| `properties(frame)` | `bind(path, mapToFrame)` | 进度环、状态图标 |
| `dynamic_lists(array)` | `bindList(path, renderItem)` | 部队列表、模板下拉 |

### 3.3 性能:脏标记(对齐原版 dirty=var)

changeset 用"路径级脏标记"。tick 后只比对变化的子树。bind.js 收到 changeset → 标记受影响图层/元素。避免每帧全量序列化 + 全量 diff。

### 3.4 "换模板"完整数据流验证

```
玩家点"换模板"按钮
  └─▶ JS: engine.change_template(div, "Panzer-Division")
       └─▶ Rust: 师查新模板 → to_division_stats → 覆盖数值
            └─▶ tick: divisions[N] 整个对象变了
                 └─▶ changeset: divisions.changed=[N]
                      └─▶ 所有绑了 divisions[N].* 的元素自动刷新:
                           软攻/装甲/装备需求/兵种符号/宽度… 全部联动更新
                           (NATO牌子、部队面板、交战视窗 都绑了 divisions[N], 全刷新)
```

---

## 4. 引擎层改造(§2/§3 确认)

### 4.1 Division 加模板引用

**现状**:`create_division template=` 把模板数值"拍扁"进师(拷贝),师创建后与模板脱钩。

**改造**:`Division` 加 `template_name: Option<String>`(创建时记)。师↔模板从"值拷贝"改成"引用关系",对齐原版 `add_units_to_division_template` 的共享语义。

### 4.2 新增命令 change_template

```
change_template = { division = N template = "Infanterie-Division" }
```
→ 重新 `to_division_stats` 汇总 → 覆盖师的 soft_attack/armor/equipment_need/max_org... → **保留 location/org/strength**(运行态不动)。

### 4.3 新增命令 edit_template(可选,本次做)

```
edit_template = { template = "X" add_regiments = { infantry = 2 } }
```
→ 改模板本身 → 所有用该模板的师重汇总(对齐原版"改模板联动所有师")。

### 4.4 修 to_division_stats 静默丢弃未知营

**现状**(`template.rs:50-57`):`filter_map` 把 `sub_units` 里查不到的营**静默丢弃**(Panzer 的 6 个 light_armor 被丢光,armor 归 0 不报错)。

**修复**:`to_division_stats` 改返回 `(DivisionStats, Vec<String>)`(统计 + 告警列表);未知营 `eprintln!` 告警 + 跳过,不 panic(对齐 Paradox 容错哲学)。`create_division` 把告警透传到日志。

### 4.5 修模块汇总异常

**实测**:`light_tank_chassis` archetype `hardness=0.80`,但型号 `light_tank_chassis_2` 掉到 `hardness=0.00`、`soft_attack=0.0`。

**修复方向**:查 `equipment.rs::compute_equipment_stats` 和 `build_equipment`。怀疑型号未写数值时回退 archetype base_stats,但 archetype 的 hardness 没进 base_stats 提取(或被模块 multiply 清零)。

> **本项的诚实标注**:具体根因需在实现阶段读 `compute_equipment_stats` 代码确认,spec 不在此臆断结论。这是"实现阶段需定位并修复"的明确任务项,而非已设计好的修复。

### 4.6 数据补全

| 文件 | 来源 | 作用 |
|---|---|---|
| `src/data_raw/units/light_armor.txt` | 原版 `common/units/light_armor.txt` | 装甲营定义(need `light_tank_chassis` 60/营) |
| `src/data_raw/history/FRA.txt` | 原版 `history/units/FRA_1936.txt`(改名) | FRA 模板:`Division d'Infanterie` 等 |

`loader.rs::load_all` 加:
```rust
load_sub_units(&mut data, include_str!("../data_raw/units/light_armor.txt"));
load_templates(&mut data, include_str!("../data_raw/history/FRA.txt"));
```

底盘(`light_tank_chassis`)+ 模块(炮塔/装甲/引擎)**已齐全**,无需再补。

---

## 5. WASM 契约扩展(§2 确认)

| 新增/改造 FFI | 对应引擎命令 | demo 用途 |
|---|---|---|
| `engine_deploy_template(owner, loc, template)` | `create_division template=...` | **数据驱动建师**(替代旧 battalions 路径) |
| `engine_change_template(div, template)` | `change_template` | 换模板 |
| `engine_declare_war(attacker, defender)` | `declare_war` | 显式宣战(外交面板) |
| `engine_create_faction / join_faction` | 同名 | 阵营操作(测自动拉入) |
| `engine_white_peace(a, b)` | `white_peace` | 测停战 |
| `engine_get_templates()` | (读 GameData) | 返回模板名列表(部署面板下拉,一次性缓存) |
| `engine_get_state()` 改造 | (补字段) | date/wars/factions 进 JSON |

**保留但标注废弃**:`engine_deploy_division(owner,loc,equip,bn)`(旧 battalions 路径)保留兼容,demo UI 改用 `engine_deploy_template`。

---

## 6. Canvas 图层化(§4 确认)

### 6.1 拆 6 层(替代单一 drawMap 168 行)

| 层 | 文件 | 内容 | z序 |
|---|---|---|---|
| 地形底 | `map/layerTerrain.js` | 地形底色 + 省界 | 0 |
| 政治着色 | `map/layerProvince.js` | 按 controller 着色 + 选中高亮 | 1 |
| NATO 部队牌 | `map/layerUnit.js` | 兵种符号/org·str条/数量/国旗 | 2 |
| 命令箭头 | `map/layerOrder.js` | 进攻/行军/支援/航点,多段折线+车道偏移(沿用现有算法) | 3 |
| 战斗气泡 | `map/layerCombat.js` | 交战省进度环 + VS | 4 |
| UI 覆盖 | `map/layerOverlay.js` | 选中/拖框/tooltip | 5 |

### 6.2 core/canvas.js 管家

```js
CanvasHost {
  相机: { x, y, zoom }              // pan/zoom 状态
  图层: [{ name, z, draw, dirty }]
  addLayer(name, z, drawFn)         // 注册
  pan(dx,dy) / zoomTo(f, cx, cy)    // 相机变换
  worldToScreen(p) / screenToWorld(p) // 坐标转换(hit-test用)
  render() { clear(); applyCamera();
             for (layer of sortedByZ)
               if (layer.dirty || fullRedraw) layer.draw(ctx, view); }
  markDirty(layerName)              // bind.js 通知数据变 → 标对应层脏
}
```

**关键**:相机(pan/zoom)只在 CanvasHost 统一应用。各图层函数只管"画什么",坐标转换由 Host 的 `worldToScreen` 统一做。触屏平移/缩放手势只改相机状态,所有图层自动跟。

### 6.3 图层 ↔ 数据绑定联动

bind.js 收到 changeset → 标记受影响图层脏:
- `divisions.changed` → 标 layerUnit + layerOrder 脏(只重画这两层)
- `provinces.changed` → 标 layerProvince 脏
- `battles.changed` → 标 layerCombat 脏
- 未变的层不重画(对齐原版 dirty=var)

### 6.4 NATO 部队牌(对齐 mapicons.gui 的 76×24)

原版牌子含:兵种符号(type)/数量(count_txt)/org条(bar_org 竖)/str条(bar_str 竖)/国旗(flag)/选中高亮。我们用抽象兵种符号(步兵=▦/装甲=◆/炮兵=◎),省/师太多时自动聚合显示数量。牌子的 org/str 条 `bind` 到 `divisions[N].org/str` 实时更新。

---

## 7. 触屏适配(§5 确认)

### 7.1 手势方案(桌面手机同构)

| 组 | 手势 | 对应桌面 |
|---|---|---|
| 🗺️ 地图 | 单指拖=平移 / 双指捏=缩放 / 双指点=复位 | 拖拽/滚轮 |
| 🎯 选择下令 | 点省弹抽屉 / 选师→点省弹命令菜单(两段式) | 点选 |
| 🏗️ 面板 | 部署/系统按钮弹抽屉滑入 | 顶栏按钮 |
| ⏱️ 时间 | 底部命令栏大按钮 | 底部按钮 |

**两段式下令**:触屏无 hover,选中师必须显式可视化(金色高亮 + 抽屉显示详情),每步明确反馈。下令菜单从底部弹出(手指可达,不遮挡地图)。

### 7.2 控制权切换(测试用,非正式手势)

原 `dblclick` 切换控制权是上帝模式(原版是控制台命令),**移出正式手势**。改为顶栏独立按钮 `[切控制权]`,点了进入"选省切换"模式 → 点省切 GER/FRA。长按手势留空(暂不分配)。

### 7.3 触屏硬约束(写入 CSS)

- 最小点击目标 **44×44px**(Apple HIG / Material 规范)
- 命中检测半径从 26 → **44**(配合屏幕坐标转世界坐标,缩放后仍准)
- 取消所有 hover 依赖,改"点击即选中即显示"
- viewport meta:`width=device-width, initial-scale=1, user-scalable=no`(禁浏览器缩放,我们自己管 pinch-zoom)
- `touch-action: none` on canvas(防止浏览器默认手势干扰)

### 7.4 core/input.js 统一输入层

用浏览器原生 `PointerEvent` 统一鼠标+触屏+笔(已天然归一),一套逻辑同时服务。`down/move/up` + 手势识别(拖动阈值/捏合检测/双指检测)。

---

## 8. demo 接通新基础构造

### 8.1 默认 setup 脚本重写

替代过时签名(`create_province owner=` + `start_battle`):
- `create_state`(建抽象 State)+ `create_province state=`(省引用 State)
- 显式 `declare_war`(不再靠 start_battle 隐式宣战)

### 8.2 兵力配置(步兵 + 装甲对比)

GER:FRA = 多模板对战。GER `Infanterie-Division`(9步)+ `Panzer-Division`(4 轻坦营 light_armor + 2 摩托化 motorized);FRA `Division d'Infanterie`(9步)+ `Division Légère Mécanique`(含 light_armor)。

**约束**:Panzer-Division 用了 `motorized` 营,本次只补 light_armor(§4.6),motorized 未补 → 该营会被 `to_division_stats` 告警+跳过(§4.4 已修,不再静默归零)。装甲对比的核心(light_armor 营)能正常汇总,已足够测硬度/穿甲。motorized 营的缺失会有告警,但不阻断 demo。后续补 motorized 时告警自动消失。

### 8.3 宣战边界场景

显式测试战争关系:中立国不交战、阵营拉入、`are_at_war` 边界 —— 通过外交面板的显式 `declare_war` 触发。

### 8.4 换模板功能

部署面板 / 部队详情支持"换模板"(选师 → 选新模板 → `change_template`),验证数据流联动刷新。

---

## 9. 测试策略(§6 确认)

### 9.1 引擎层测试(Rust `#[test]`,加入现有 180 测试)
- `change_template`:师换模板后数值更新、location/org/strength 保留、template_name 字段正确
- `to_division_stats` 未知营:返回告警列表、跳过不 panic、已知营正常汇总
- 数据加载:补 light_armor + FRA 后,`load_all` 产出完整(装甲师 armor > 0)
- `edit_template`:改模板后所有引用师重汇总
- 模块汇总 bug:修复后 light_tank_chassis hardness/soft_attack 正确

### 9.2 WASM 契约测试(Rust 测 `wasm_api`)
- `engine_change_template` FFI 正确转发
- `engine_get_state` 新增 date/wars/factions 字段正确序列化
- `engine_get_templates` 返回完整模板列表

### 9.3 端到端验证(手动 + 浏览器,无自动化前端测试)
- 完整对战一局(部署步兵+装甲 → 宣战 → 战斗 → 换模板看数值变化)
- 触屏验证(手机浏览器:拖地图/捏缩放/两段式下令/底部抽屉)
- 性能验证(tick 时 UI 不卡,脏标记生效)

### 9.4 数据流验证(关键,对齐原版)
- 换模板后,NATO 牌子/部队面板/交战视窗**都自动刷新**(证明 changeset→bind 链路通)
- 只有师移动时,顶栏不重算(证明脏标记生效)

---

## 10. 约束与风险

- **工具链**:必须用 `stable-x86_64-pc-windows-gnu`(无 MSVC 链接器)
- **WASM FFI**:u64 参数在 JS 侧要 BigInt,用 u32 避免
- **WASM 更新后**:fetch 加 `?v=Date.now()` 防缓存
- **engine_tick**:必须用 `GameClock::advance`(完整主循环),不能内联
- **借用冲突**:get_mut 持有借用时不能再 `world.divisions.values()`,用快照→计算→写回模式
- **ES Modules**:需通过 http(s) 提供服务(不能 file://),现有 `python http.server` 满足
- **change_template 原子性**:重汇总数值 + 改 template_name 必须一起完成,不能中间状态

---

## 11. 实施顺序建议(供 writing-plans 参考)

1. **引擎层修复 + 数据补全**(§4):补 light_armor + FRA、修 to_division_stats、修模块汇总、加 template_name + change_template。跑测试到绿。
2. **WASM 契约扩展**(§5):新增 FFI、补 get_state 字段、engine_get_templates。
3. **UI 骨架**(§2 架构):建 web/js 四层目录 + core/* + main.js 装配。先跑通空壳(顶栏 + 空地图)。
4. **数据流**(§3):engine/state.js 序列化 + core/bind.js + store。
5. **Canvas 图层化**(§6):canvas.js 管家 + 6 图层,迁移现有 drawMap 逻辑(尤其行军箭头车道算法)。
6. **触屏 + 交互**(§7):input.js + 手势 + 抽屉 + 下令菜单两段式。
7. **面板内容**(§1 views/):部署(template)/ 外交(declare_war)/ 部队列表 / 交战视窗。
8. **demo 接通 + 端到端**(§8):重写 setup 脚本、跑完整对战、换模板验证。
