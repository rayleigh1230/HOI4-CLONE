# 地图视觉表现 & 部队牌/战斗可视化 改造设计

> 日期: 2026-06-25
> 状态: 已批准(头脑风暴 4 节全部确认),待实现
> 关联: `docs/design-principles.md`(原则1: 原版是首要参考)
> 关联: `docs/superpowers/specs/2026-06-25-demo-overhaul-design.md`(demo 架构地基, 本设计在其上)
> 参考来源: 原版客户端 `G:\steam\steamapps\common\Hearts of Iron IV`
>   - `interface/mapicons.gui`(`unit_counter` 76×24 NATO 牌 / `land_combat_mapicon` 战斗图标)
>   - `interface/landcombat.gui`(`landcombatview` 战斗详情弹窗 / `division_combat_attacker_entry` 师条目)

---

## 0. 背景与目标

### 现状问题

demo 改造后(2026-06-25)地图功能通了(13/13 验证),但**视觉表现和可视化**停留在抽象占位:
- 省份 = 26px 实心圆 + 蓝色虚线连邻接,全黑背景,**无地形感**
- 部队牌 = 只有兵种符号(▦/◆)+ 数量,**无 org/str 实时条、无完整 NATO 牌**(spec §6.4 要求对齐原版 76×24 但没实现)
- 战斗气泡 = 黑底方框 + "攻X VS 守Y",**简陋**,且与原版 `land_combat_mapicon`(带进度数字小圆)不符

### 目标

按用户确认的两个优先方向改造:
1. **视觉表现力**: 省份从抽象圆点改成**多边形拼图 + 地形底色**,引入固定世界坐标系
2. **部队牌 & 战斗可视化**: 部队牌对齐原版**完整 NATO 76×24 牌**;战斗地图指示对齐原版**带进度数字小圆**,点击弹**战斗详情面板**(landcombatview 风格)

### 范围

- **A. 世界坐标系 + 多边形布局**: 固定 1000×700 世界,手写 10 省多边形 + 地形类型
- **B. 地图图层重做**: terrain(地形底图)/province(政治描边)/overlay(选中+前线)从画圆改画多边形;命中改点在多边形内
- **C. 完整 NATO 部队牌**: layerUnit 重写,对齐 unit_counter(兵种+org/str竖条+数量+国旗色边框+牌堆合并)
- **D. 战斗可视化**: layerCombat 改带进度数字小圆(可点击);combatPanel 升级为 landcombatview 风格
- **E. 引擎补字段**: get_state 序列化补 soft/hard/defense/breakthrough/armor/piercing/combat_width(战斗详情面板要显示)

### 非目标

- 不接真实地图多边形(spec §非目标延续: 自定义手画多边形,非原版 province 轮廓)
- 不实现战术系统(landcombatview 的 tactics;简化为"相持/推进/撤退"阶段文字)
- 不实现将领系统(landcombatview 的 leader/skill;省略)
- 不动引擎战斗逻辑(resolve.rs 等),只补 get_state 的序列化字段

### 验证方法

调研了原版客户端 `mapicons.gui` + `landcombat.gui` 实物,各元素布局对齐原版定义(尺寸/位置/字段)。

---

## 1. 核心设计决策(头脑风暴确认)

| # | 决策 | 选择 |
|---|---|---|
| 1 | 省份呈现 | 自定义多边形布局(手画 10 块 + 地形底色) |
| 2 | 多边形数据来源 | layout.js 手写顶点坐标(每个省 5-7 点) |
| 3 | 部队牌密度 | 完整 NATO 牌(对齐 unit_counter 76×24) |
| 4 | 坐标系方案 | 方案 A: 固定世界坐标系(1000×700) + 相机变换 |
| 5 | 多师呈现 | 牌堆合并(对齐原版, 主牌显示合计 + 堆叠角标, 详情走抽屉) |
| 6 | 战斗图标 | 带进度数字小圆(对齐 land_combat_mapicon) |
| 7 | 战斗详情 | 点击战斗图标弹独立面板(landcombatview 风格) |
| 8 | 点击分流 | 战斗图标独立点击区, 命中优先级最高 |

---

## 2. 世界坐标系 + 多边形布局(§1 决策 1/2/4)

### 2.1 固定世界坐标系

引入 **1000(宽)× 700(高)** 虚拟世界。所有多边形顶点、部队牌位置、命令箭头、战斗图标都用**世界坐标**。相机(已有 `core/canvas.js`)把世界坐标变换到屏幕坐标:

```
世界坐标 (1000×700 固定)
   │ canvas.worldToScreen(p) = { x: p.x*zoom + camX, y: p.y*zoom + camY }
   ▼
屏幕坐标 (随窗口/缩放变)
```

窗口尺寸变化只影响"世界→屏幕映射比例",世界本身不变 → **缩放/平移下布局不变形**。当前 `layout.js:provincePos` 用视口 W×H 当世界坐标的问题(随窗口变)一并解决。

### 2.2 布局数据模型(重写 layout.js)

```js
// layout.js — 10 省多边形(手写顶点, 世界坐标), 替代旧的 provincePos
export const WORLD_W = 1000, WORLD_H = 700;

export const PROVINCES = {
  1: { terrain: 'plains',  poly: [[x1,y1],...], centroid: [cx,cy] },
  // ... 10 个省
};

export const TERRAIN_COLORS = {
  plains: '#3a5a40', forest: '#2d4a2b', hills: '#5a4a3a',
  urban: '#4a4a4a', mountain: '#6b5b4a',
};

// 省份重心(部队牌/选中高亮/战斗图标画这里; 离线预算存 PROVINCES, 不每帧算)
export function provinceCentroid(id) { return PROVINCES[id]?.centroid; }
// 多边形顶点(世界坐标)
export function provincePoly(id) { return PROVINCES[id]?.poly; }
```

### 2.3 多边形设计原则

- 上下两排各 5 块(上 GER y 小, 下 FRA y 大), 每块约 200×300 区域
- 块间**共享边**对齐: neighbors 里相邻的省(1↔2/6/7 等)多边形真的有共享边(视觉接壤)
- 顶点手画: 我会先在草稿上定坐标, 保证 10 块拼满世界、共享边吻合 neighbors
- centroid = 多边形重心(离线算好存入, 不每帧重算)

### 2.4 相机适配

`canvas.js` 相机已有 pan/zoom/worldToScreen/screenToWorld。改 layout 后:
- 相机初始定位: 让世界(1000×700)按屏幕宽高比 fit 到屏幕居中(类似 `camera` 初始化时算 fit 缩放)
- `main.js` 命中检测: `screenToWorld(点击点)` → 世界坐标 → 判断在哪个多边形内

---

## 3. 地图图层重做(§1 决策 4)

### 3.1 layerTerrain.js(图层 0)——地形底图

遍历 10 省, 按 `terrain` 用 `TERRAIN_COLORS` **填充多边形**(`ctx.fill()`),形成彩色拼图底图, 替代全黑背景:
- 多边形之间共享边对齐, 拼满整个世界
- 加纹理感: 填充后叠一层低透明度静态噪点/等高线(预渲染到 offscreen canvas 一次, 避免每帧重算)
- 海洋/边界: 世界外侧(多边形之外)填深色作"地图边框"

### 3.2 layerProvince.js(图层 1)——政治着色 + 省界

**不再整块填 controller 色**, 改为:
- 多边形**描边** = controller 色(GER 红 `#e94560` / FRA 绿 `#16c79a`), 线宽 2px, 归属一眼可见
- 多边形**填充**叠一层很淡的 controller 色(alpha≈0.18)在地形色上, 地形底色仍透出
- **省界**: 相邻省共享边用稍亮线描一次(区分省界和地图边缘)
- 省号文字画在重心

### 3.3 layerOverlay.js(图层 5)——选中 + 前线

- 选中省: 金色加粗描边(沿多边形描边, 非圆环)+ 重心金色标签(已有逻辑, 适配多边形)
- **前线高亮**(新增): controller 不同的相邻省之间的共享边, 用红色脉冲线标"交战前线"

### 3.4 命中检测改造(main.js)

- 现 `nearestProvince` 用"最近圆心 < 44", 改成**点在多边形内**(射线法 `pointInPolygon(worldPoint, poly)`)
- 点击拿屏幕坐标 → `screenToWorld` → 世界坐标 → 遍历省多边形判断包含关系
- 缩放后仍准(世界坐标判定, 与缩放无关)

---

## 4. 完整 NATO 部队牌(§1 决策 3/5)

对齐 `mapicons.gui` 的 `unit_counter`(76×24)。这是用户明确要的"完整 NATO 牌"。

### 4.1 单个师牌子结构(76×24 世界单位)

```
┌──────────────────────────────┬──┐
│ 兵种符号  │org│str│  count   │旗 │   ← 对齐 unit_counter:
│  (16宽)  │竖条│竖条│ (数字)  │色 │      type + bar_org + bar_str
└──────────────────────────────┴──┘      + count_txt + flag
   ←──────── 60 ──────────────→←16→
```

| 元素 | 原版字段 | 我们的实现 |
|---|---|---|
| 兵种符号 | `type`(GFX 兵种 icon) | ▦步兵/◆装甲/◎炮兵/◇机械化(按 template 名推断) |
| org 竖条 | `bar_org`(rotation 1.5708=90°竖向) | 绿色竖条, 高度=org/max_org, 实时绑 `divisions[N].org` |
| str 竖条 | `bar_str` | 红色竖条, 高度=str/max_str, 实时绑 |
| 数量 | `count_txt`(单数字) | 该堆师数(单数字) |
| 国旗色边框 | `flag`/`large_flag` + `count_selected` | 牌框描边 = owner 国旗色(GER红/FRA绿) |

### 4.2 同省多师 = 牌堆合并(对齐原版)

同省多师**合并成一张牌堆**(原版行为), 不各画各:
- 主牌显示该省驻军**合计**: 兵种符号(取主力兵种)+ count(堆叠数)+ org/str 竖条(取堆内平均)
- 堆叠角标: 右下小角标显示堆叠数(如 "③")
- 详情走抽屉(已有 drawer): 点省份弹抽屉展开每个师
- 理由: HOI4 原版同省多师叠成堆, 避免满屏牌子

### 4.3 位置 + 状态

- 牌子画在该省**重心上方**(重心画省号, 牌子上方不遮邻接)
- 撤退中师(retreating)牌子加灰色半透明蒙版
- 战斗中师(atk/def 数组里)牌子加**红色描边**(第 5 节)

### 4.4 实时绑定(对齐 spec §3.3 脏标记)

org/str 条随 tick 实时变。layerUnit 订阅 `divisions` key 变化 → markDirty('unit') → 重画牌子。复用刚修好的路径级脏标记, 不每帧全画。

---

## 5. 战斗可视化(§1 决策 6/7/8)

对齐 `mapicons.gui:land_combat_mapicon` + `landcombat.gui:landcombatview`。

### 5.1 地图战斗图标(layerCombat.js)——带进度数字小圆

交战省(被进攻的防守省)重心处画**带进度数字的小圆图标**(对齐 `land_combat_mapicon`, **去掉方向箭头**——方向是 layerOrder 进军箭头的事):

```
    ┌─────┐
    │ 67% │   ← 圆形/盾牌底(红描边) + 进度数字
    │▓▓▓░░│   ← 下方细进度条(视觉强化)
    └─────┘
```

- 圆形底 + 红色描边(有战斗时); 圆心显示**进度数字**(攻方 org 损耗比 %)
- 进度条: 圆下方一条细条, 填充比例 = 进度
- **可点击**: 命中半径内点击 → 打开战斗详情面板(命中优先级最高, 第 5.4)
- 无战斗的省不画

### 5.2 进军箭头(layerOrder.js)——已有, 不归战斗层

行军/进攻箭头保持现状(多段折线 + 车道偏移 + 进度尾端)。它和战斗图标是**两个独立图层**, 对应地图上箭头 + 小圆两个元素。

### 5.3 战斗详情面板(combatPanel.js 升级)——landcombatview 风格

点击战斗图标 → 打开"交战"面板(router 已注册), 内容对齐 `landcombatview`:

```
┌─ ⚔ 省7 交战 ──── 进度 ▓▓▓░░ 67% ─┐
│ 宽度: 60                           │
│                                    │
│ 攻方(GER)        守方(FRA)         │
│ 2 Divisions      1 Divisions       │
│ 1 Reserves       0 Reserves        │
│ ┌────────┐       ┌────────┐        │
│ │◆ org▓str▓│     │▦ org▓str▓│      │  ← 师条目(对齐 division_combat_entry)
│ │ SA12 HA3 │     │ SA8  HA1 │      │     兵种 + org/str竖条 + soft/hard/defense
│ │ DEF10    │     │ DEF15    │      │
│ └────────┘       └────────┘        │
│ 阶段: 推进中                       │
└────────────────────────────────────┘
```

- 顶部: 省名 + 进度条 + 战斗宽度
- 左攻/右守两栏, 每栏: "N Divisions" + "N Reserves" + 师列表
- 师条目(对齐 `division_combat_attacker_entry`): 兵种符号 + org竖条 + str竖条 + soft_attack/hard_attack/defense 数值
- 中间战术区简化为阶段文字(相持/推进/撤退; 原版 tactics 系统不实现)

### 5.4 命中优先级(input.js / main.js)

点击地图命中检测从上到下(上层消费后停止):
1. **战斗图标**(交战省重心的小圆, 命中半径内)→ 打开战斗面板
2. **省份多边形**(点在多边形内)→ 驻军抽屉 / 两段式下令

战斗图标叠在省上面, 点击优先吃掉; 点省其他区域才走省份逻辑。

---

## 6. 引擎层补字段(§1 决策 - 支撑战斗详情面板)

战斗详情面板要显示师的 soft/hard/defense/breakthrough/armor/piercing/combat_width, 当前 `get_state` 没序列化这些。

### 6.1 wasm_api.rs get_state 补字段

`serialize_state` 的 division 序列化(现 format! 字符串)追加:
- `soft_attack`, `hard_attack`, `defense`, `breakthrough`, `armor`, `piercing`, `combat_width`

这些字段 Division struct 本就有(combat 模块用), 只是没进 get_state JSON。补序列化即可, **不改引擎逻辑**。

### 6.2 进度数据(不改引擎)

战斗进度用**方案 A: 前端现算攻方 org 损耗比**:
```
进度 = (Σ攻方 max_org - Σ攻方 org) / Σ攻方 max_org
```
从 atk 数组的 divisions 现算, 不改引擎。demo 阶段够用(真实战斗推进度需引擎改, 留后续)。

---

## 7. 约束与风险

- **工具链**: stable-x86_64-pc-windows-gnu(沿用)
- **WASM FFI**: 补字段不涉及新 FFI, 只改 serialize_state 的 format 字符串
- **借用冲突**: serialize_state 只读 world, 补字段不引入新借用(读 Division 已有字段)
- **坐标一致性**: 所有图层必须用世界坐标(经 worldToScreen 变换); main.js 命中必须 screenToWorld 转世界坐标再判断。混用会导致点击/渲染错位
- **多边形顶点手画**: 10 省 60+ 顶点坐标, 需仔细对齐共享边(neighbors); 建议先草稿定坐标再写代码
- **offscreen 纹理**: 地形噪点预渲染到 offscreen canvas, 避免每帧重算; 注意 worldToScreen 变换时纹理也要跟着缩放(或固定世界大小只画一次, 缩放时用 drawImage 缩放)

---

## 8. 实施顺序建议(供 writing-plans 参考)

1. **引擎补字段**(§6): get_state 补 soft/hard/defense 等, 重编 wasm。跑测试 + web_demo.mjs 确认新字段出现
2. **世界坐标系 + 多边形布局**(§2): 重写 layout.js(PROVINCES 多边形 + 地形 + 重心), 相机初始 fit。先跑通空地图(多边形拼图渲染)
3. **图层重做**(§3): terrain/province/overlay 从画圆改画多边形; 命中改 pointInPolygon。验证 10 省拼图 + 选中 + 点击命中
4. **完整 NATO 牌**(§4): layerUnit 重写(兵种+org/str竖条+数量+国旗边框+牌堆合并)。验证牌子实时更新
5. **战斗图标 + 详情面板**(§5): layerCombat 改带进度数字小圆(可点击); combatPanel 升级 landcombatview 风格; 命中优先级
6. **端到端验证**: 跑完整对战(部署→宣战→战斗), 战斗图标可点击弹详情; web_demo.mjs 扩展验证项; 触屏验证
