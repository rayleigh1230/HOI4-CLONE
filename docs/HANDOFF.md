# hoi4-clone 项目交接文档

> **用途**: 在新会话中继续开发。读本文件 + 列出的关键文件即可接上。
> **更新**: 2026-06-21

---

## 0. 项目概况

完整复刻 HOI4 核心机制的自制游戏, 分享给朋友玩。
- **技术栈**: Rust → WASM + 单文件 HTML/JS 前端
- **位置**: `G:\projects\hoi4-clone\`
- **运行**: `cargo test`(测试) / `cargo run --bin hoi4_demo`(CLI) / 浏览器 `http://127.0.0.1:8765`(UI demo)
- **工具链**: `stable-x86_64-pc-windows-gnu`(rustup override 绑定)
- **规模**: ~4000 行 Rust + UI, 73 测试, 220KB WASM

---

## 1. 已完成的里程碑

| M | 内容 | 测试 | tag |
|---|---|---|---|
| M1 | 脚本引擎(parser/AST/interpreter) | 19 | m1-complete |
| M2 | 运行时重构(Registry/Result/主循环) | 25 | m2-complete |
| M3 | 实体+作用域+陆战伤害 | 38 | m3-complete |
| M4a | 装备系统(库存/消耗/增援) | 42 | m4a-complete |
| 陆战循环 | 四量模型+撤退+歼灭+行军+包围+宽度+预备队+带溃 | 73 | (分支) |

### 陆战已实现的核心机制
- 四量模型: 组织度(Org) / HP / 装备 / 人力 独立
- 伤害: 骰子(d4/d2) + 防御池(10%/40%命中) + 装甲碾压 + 穿甲系数
- 撤退(org归零+HP有余) / 歼灭(HP归零) / 包围歼灭(无邻省)
- 行军(点选移动) + 进军(敌方地块红箭头) + 到达占领
- 战斗宽度(70) + 预备队 + 补位(2%/h) + 带溃(前线崩→预备队溃)
- 同出发地后续进攻→预备队(origin_province判定)
- 到达=进度满+战斗胜(两者满足才结算归属)
- org损耗: 敌方地块行军掉, 己方恢复

---

## 2. 当前代码结构

```
src/
├── parser/       lexer + block(脚本→AST树) + List(裸值列表)
├── ast/          effect/trigger/lower(Block→Effect/Trigger)
├── runtime/
│   ├── entities.rs   Province/Country/Division/Battle/Scope
│   ├── world.rs      World状态 + scope_stack + 实体管理
│   ├── interp.rs     Interpreter(run/eval + run_for_each作用域枚举)
│   ├── registry.rs   Registry(effects/triggers) + ParamGet
│   ├── clock.rs      GameClock::tick(主循环)
│   └── error.rs      CmdError
├── combat/
│   ├── resolve.rs    陆战结算(骰子/防御池/装甲) + cleanup(撤退/歼灭/带溃/占地)
│   ├── movement.rs   check_engagements + advance_movement(行军+到达+占领)
│   ├── width.rs      战斗宽度 + reinforce_reserves(预备队补位2%/h)
│   ├── recovery.rs   组织度恢复(敌方掉/己方恢复/移动中掉)
│   ├── reinforce.rs  装备+人力增援(每日, 排除移动中)
│   ├── commands.rs   create_division/start_battle/move_division等
│   └── equipment_data.rs  5种1936装备真实数值
├── commands/     vars/control/scope命令注册
├── wasm_api.rs   WASM桥接(engine_tick/deploy/move/serialize)
└── lib.rs / main.rs
web/
└── index.html    UI(部署面板+Canvas节点图+交战视窗+控制)
```

### 主循环顺序(clock.rs 每小时)
```
1. fire_event(on_hourly)
2. check_engagements    — 移动中师遇敌→开战
3. resolve_all_battles  — 战斗结算(伤害+撤退+歼灭+带溃+占地)
4. reinforce_reserves   — 预备队补位(2%/h)
5. advance_movement     — 行军推进 + 到达占领 + pending_arrival结算
6. recover_org          — org恢复/损耗
```

---

## 3. 下一步: 多战场 + 状态共享(待实现)

### 用户要求的机制(核心未完成项)

**3.1 一个师同时参与多场战斗**
- 师A从省X进攻省Y → A是攻方(省Y战斗)
- 敌军从省Z进攻省X → 省X战斗爆发, A自动成为**防守方**(归属地在省X)
- A同时打两场, **状态共享**(org/HP是同一个值, 两边被打都掉)
- 之前的进攻行动**不中断**

**3.2 进攻失败→瞬间回出发地**
- 进攻失败(撤退) → 直接回 origin_province, **不需要行军时间**
- (当前实现是撤到邻接己方省 + 行军, 需改成瞬间回出发地)

**3.3 UI部队位置始终=归属地**
- 部队图标在Canvas上始终显示在 location_province(归属地)
- 进攻别处时, 图标不移动(仍在出发地)
- 只有到达目标+战斗胜利+归属变更后, 图标才移到新地块

**3.4 resolve多战场伤害累积(非覆盖)**
- 当前bug: 师在多场战斗时, final_state.insert 覆盖(只保留最后一场伤害)
- 改: 算伤害差值(org_before-org_after), 累积到同一个师

### 实现优先级
1. **resolve多战场伤害累积** — 核心bug, 先修
2. **check_engagements加归属地防守** — 地块被进攻→归属师成防守方
3. **进攻失败瞬间回出发地** — 改cleanup撤退逻辑
4. **UI位置=归属地** — 确认pending_arrival不改location

### 关键代码位置
- resolve写回(覆盖bug): `src/combat/resolve.rs` 的 `final_state.insert`
- check_engagements: `src/combat/movement.rs` 的 `check_engagements`
- 撤退逻辑: `src/combat/resolve.rs` 的 `cleanup_battles`
- Division结构: `src/runtime/entities.rs`

---

## 4. 交战与归属规则(完整, 用户多轮纠正确认)

详见 `docs/specs/2026-06-21-combat-ownership-rules.md`

核心:
- 规则0: origin_province出发地记录
- 规则1: 同省异国师立刻开战(不管到达)
- 规则2: 战斗中绝不改归属; 无战斗+无敌军+非己方→占领
- 规则3: 战斗胜利方占领
- 到达=进度满+战斗胜(缺一不可)
- 行军中部队归属(location)不变

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

---

## 7. 新会话怎么接上

1. 在 `G:\projects\hoi4-clone\` 开新对话
2. 读本文件 + `docs/specs/2026-06-21-combat-ownership-rules.md`
3. 看 `git log --oneline` 了解最近改动
4. 跑 `cargo test` 确认基线(73测试)
5. 从 §3 的"实现优先级"开始做(多战场伤害累积)

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
- HashMap多可变借用: 用快照→计算→写回模式
