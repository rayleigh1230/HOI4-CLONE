# hoi4-clone 项目交接文档

> **用途**: 在新会话中继续开发。读本文件 + 列出的关键文件即可接上。
> **更新**: 2026-06-23(多段路径行军 + 航点规划 + 支援攻击邻接收敛 + 路径失效应对)

---

## 0. 项目概况

完整复刻 HOI4 核心机制的自制游戏, 分享给朋友玩。
- **技术栈**: Rust → WASM + 单文件 HTML/JS 前端
- **位置**: `G:\projects\hoi4-clone\`
- **运行**: `cargo test`(测试) / `cargo run --bin hoi4_demo`(CLI) / 浏览器 `http://127.0.0.1:8765`(UI demo)
- **工具链**: `stable-x86_64-pc-windows-gnu`(rustup override 绑定)
- **规模**: ~5500 行 Rust + UI, **117 测试**, 315KB WASM

---

## 1. 已完成的里程碑

| M | 内容 | 测试 | tag |
|---|---|---|---|
| M1 | 脚本引擎(parser/AST/interpreter) | 19 | m1-complete |
| M2 | 运行时重构(Registry/Result/主循环) | 25 | m2-complete |
| M3 | 实体+作用域+陆战伤害 | 38 | m3-complete |
| M4a | 装备系统(库存/消耗/增援) | 42 | m4a-complete |
| 陆战循环 | 四量模型+撤退+歼灭+行军+包围+宽度+预备队+带溃 | 73 | (分支) |
| 多战场+指令 | 多战场伤害累积+支援攻击+停止+防守撤退+预备队时机 | 99 | (阶段) |
| 状态机重构 | OrderState enum 替代 7 扁平字段 + 4 条规则对齐 + 瞬移根治 | 101 | (阶段) |
| **多段路径行军** | **自动寻路 + 航点规划 + 支援攻击邻接收敛 + 路径失效应对** | **117** | **(本阶段)** |

### 本阶段核心改动: 多段路径行军(2026-06-23)

**新增能力**: 师可沿多省路线逐段推进(点远处省自动 BFS 寻路), 支持追加航点。

**数据结构**: `OrderState::Moving` 加 `remaining: Vec<u32>`(dest 走完之后还要去的省, 不含 dest); `OrderState::Pending` 也加 `remaining`(战斗胜后续走)。单段移动 = remaining 空, 行为不变。

**新模块 `src/combat/pathfinding.rs`**: BFS 寻路 + 双插槽
- `is_passable`(现在恒 true): 未来加"未开战不得入境""绕开驻军省"只改此函数
- `edge_weight`(预留, 现权重全1=BFS): 未来加省份距离数据 → 自动升级为 Dijkstra(距离之和最短)

**新增命令**:
- `move_division`(改造): 下单时 BFS 寻路填 remaining; 战败/停止路径自动取消(Moving 变 Idle/Retreating)
- `queue_move`(新): 追加航点到路径末尾(手机端友好, 无需 shift); Idle 时等同 move_division
- `support_attack`(收敛, 决策13): 目标省须与师 location 相邻, 否则静默无效

**行军推进**(`advance_movement`):
- 到达中途省(占领)→ 检查 remaining 续走下一段(dest=下站, progress=0)
- Pending 战斗胜利占领 → 也续走
- 决策14 路径失效应对: dest 不可进入(`is_passable` 返回 false)→ 师停止(转 Idle); 强制中止函数 `invalidate_paths_to_inaccessible`(供未来投降/停战事件批量调用)

**WASM**: 新增 `engine_queue_move` FFI; 序列化零改动(remaining 不暴露给 JS, 前端只画当前段)。

**设计文档**: `docs/superpowers/specs/2026-06-23-multi-hop-marching-design.md`(14 条决策)
**实现计划**: `docs/superpowers/plans/2026-06-23-multi-hop-marching.md`(11 Task)

### 本阶段核心改动: OrderState 状态机(2026-06-22)

**重构前**: Division 有 7 个扁平行动字段(retreating/destination/move_progress/attacking/origin_province/pending_arrival/supporting),状态转换规则散落在 3 个文件,反复 fix(land) 20+ 次。

**重构后**: 单一 `order: OrderState` 字段,5 个变体:
```rust
enum OrderState {
    Idle,
    Moving { dest, progress, hostile, origin },  // 主动行军(绿/红)
    Retreating { dest, progress },                // 撤退(独立判定, 对其他系统不可见)
    Pending { dest },                             // 到达但战斗未胜, 等占领
    Supporting { target },                        // 支援攻击(不移动)
}
```

**4 条状态机规则(宪法, 详见 `docs/specs/2026-06-22-order-state-semantics.md`)**:
1. **进军/移动同一指令两种状态**: dest 归属地己方+无敌军→移动; 非己方或有敌军→进军
2. **每小时索敌**(check_engagements): 移动遇敌→开战; 归属地被攻→自动当守方
3. **归属地变更需**: 进度满 100% **且** 自身未处于任何战场(进度满时检查师自身在不在 battle 列表)
4. **撤退仅邻省**: Retreating 状态独立判定, 只去邻近省份

**战败区分(关键)**:
- 攻方战败 + 归属地仍己方 → **瞬间回归属地**(转 Idle, 不是 Retreating)
- 攻方战败 + 归属地已丢 → 进 Retreating 撤向邻省
- 守方战败 → 进 Retreating 撤向邻省

**撤退到达独立判定**(Retreating 组, 不套用 Moving 逻辑):
- 邻省己方/敌方无敌军 → Capture(归属+Idle; 敌方空省占领)
- 邻省敌方有敌军 → RetreatIntoEnemy(强制归属+进入战场)

**瞬移 bug 根治**: 之前用"撤退瞬间改 location"hack 修瞬移,有副作用。新语义下 **Pending 不改 location**(师始终显示在归属地),战败"回归属地"=原地转 Idle,自然无瞬移。

### 陆战已实现的核心机制(完整)

**四量模型与伤害**
- 四量模型: 组织度(Org) / HP / 装备 / 人力 独立
- 伤害: 骰子(d4/d2) + 防御池(10%/40%命中) + 装甲碾压 + 穿甲系数
- **多战场伤害累积**(P1-6): 同一师参与多场战斗, 伤害 delta 累加(非覆盖)

**战斗生命周期**
- 撤退(org归零+HP有余) / 歼灭(HP归零) / 包围歼灭(无邻省)
- 带溃(前线崩→预备队强制撤退)
- **Retreating 对其他系统不可见**: check_engagements 完全忽略(不当攻方也不当守方)

**战斗指令(4种, 已完整)**
- **移动/进军**(move_division): 点选移动, 目标有敌军→进攻(红箭头), 无敌军→普通移动(绿)
- **支援攻击**(support_attack): 师不移动, 作为攻方远程参战(蓝箭头); 下单时目标省须已有战斗
- **停止**(stop_order): 取消主动行动(进军/移动/支援), 保留被动防守和撤退
- **防守主动撤退**(move_division 到相邻己方省): 战斗地块下移动到己方省→撤退状态

**战斗宽度与预备队**
- 战斗宽度(70) + 预备队 + 补位(2%/h)
- **预备队判定时机**(started 标志): 部署阶段(started=false)同方向都进前线; 游戏开始后同 origin 后到的进预备队
- 同出发地判定(Moving.origin), 宽度限制始终生效

---

## 2. 当前代码结构

```
src/
├── parser/       lexer + block(脚本→AST树) + List(裸值列表)
├── ast/          effect/trigger/lower(Block→Effect/Trigger)
├── runtime/
│   ├── entities.rs   Province/Country/Division/Battle/Scope + OrderState enum
│   │                 Division.order: OrderState(替代旧 7 扁平字段)
│   │                 派生方法: is_moving/is_withdrawing/is_pending/is_idle/
│   │                 move_dest/retreat_dest/pending_dest/move_origin/move_progress
│   ├── world.rs      World状态 + scope_stack + 实体管理 + started标志
│   ├── interp.rs     Interpreter(run/eval + run_for_each作用域枚举)
│   ├── registry.rs   Registry(effects/triggers) + ParamGet
│   ├── clock.rs      GameClock::tick(主循环)
│   └── error.rs      CmdError
├── combat/
│   ├── resolve.rs    陆战结算(骰子/防御池/装甲) + cleanup(撤退/歼灭/带溃/占地)
│   │                 多战场伤害 delta 累积; 攻方战败区分(归属地己方→Idle/丢→Retreating)
│   ├── movement.rs   check_engagements(过滤 Retreating) + cancel_finished_supports
│   │                 + advance_movement(进度推进+到达判定, 规则3: 自身在战场不结算归属)
│   │                 到达分支: Moving→Capture/Pending; Retreating→Capture/RetreatIntoEnemy
│   ├── width.rs      战斗宽度 + reinforce_reserves(预备队补位2%/h)
│   ├── recovery.rs   组织度恢复(读 OrderState, Moving敌方掉/Retreating恢复/Idle恢复)
│   ├── reinforce.rs  装备+人力增援(每日, 排除 Moving/Retreating)
│   ├── commands.rs   create_division/start_battle/move_division(含邻接检查)/
│   │                 support_attack/stop_order + join_as_attacker(共用)
│   └── equipment_data.rs  5种1936装备真实数值
├── commands/     vars/control/scope命令注册
├── wasm_api.rs   WASM桥接(序列化时 OrderState 拍平为原 6 JSON 键, JS 零改动)
└── lib.rs / main.rs
web/
└── index.html    UI(部署面板+Canvas节点图+交战视窗+弹菜单+状态条+进度箭头)
```

### 主循环顺序(clock.rs 每小时)
```
1. hour += 1; started = true(首次tick)
2. fire_event(on_hourly)
3. check_engagements        — Moving/Pending师遇敌→开战(过滤Retreating)
4. cancel_finished_supports — 支援目标省战斗结束→清Supporting
5. resolve_all_battles      — 战斗结算(伤害累积+撤退+歼灭+带溃+占地)
6. reinforce_reserves       — 预备队补位(2%/h)
7. advance_movement         — 进度推进 + 到达判定(规则3: 自身在战场不结算)
8. recover_org              — org恢复/损耗(读OrderState)
```

---

## 3. 战斗指令完整参考

### 3.1 四种指令

| 指令 | 命令 | 语义 | 触发条件 |
|---|---|---|---|
| 移动/进军 | `move_division = { division target }` | 师移动到目标省 | dest有敌军→进攻(红); 无敌军→普通移动(绿) |
| 支援攻击 | `support_attack = { division target }` | 师不移动, 远程参战(蓝) | **下单时目标省须已有战斗**, 否则无效 |
| 停止 | `stop_order = { division }` | 取消主动行动 | Moving/Supporting 可停; Retreating/Pending 不可 |
| 防守撤退 | `move_division`(到相邻己方省) | 战斗地块下移动→撤退 | 师在战斗地块 + 目标相邻己方省(规则4) |

### 3.2 关键判定逻辑

**move_division 的分支**(commands.rs, 按顺序):
1. 师在战斗地块 + 目标**相邻**己方省 → **防守撤退**(转 Retreating, 退出战斗)
2. 否则 → **Moving**(dest=target, origin=cur_loc, hostile=目标非己方)

**stop_order 语义**(取消主动, 保留被动):
- Retreating/Pending → 忽略(不可停)
- Moving/Supporting → 转 Idle + 从 attackers/reserve_attackers 移除
- **不动** defenders/reserve_defenders(被动防守继续)

**support_attack**:
- 下单时无战斗 → 静默无效(蓝箭头不出现)
- 有战斗 → 转 Supporting, 加入攻方(复用 join_as_attacker), 不移动
- 战斗消失 → cancel_finished_supports 自动转 Idle
- 战败 → 按攻方战败规则(归属地己方→Idle)
- 不占地(location≠目标省)

### 3.3 预备队判定(join_as_attacker)
- **部署阶段**(started=false): 同方向(同 origin)都进前线
- **游戏开始后**(started=true): 同 origin 后到的进预备队
- **宽度限制**: 始终生效(超宽进预备队, 与 started 无关)
- origin 取值: Moving 用其 origin 字段; 其它(支援/守方转攻)用 location_province

---

## 4. 状态机语义(宪法)

详见 `docs/specs/2026-06-22-order-state-semantics.md`

**Moving 组(进军/移动)**:
- 进度满 + dest己方/无敌军 → Capture(归属+Idle; 敌方空省占领+org掉血)
- 进度满 + dest有战斗/敌军 → Pending(**不改 location**, 规则3)
- 进度满 + **师自身在战场**(in_battle) → 不结算(保持Moving, 等战斗结束)
- 途中每小时索敌: dest出现敌军→开战; 归属地被攻→自动当守方

**Pending 处理**:
- 战斗胜+无敌人 → location=dest(占领), Idle
- 战斗败 → 转入战败处理

**Retreating 组(撤退, 独立判定)**:
- 进入途径: 守方战败 / 攻方战败(归属地丢) / 玩家主动下令
- 只去邻省(规则4)
- 到达邻省: 己方/敌方无敌军→Capture; 敌方有敌军→RetreatIntoEnemy(强制归属+开战)
- RetreatIntoEnemy 战胜→占领; 战败→找周围己方撤退; 无→歼灭

**战败统一规则**:
- 攻方战败 + 归属地己方 → 瞬间回归属地(Idle)
- 攻方战败 + 归属地丢 → Retreating 撤邻省
- 守方战败 → Retreating 撤邻省
- 无邻省 → 歼灭(包围)

---

## 5. 未实现的交战规则(后续按优先级)

| 规则 | defines值 | 优先级 |
|---|---|---|
| 堆叠惩罚 | COMBAT_STACKING_START=5, -2%/师 | 中 |
| 超宽惩罚 | OVER_WIDTH -1%/%, max -33% | 中 |
| 堑壕 | DIG_IN_FACTOR=0.02, CAP=5 | 中 |
| 多方向被攻 | MULTIPLE_COMBATS_PENALTY=-0.5 | 中 |
| 将领技能 | 攻/防/计划 | 低(无将领系统) |
| 渡河/要塞 | RIVER -30/-60%, FORT -15% | 低(无地形系统) |
| CAS空中支援 | AIR_SUPPORT_BASE=0.25 | 低(无空军) |
| 战术系统 | TACTIC_SWAP=12h | 低 |
| 计划加成 | PLANNING_MAX=0.3 | 低 |

### 多段路径行军(寻路)设计要点(已实现 2026-06-23)
详见 `docs/superpowers/specs/2026-06-23-multi-hop-marching-design.md`(14 条决策)。
- Moving/Pending 加 `remaining: Vec<u32>`(dest 走完之后还要去的省)
- 用户点远处省 → BFS 寻路 → 逐段推进, 到达中途省占领后续走
- 途中遇敌开战, 胜后继续剩余路径(Pending 加 remaining)
- 航点规划: `queue_move` 追加航点(手机端友好, 无需 shift)
- 路径失效应对: dest 不可进入则停止 + `invalidate_paths_to_inaccessible` 强制中止函数
- 前端只画当前段(零改动)

---

## 6. 后续系统(待做)

- **生产系统**: 工厂→IC→生产线→产装备(现在装备靠"补给"按钮凭空给)
- **AI对手**: FRA自动部署/防守/反击
- **国策系统**: 加载核心国策, 点国策触发效果
- **扩展地图**: 从3省扩展到几十省
- **海军/空军**: 简化为制海权/制空权数值

---

## 7. 新会话怎么接上

1. 在 `G:\projects\hoi4-clone\` 开新对话
2. 读本文件 + `docs/specs/2026-06-22-order-state-semantics.md`(状态机宪法)
3. 看 `git log --oneline` 了解最近改动
4. 跑 `cargo test` 确认基线(**101测试**)
5. 从 §5/§6 选下一步做

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
- HashMap多可变借用: 用快照→计算→写回模式(advance_movement 的到达判定尤其注意)
- **撤退师过滤**: check_engagements/move_division 查敌军时都要 `!od.retreating`, 否则撤退师被重拉入战斗
- **借用冲突**: get_mut 持有借用时不能再 world.divisions.values(), 用局部作用域释放

---

## 8. 本阶段(2026-06-21~22)完成的提交

| 提交 | 内容 |
|---|---|
| 9d4d8a0 | P1-6 多战场伤害累积(非覆盖) + P2 归属地防守回归测试 |
| 229a8e9 | P3 进攻失败瞬间回 origin_province |
| 26f0d2a | P4 UI部队位置=归属地 + pending_arrival虚线箭头 |
| 6586c01 | UI 自动战斗→时间流逝开关(修无战斗不流逝) |
| f61b9db | UI 行军进度填充箭头 + 撤退灰色 + 交战面板4状态条 |
| 03122da | fix 撤退师不被重新拉入战斗(org归零后不再掉血至歼灭) |
| b4da174 | fix 撤退到达敌方驻军省应变攻方开战(非直接占领) |
| e09fc21 | fix 战败撤退瞬间归属地变目的地(防瞬移BUG) |
| 9bc01c1 | feat 支援攻击(support_attack)— 师不移动的远程攻击 |
| d0dc0d9 | feat 停止命令(stop_order)— 取消主动行动保留被动防守 |
| 353a0ca | feat 防守主动撤退 — 战斗地块下移动到己方省变撤退 |
| 2a7207a | feat 预备队判定时机 — 部署阶段同方向都进前线 |

## 8b. 本阶段(2026-06-23)完成的提交 — 多段路径行军

| 提交 | 内容 |
|---|---|
| 033876e | refactor Moving 加 remaining 字段(多段路径地基, 单段行为不变) |
| 2073b1e | feat pathfinding.rs BFS 寻路模块 + is_passable 插槽 |
| f2d4b2f | feat move_division 接入 BFS 寻路 + 边界B/C 处理 |
| 8554d23 | feat 到达中途省后续走(决策5) — Capture 后检查 remaining |
| 91d2b6c | feat Pending 战斗胜利后续走 — Pending 加 remaining 字段 |
| e6d0f97 | feat support_attack 邻接收敛(决策13) — 只能相邻省发起 |
| 2067e1d | feat queue_move 航点追加命令(决策9) — 手机端友好 |
| bfd67e7 | feat 路径失效应对(决策14) — dest 不可进入则停止 + invalidate 函数 |
| 12d2502 | feat wasm engine_queue_move FFI |
| 41f4906 | test 多段行军边界测试补全(决策11/12) |
