# hoi4-clone 项目交接文档

> **用途**: 在新会话中继续开发。读本文件 + 列出的关键文件即可接上。
> **更新**: 2026-06-22(陆战基本战斗指令逻辑完整, 阶段总结)

---

## 0. 项目概况

完整复刻 HOI4 核心机制的自制游戏, 分享给朋友玩。
- **技术栈**: Rust → WASM + 单文件 HTML/JS 前端
- **位置**: `G:\projects\hoi4-clone\`
- **运行**: `cargo test`(测试) / `cargo run --bin hoi4_demo`(CLI) / 浏览器 `http://127.0.0.1:8765`(UI demo)
- **工具链**: `stable-x86_64-pc-windows-gnu`(rustup override 绑定)
- **规模**: ~5000 行 Rust + UI, **99 测试**, 296KB WASM

---

## 1. 已完成的里程碑

| M | 内容 | 测试 | tag |
|---|---|---|---|
| M1 | 脚本引擎(parser/AST/interpreter) | 19 | m1-complete |
| M2 | 运行时重构(Registry/Result/主循环) | 25 | m2-complete |
| M3 | 实体+作用域+陆战伤害 | 38 | m3-complete |
| M4a | 装备系统(库存/消耗/增援) | 42 | m4a-complete |
| 陆战循环 | 四量模型+撤退+歼灭+行军+包围+宽度+预备队+带溃 | 73 | (分支) |
| 多战场+指令 | 多战场伤害累积+支援攻击+停止+防守撤退+预备队时机 | **99** | (本阶段) |

### 陆战已实现的核心机制(完整)

**四量模型与伤害**
- 四量模型: 组织度(Org) / HP / 装备 / 人力 独立
- 伤害: 骰子(d4/d2) + 防御池(10%/40%命中) + 装甲碾压 + 穿甲系数
- **多战场伤害累积**(P1-6): 同一师参与多场战斗, 伤害 delta 累加(非覆盖)

**战斗生命周期**
- 撤退(org归零+HP有余) / 歼灭(HP归零) / 包围歼灭(无邻省)
- 带溃(前线崩→预备队强制撤退)
- **撤退不被重新拉入战斗**: retreating 师被 check_engagements 完全忽略(不当攻方也不当守方)
- **撤退瞬间归属地变更**: 战败撤退时 location 改成撤退目的地(防"原省被夺回→瞬移"BUG)
- **撤退到达敌方驻军省**: 变攻方开战(非直接占领); 战败继续撤退

**战斗指令(4种, 已完整)**
- **移动/进军**(move_division): 点选移动, 目标有敌军→进攻(红箭头), 无敌军→普通移动(绿)
- **支援攻击**(support_attack): 师不移动, 作为攻方远程参战(蓝箭头); 下单时目标省须已有战斗
- **停止**(stop_order): 取消主动行动(进军/移动/支援), 保留被动防守和撤退
- **防守主动撤退**(move_division 到己方省): 战斗地块下移动到己方省→撤退状态

**战斗宽度与预备队**
- 战斗宽度(70) + 预备队 + 补位(2%/h)
- **预备队判定时机**(started 标志): 部署阶段(started=false)同方向都进前线; 游戏开始后同 origin 后到的进预备队
- 同出发地判定(origin_province), 宽度限制始终生效

**占领规则**
- 到达=进度满+战斗胜(两者满足才结算归属)
- 战斗中绝不改归属; 无战斗+无敌军+非己方→占领
- 支援攻方不占地(location≠目标省)
- org损耗: 敌方地块行军掉, 己方恢复

---

## 2. 当前代码结构

```
src/
├── parser/       lexer + block(脚本→AST树) + List(裸值列表)
├── ast/          effect/trigger/lower(Block→Effect/Trigger)
├── runtime/
│   ├── entities.rs   Province/Country/Division/Battle/Scope
│   │                 Division 字段: location/destination/origin_province/
│   │                 pending_arrival/supporting/retreating/attacking 等
│   ├── world.rs      World状态 + scope_stack + 实体管理 + started标志
│   ├── interp.rs     Interpreter(run/eval + run_for_each作用域枚举)
│   ├── registry.rs   Registry(effects/triggers) + ParamGet
│   ├── clock.rs      GameClock::tick(主循环)
│   └── error.rs      CmdError
├── combat/
│   ├── resolve.rs    陆战结算(骰子/防御池/装甲) + cleanup(撤退/歼灭/带溃/占地)
│   │                 多战场伤害 delta 累积; 攻方撤退回origin; 守方撤退改location
│   ├── movement.rs   check_engagements(过滤retreating) + cancel_finished_supports
│   │                 + advance_movement(行军+到达+占领, 到达查敌军)
│   ├── width.rs      战斗宽度 + reinforce_reserves(预备队补位2%/h)
│   ├── recovery.rs   组织度恢复(敌方掉/己方恢复/移动中掉)
│   ├── reinforce.rs  装备+人力增援(每日, 排除移动中)
│   ├── commands.rs   create_division/start_battle/move_division/
│   │                 support_attack/stop_order + join_as_attacker(共用)
│   └── equipment_data.rs  5种1936装备真实数值
├── commands/     vars/control/scope命令注册
├── wasm_api.rs   WASM桥接(engine_tick/deploy/move/support_attack/
│                 stop_order/serialize, 序列化含 supporting/progress/pending)
└── lib.rs / main.rs
web/
└── index.html    UI(部署面板+Canvas节点图+交战视窗+弹菜单+状态条+进度箭头)
```

### 主循环顺序(clock.rs 每小时)
```
1. hour += 1; started = true(首次tick)
2. fire_event(on_hourly)
3. check_engagements        — 移动中/pending师遇敌→开战(过滤retreating)
4. cancel_finished_supports — 支援目标省战斗结束→清supporting
5. resolve_all_battles      — 战斗结算(伤害累积+撤退+歼灭+带溃+占地)
6. reinforce_reserves       — 预备队补位(2%/h)
7. advance_movement         — 行军推进 + 到达占领(查敌军) + pending_arrival结算
8. recover_org              — org恢复/损耗
```

---

## 3. 战斗指令完整参考(本阶段新增重点)

### 3.1 四种指令

| 指令 | 命令 | 语义 | 触发条件 |
|---|---|---|---|
| 移动/进军 | `move_division = { division target }` | 师移动到目标省 | 目标有敌军→进攻(红); 无敌军→普通移动(绿) |
| 支援攻击 | `support_attack = { division target }` | 师不移动, 远程参战(蓝) | **下单时目标省须已有战斗**, 否则无效 |
| 停止 | `stop_order = { division }` | 取消主动行动 | 有 destination/supporting 且非 retreating |
| 防守撤退 | `move_division`(到己方省) | 战斗地块下移动→撤退 | 师在战斗地块 + 目标己方省 |

### 3.2 关键判定逻辑

**move_division 的分支**(commands.rs, 按顺序):
1. 师在战斗地块 + 目标己方省 → **防守撤退**(retreating=true, 退出战斗, 不分攻守)
2. 目标有敌军 → **进攻**(开战, attacking=true)
3. 否则 → **普通移动**

**stop_order 语义**(取消主动, 保留被动):
- retreating → 忽略(撤退不能停)
- 有 destination/supporting → 清状态 + 从 attackers/reserve_attackers 移除
- **不动** defenders/reserve_defenders(被动防守继续)
- 撤退变攻方(无 destination/supporting)→ 忽略(避免同省停止冲突)

**support_attack**:
- 下单时无战斗 → 静默无效(蓝箭头不出现)
- 有战斗 → 加入攻方(复用 join_as_attacker), 不移动
- 战斗消失 → cancel_finished_supports 自动清 supporting
- 战败 → 攻方撤退回 origin(原地), 清 supporting
- 不占地(location≠目标省)

### 3.3 预备队判定(join_as_attacker)
- **部署阶段**(started=false): 同方向(同 origin)都进前线
- **游戏开始后**(started=true): 同 origin 后到的进预备队
- **宽度限制**: 始终生效(超宽进预备队, 与 started 无关)

---

## 4. 交战与归属规则(完整)

详见 `docs/specs/2026-06-21-combat-ownership-rules.md` + `docs/specs/2026-06-21-support-attack.md`

核心:
- 规则0: origin_province 出发地记录
- 规则1: 同省异国师立刻开战(不管到达); check_engagements 过滤 retreating 师
- 规则2: 战斗中绝不改归属; 无战斗+无敌军+非己方→占领
- 规则3: 战斗胜利方占领; 支援攻方不占地
- 到达=进度满+战斗胜(缺一不可); 到达时查敌军, 有敌军→pending等开战
- 撤退瞬间 location 改成目的地(防瞬移BUG)

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

---

## 6. 后续系统(待做)

- **生产系统**: 工厂→IC→生产线→产装备(现在装备靠"补给"按钮凭空给)
- **AI对手**: FRA自动部署/防守/反击
- **国策系统**: 加载核心国策, 点国策触发效果
- **扩展地图**: 从3省扩展到几十省
- **海军/空军**: 简化为制海权/制空权数值
- **UI 撤退显示优化**: 当前撤退中 UI 图标会跳到目的地(逻辑优先), 后续优化为显示行军过程

---

## 7. 新会话怎么接上

1. 在 `G:\projects\hoi4-clone\` 开新对话
2. 读本文件 + `docs/specs/2026-06-21-combat-ownership-rules.md` + `docs/specs/2026-06-21-support-attack.md`
3. 看 `git log --oneline` 了解最近改动
4. 跑 `cargo test` 确认基线(**99测试**)
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
