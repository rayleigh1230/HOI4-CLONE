# 多段路径行军 + 航点规划 设计文档

> 日期: 2026-06-23
> 状态: 已批准(头脑风暴),待实现
> 关联: `docs/specs/2026-06-22-order-state-semantics.md`(状态机宪法,本次不动其语义)

---

## 0. 背景与目标

### 现状

当前 `move_division` 只支持**单段移动**——玩家点选目标省,师直接朝该省移动。这有两个问题:

1. **远处省无法到达**:虽然命令接受任意 target,但没有寻路,师只能"一步跨过去"。在多省地图上,玩家无法命令师前往不相邻的远方省。
2. **无法长程规划**:玩家不能指定"先去 A 再去 B",只能逐次下令,且每次都要等上一段走完。

### 目标

实现两个相关功能:

- **功能 1 — 自动寻路**:玩家点远处省,系统自动 BFS 算最短路径,师逐段推进。
- **功能 2 — 航点规划**:新增 `queue_move` 命令,玩家可追加多个目标,师按顺序逐个执行(手机端友好,无需 shift)。

### 非目标(本次不做)

- 省份距离/地形速度差异(现在各省等距,将来加 `distance` 数据后寻路自然升级为 Dijkstra)
- 寻路时的避让规则(避开敌方驻军省、未开战不得入境等)——架构留口子,本次 `is_passable` 恒 true
- 航点的编辑/取消 UI(本次只支持"追加",取消靠 stop_order 整体取消)

---

## 1. 核心设计决策(头脑风暴结论)

| # | 决策 | 选择 |
|---|---|---|
| 1 | 寻路可通过性 | BFS 主体 + 抽象 `is_passable` 谓词(现在恒 true,未来可扩展) |
| 2 | 路径权重 | 带权图搜索框架,现在权重全 1(=BFS)。将来加距离数据 → 改权重插槽 → 自动变 Dijkstra |
| 3 | path 存储位置 | `OrderState::Moving` 变体加 `remaining: Vec<u32>` 字段(不动状态机宪法语义) |
| 4 | remaining 约定 | 存"dest 走完之后还要去的省",**不含当前 dest** |
| 5 | 中途段到达 | 走现有 Capture 逻辑(占领 + org 损),然后续走下一段 |
| 6 | 中途遇敌 | 完全复用现有规则(开战/Pending/战败),无需新逻辑 |
| 7 | 战败与路径 | 状态机一变(Moving→Idle/Retreating),remaining 自然消失,路径自动取消 |
| 8 | 航点架构 | 架构 Y:引擎只认一个 `remaining` 路径列表;航点和自动寻路在"下单时"都压平成同一个列表,运行时行为一致 |
| 9 | 航点触发 | 新命令 `queue_move`(追加到路径末尾);`move_division` 保持覆盖式 |
| 10 | 边界 A(行军中再下令) | `move_division`:覆盖(重新从当前省寻路);`queue_move`:追加到末尾 |
| 11 | 边界 B(战斗中下令) | Pending/Retreating 时新移动命令被忽略(不能中断战斗/撤退) |
| 12 | 边界 C(同省命令) | 起点终点同省 → 忽略(无意义命令) |
| 13 | 支援攻击收敛 | `support_attack` 的 target 必须与师 `location_province` **相邻**;不相邻 → 静默无效(与"无战斗"处理一致) |
| 14 | 路径中途失效应对 | 多段行军的师每小时检查"当前 dest 是否仍 `is_passable`";不可进入 → 停止(转 Idle,清 remaining)。另设强制中止函数 `invalidate_paths_to_inaccessible()`,供未来投降/停战事件批量调用(原版"强制中止敌对行为"的等价)。所有失效统一停止,**不重算绕路**(小地图无环,等价停止;重算留作未来大地图扩展) |

---

## 2. 数据结构变更

### 2.1 OrderState::Moving 加 remaining 字段

```rust
pub enum OrderState {
    Idle,
    Moving {
        dest: u32,           // 当前段终点(不变)
        progress: f64,       // 当前段进度(不变)
        hostile: bool,       // 当前段是否进军敌方(不变)
        origin: u32,         // 当前段出发地(不变)
        remaining: Vec<u32>, // 【新】剩余中转省(不含 dest,含最终目标)
    },
    Retreating { dest: u32, progress: f64 },  // 不变(撤退永远单段)
    Pending { dest: u32 },                    // 不变
    Supporting { target: u32 },               // 不变
}
```

### 2.2 remaining 语义(快递员比喻)

把师想象成快递员,身上揣一张"路线单"(remaining):
- `dest` = 当前正在去的下一站
- `remaining` = dest 走完之后还要去的站(按顺序)

举例:师在省1,点选省4,寻路得 `[省2,省3,省4]`:
- 下令时:`dest=省2, remaining=[省3,省4]`
- 走到省2(占领):`dest=省3, remaining=[省4]`(从单子划掉省2)
- 走到省3(占领):`dest=省4, remaining=[]`
- 走到省4(占领):remaining 空 → 转 Idle

### 2.3 单段移动 = remaining 为空

下单到相邻省时,寻路返回单元素路径 `[target]`:
- `dest=target, remaining=[]`
- 等价于现有单段行为,**完全向后兼容**

→ 现有所有单段测试零改动。

### 2.4 Default 兼容

`Vec<u32>` 实现 Default,`#[derive(Default)]` 的 Idle 默认值不受影响。但 Moving 变体手动构造处(测试、demo)需补 `remaining` 字段。

### 2.5 派生方法(可选)

`Division` 可加便捷方法:
```rust
/// 当前多段路径剩余的站数(含 dest)。单段返回 1,多段返回 >1
pub fn path_remaining_count(&self) -> usize { ... }
```
非必须,看实现需要。

---

## 3. 寻路模块(新文件 `src/combat/pathfinding.rs`)

### 3.1 核心函数

```rust
/// 从 from 寻路到 to,返回路径(含 to,不含 from)。
/// - 路径第一个元素 = 下一站,最后一个元素 = 最终目标
/// - 找不到路径返回 None
/// - from == to 返回 None(无意义)
pub fn find_path(world: &World, from: u32, to: u32) -> Option<Vec<u32>>
```

返回值约定:
- `find_path(1, 5)` 若路线为 1→2→3→5,返回 `Some([2, 3, 5])`
- `find_path(1, 1)` 返回 `None`(同省)
- 不连通返回 `None`

### 3.2 算法:带权 BFS(现为标准 BFS)

```
fn find_path(world, from, to):
    if from == to: return None
    队列 = [from]
    visited = {from}
    came_from = {}   // 记录每个省的"来路"

    while 队列非空:
        cur = 队列弹出
        if cur == to: break
        for n in neighbors(cur):
            if n 未访问 且 is_passable(world, n):
                visited 加入 n
                came_from[n] = cur
                队列加入 n

    if to 未在 came_from: return None  // 不连通

    // 从 to 回溯到 from,得到路径
    路径 = []
    cur = to
    while cur != from:
        路径.prepend(cur)
        cur = came_from[cur]
    return 路径
```

### 3.3 is_passable 插槽(可扩展性核心)

```rust
/// 判断一个省能否作为寻路中转(穿过)。
/// 当前实现:恒返回 true(任何省可穿,穿过敌方驻军省时由运行时战斗逻辑处理)。
///
/// 未来扩展点:
/// - "未开战不得入境":检查 n.controller 是否己方或已开战
/// - "绕开驻军省":检查 n 有无敌方非撤退师
/// 改这个函数即可,BFS 主体不动。
fn is_passable(world: &World, prov: u32) -> bool {
    true
}
```

### 3.4 权重插槽(为将来 Dijkstra 预留)

现在 BFS 等价于"每条边权重=1"。将来加距离数据时:
```rust
/// 两邻接省之间的边权重(行军代价)。
/// 当前:恒 1(等距,BFS 行为)。
/// 将来:从 province.neighbor_distances 取实际距离 → 寻路自动变 Dijkstra。
fn edge_weight(world: &World, from: u32, to: u32) -> f64 {
    1.0
}
```

实现时 BFS 主体可写成"按累计权重排序的优先队列"(Dijkstra 框架),权重全 1 时退化为 BFS。这样将来改 `edge_weight` 即升级,**寻路主体零改动**。

> 注:本次为降低实现复杂度,BFS 主体可用简单队列实现(不强制 Dijkstra 框架),但 `edge_weight` 函数预留,注释说明升级路径。具体由实现计划决定。

### 3.5 寻路失败处理

`find_path` 返回 `None` 时,下令逻辑应:
- 师不动(保持原状态)
- 静默忽略(不报错,不崩溃)
- 可选:日志记录便于调试

---

## 4. 命令变更(`src/combat/commands.rs`)

### 4.1 move_division 改动(覆盖式 + 寻路)

现有分支保留,在"非防守撤退"分支插入寻路:

```
move_division(div, target):
    1. 读 owner, cur_loc
    2. 【不变】防守撤退判定(在战斗地块 + 目标相邻己方省)→ Retreating, return
    3.【新增边界B】若师在 Pending/Retreating/Supporting → 忽略命令, return
    4.【新增边界C】if target == cur_loc → 忽略, return
    5.【新增】path = find_path(world, cur_loc, target)
       - if path == None → 忽略(寻路失败), return
    6. path 非空:
       - first = path[0], rest = path[1..]
       - hostile = (provinces[first].controller != owner)
       - order = Moving { dest: first, progress: 0, hostile, origin: cur_loc, remaining: rest }
    7.【不变】若 first 省有敌军 → join_as_attacker 开战
```

**单段特例**:相邻省寻路返回 `[target]`,rest=`[]`,行为与现在一致。

### 4.2 新命令 queue_move(追加式航点)

```
queue_move(div, target):
    1. 读 owner, cur_loc, 当前 order
    2.【边界B】若师在 Pending/Retreating → 忽略(不能追加到进行中的战斗/撤退)
    3.【边界C】if target == cur_loc → 忽略
    4. 追加逻辑(分两种当前状态):
       a. 当前 Moving: 
          - 从 cur_loc 到 target 寻路?不对——应追加到"当前路径末尾"。
          - 路径末尾 = remaining 最后一个,或 dest(若 remaining 空)
          - end_prov = remaining.last().unwrap_or(dest)
          - seg = find_path(world, end_prov, target)
          - remaining.extend(seg)
       b. 当前 Idle/Supporting:
          - 等同 move_division(从头寻路 cur_loc → target)
          - 若 Idle:设 Moving;若 Supporting:先停支援再 Moving(或忽略?见 4.4)
     5. 若 seg == None → 忽略(追加失败,保持原状)
```

> 注:queue_move 追加时,`end_prov`(当前路径末尾)可能是 dest(remaining 空)或 remaining.last()。若 end_prov == target(追加的就是当前末尾),find_path 返回 None → 忽略(无意义追加)。

### 4.3 support_attack 收敛(新增邻接检查)

现有 support_attack 已有"目标省无战斗 → 静默无效"判定。**新增**:目标省与师 `location_province` 不相邻 → 同样静默无效。

理由:
- HOI4 原版支援攻击就是"从相邻省发起的远程参战",不是跨地图开火
- 多段行军引入"远距离"概念后,若不限邻接,师能隔着整个地图支援,破坏空间逻辑
- 处理方式与现有"无战斗无效"完全一致(静默,不报错,不设 Supporting),保持代码风格统一

```
support_attack(div, target):
    1. 读 cur_loc
    2.【新增】adjacent = provinces[cur_loc].neighbors.contains(target)
       if !adjacent → 静默无效(不设 Supporting,蓝箭头不出现), return
    3.【不变】has_battle = battles 里 target 省有战斗
       if !has_battle → 静默无效, return
    4.【不变】设 Supporting{target} + join_as_attacker
```

### 4.4 queue_move 对 Supporting 状态的处理(待定,倾向)

师正在支援攻击(Supporting)时收到 queue_move:
- **倾向**:忽略(支援是主动战斗行为,不应被移动命令打断;玩家应先 stop_order 再 queue_move)
- 备选:停止支援后开始移动。但这会让 support_attack 和 queue_move 互相干扰。

→ 实现时按"Supporting 时忽略 queue_move"处理,文档记录。

### 4.5 WASM API 新增

`src/wasm_api.rs` 加:
```rust
#[no_mangle]
pub extern "C" fn engine_queue_move(division_id: u32, target: u32) {
    // 构造 queue_move = { division = X target = Y } 脚本执行
}
```

`engine_move_division` **无需改签名**(仍是 div_id + target),内部走新寻路逻辑。

### 4.6 路径中途失效应对(决策 14)

**问题**:师沿多段路径行军时,省份的可进入性可能动态变化(未来投降/停战导致对方领土变不可进入)。需要让师"不做傻事"——不闯入不可进入的省。

**失效类型与统一应对**:

| 失效位置 | 例子 | 应对 |
|---|---|---|
| 当前 dest 段(progress 未满) | 师正朝省2走,省2突然不可进入 | **停止**(转 Idle,清 remaining) |
| 航点(玩家必经点) | 玩家设的省5不可进入 | **停止**(不能绕,绕则违背玩家意图) |
| 终点 | 最终目标省不可进入 | **停止** |
| 中转省(系统算的) | 寻路途经省4不可进入 | **停止**(小地图无环,重算=停止;重算留未来扩展) |

**机制 1:每小时检查(轻量,主循环内置)**

每个主循环 tick,在推进进度前(advance_movement 开头),检查每个多段行军师的**当前 dest** 是否仍 `is_passable`:
```
advance_movement 开头(新增第 0 步):
    for 每个 Moving 师的 (id, dest):
        if !is_passable(world, dest):
            师转 Idle,清 remaining(路径停止)
            // 不从攻方角色移除(此时师不在战斗里,dest 不可进说明无战斗)
```

只查 dest(当前正在去的省),不扫整条 remaining——因为师还没走到后面的省,那些省的状态等走到时再查。这避免每 tick 扫整条路径(性能),且语义正确(只关心"下一步能不能迈")。

**机制 2:强制中止函数(供未来事件调用)**

```rust
/// 强制中止所有路径涉及不可进入省的师(转 Idle)。
/// 供未来投降/停战/领土移交事件批量调用 —— 即原版"强制中止敌对行为"的等价。
/// 扫描所有 Moving 师的 dest + remaining,任一不可进入则整条路径作废。
pub fn invalidate_paths_to_inaccessible(world: &mut World) {
    for d in world.divisions.values_mut() {
        if let OrderState::Moving { dest, ref remaining, .. } = d.order {
            let blocked = !is_passable(world, dest)
                || remaining.iter().any(|&p| !is_passable(world, p));
            if blocked {
                d.order = OrderState::Idle;  // 清 remaining(Moving→Idle)
            }
        }
    }
}
```

与机制 1 的区别:机制 1 只查 dest(逐 tick 渐进),机制 2 扫整条路径(事件触发时一次性)。投降事件用机制 2 立刻清场,不等师逐站走到失效点。

**为什么不重算绕路**:
- 小地图是线性链,中转省失效 = 整条路断,重算也找不到新路 = 等价停止
- 重算需要地图有环(多点连通),当前地图不具备
- 写了重算逻辑也是 dead code,徒增复杂度
- **未来扩展**:地图加环后,把"中转省失效→停止"升级为"→重新 find_path"是一处改动(advance_movement 第 0 步 + invalidate 函数),架构已为此预留

**触发源现状**:
- `is_passable` 现在恒 true → 上述两机制**当前不会触发任何停止**(没有省会变不可进入)
- 机制就位,等未来投降/停战系统接入 `is_passable` 的真实判定后自然生效
- 测试时临时 mock `is_passable` 返回 false 验证停止行为(决策 14 的测试用此方式)

---

## 5. 行军推进变更(`src/combat/movement.rs`)

### 5.1 advance_movement 到达后的续走逻辑

现有到达判定(Capture/Pending/RetreatIntoEnemy)**保留**。新增:Capture 分支成功占领后,检查 remaining。

```
第二阶段应用决策 - Capture 分支后:
    if let Some(d) = world.divisions.get_mut(&a.id):
        d.location_province = a.dest
        d.order = OrderState::Idle   // 临时,下面可能覆盖

    // 【新增】检查路径剩余
    if !a.remaining.is_empty():     // a 需携带 remaining 信息
        next = a.remaining[0]
        new_remaining = a.remaining[1..]
        hostile = (provinces[next].controller != a.owner)
        d.order = Moving { dest: next, progress: 0, hostile, origin: a.dest, remaining: new_remaining }
        // origin = a.dest(刚占领的省成为下一段出发地)
        // 若 next 有敌军 → 不在此开战,交给下一 tick 的 check_engagements
    else:
        d.order = Idle   // 路径走完
```

### 5.2 Arrival 结构体加 remaining 字段

```rust
struct Arrival { id: u64, dest: u32, owner: String, remaining: Vec<u32> }
```
第一阶段收集到达候选时,从 Moving 的 remaining 取出一并携带。

### 5.3 续走时不立即开战

占领中途省后续走,若下一站有敌军,**不在此处开战**,而是等下一主循环 tick 的 `check_engagements` 处理(与现有"Moving 师每小时索敌"规则一致)。这保持主循环顺序不变。

### 5.4 Pending 续走(战斗胜利后)

Pending 的师(战斗胜 + 无敌人)走第四阶段结算时,**也要检查 remaining**:
- 战斗胜利占领 dest → 同 5.1 的续走逻辑(看 remaining 有无下一站)

→ 第四阶段"Pending → 占领"分支同样需要加 remaining 续走判断。这是**两处**续走点:
1. 第五阶段:Moving 直接占领(无敌军)后续走
2. 第四阶段:Pending 战斗胜利占领后续走

两处逻辑相同,可抽成辅助函数 `continue_path_if_any(world, div_id)`。

### 5.5 战败/停止:零改动(架构 Y 的优雅)

- 进攻战败(归属地己方)→ Moving 变 Idle → remaining 字段随之消失 → **路径自动取消** ✅
- 归属地丢 → Moving 变 Retreating → remaining 消失 → **路径自动取消** ✅
- stop_order → Moving 变 Idle → remaining 消失 → **剩余路径取消** ✅

无需在战败/停止逻辑里手动清 remaining。这是"remaining 只存在 Moving 变体内"的核心好处。

### 5.6 中途省占领的 org 损

中途省占领走现有 Capture 第三阶段逻辑(占领非己方省 → org 损 ORG_LOSS_ON_CONQUER)。每占一省掉一次 org,长途行军累积消耗。**现有逻辑覆盖,无需改**。

---

## 6. WASM 序列化(`src/wasm_api.rs`)

### 6.1 零 UI 改动承诺(部分保留)

现有 serialize_state 把 Moving 拍平为 `dest, progress, attacking` 等键。**这些键不变**——前端只画当前段(dest → origin 的箭头),remaining 不暴露给 JS。

前端看到的是"师朝 dest 移动",与单段完全一样。师走完一段换 dest,前端箭头自然切换。**前端代码零改动**。

### 6.2 可选:暴露 path 长度(调试/UI 增强)

若将来想让前端画出完整路径(虚线途经点),可加:
```rust
// 在 division JSON 里加:
"path_len": remaining.len() + 1   // 含 dest 的总站数
```
本次**不做**,保持零 UI 改动。记录为未来增强。

### 6.3 engine_queue_move 新 FFI

见 4.4。前端调用方式(本次不改前端,但记录接口):
```js
// 将来前端按钮调用:
engine_queue_move(divId, targetProvince)
```

---

## 7. 测试计划

### 7.1 现有测试回归(零改动验证)

- `t_division_moves_to_destination`:单段,remaining=[],应通过
- `t_conquering_loses_org`:单段,应通过
- 所有 battle.rs / teleport_bug.rs:单段场景,应通过
- → **预期全部通过**,验证向后兼容

需要给现有测试里所有 `OrderState::Moving { ... }` 构造补 `remaining: vec![]`(编译期会报错提示位置)。

### 7.2 新增单元测试(pathfinding.rs)

| 测试 | 验证 |
|---|---|
| `t_find_path_adjacent` | 相邻省寻路返回单元素 `[to]` |
| `t_find_path_multi_hop` | 链式拓扑 1-2-3-4,寻路 1→4 返回 `[2,3,4]` |
| `t_find_path_same_province` | from==to 返回 None |
| `t_find_path_disconnected` | 不连通返回 None |
| `t_find_path_picks_shortest` | 有两条路时选跳数少的(权重全1=BFS) |

### 7.3 新增集成测试(movement.rs / battle.rs)

| 测试 | 场景 |
|---|---|
| `t_multi_hop_arrival_conquers_each` | 3 省链,师从省1走 `move_division` 到省3,逐段占领省2、省3,每段掉 org |
| `t_multi_hop_continues_after_mid_capture` | 占领中途省2后自动续走省3,最终 Idle 在省3 |
| `t_multi_hop_cancel_on_defeat` | 中途省2遇敌战败 → 路径取消,师回原地(不继续省3) |
| `t_multi_hop_stop_cancels_remaining` | 多段行军中 stop_order → 师停在当前省,剩余路径取消 |
| `t_queue_move_appends` | queue_move 追加目标,remaining 末尾增长,师按顺序走完 |
| `t_queue_move_from_idle` | Idle 时 queue_move 等同 move_division |
| `t_move_division_overrides_path` | 行军中 move_division 新目标 → 重新寻路覆盖 |

### 7.4 边界测试

| 测试 | 验证 |
|---|---|
| `t_move_to_same_province_ignored` | 边界 C:同省命令忽略 |
| `t_move_during_pending_ignored` | 边界 B:Pending 时命令忽略 |
| `t_move_during_retreating_ignored` | 边界 B:Retreating 时命令忽略 |
| `t_find_path_no_route_ignored` | 寻路失败 → 师不动 |
| `t_support_attack_non_adjacent_ignored` | 决策13:支援攻击 target 不相邻 → 静默无效(不设 Supporting) |
| `t_support_attack_adjacent_works` | 相邻 + 有战斗 → 正常支援(回归,确保邻接检查不误伤合法支援) |
| `t_path_stops_when_dest_inaccessible` | 决策14机制1:多段行军途中,dest 突然不可进入(mock is_passable)→ 师转 Idle,remaining 清空 |
| `t_invalidate_paths_clears_blocked` | 决策14机制2:invalidate_paths_to_inaccessible 扫描,dest 或 remaining 任一不可进入 → 整条路径停止 |

---

## 8. 文件改动清单

| 文件 | 改动 |
|---|---|
| `src/runtime/entities.rs` | `OrderState::Moving` 加 `remaining: Vec<u32>` 字段 |
| `src/combat/pathfinding.rs` | **新文件**:`find_path` + `is_passable` + `edge_weight` |
| `src/combat/mod.rs` | 声明 `pub mod pathfinding;` |
| `src/combat/commands.rs` | `move_division` 加寻路;新增 `queue_move` 命令注册;`support_attack` 加邻接检查(决策13) |
| `src/combat/movement.rs` | `advance_movement` 到达后续走逻辑 + 开头加 dest 可进入性检查(决策14机制1);`Arrival` 加 remaining;辅助函数 `continue_path_if_any`;`invalidate_paths_to_inaccessible`(决策14机制2) |
| `src/wasm_api.rs` | 新增 `engine_queue_move` FFI |
| `tests/battle.rs`(或新文件) | 补 `remaining: vec![]` 到现有 Moving 构造;加多段测试 |

**不改的文件**:
- `clock.rs`(主循环顺序不变)
- `combat/resolve.rs`(战败规则不变,自动取消路径)
- `combat/recovery.rs`(org 恢复读 OrderState,Moving 语义不变)
- `combat/reinforce.rs`(同上)
- `web/index.html`(零 UI 改动)

---

## 9. 风险与回退

### 9.1 风险

| 风险 | 应对 |
|---|---|
| 现有测试因补 remaining 字段大面积改 | 编译器逐个报错,机械补 `vec![]`,风险低 |
| 续走逻辑忘记处理某到达分支 | 续走集中在辅助函数 `continue_path_if_any`,两处调用点明确 |
| 寻路在小地图上性能 | 3-9 省 BFS 可忽略;将来大地图再加 A* 或缓存 |
| queue_move 对 Supporting 的处理分歧 | 文档定为"忽略",实现时加注释 |

### 9.2 回退

改动集中在 Moving 变体 + 新文件 pathfinding.rs。若出问题:
- 删除 pathfinding.rs + queue_move
- move_division 退回不寻路(相邻省直接 Moving,remaining=vec![])
- 保留 remaining 字段(空)不影响其他逻辑

---

## 10. 未来扩展(本次不做,架构已预留)

1. **加权寻路(Dijkstra)**:给 `Province` 加 `neighbor_distances: HashMap<u32,f64>`,改 `edge_weight` 插槽 → 寻路自动升级为距离之和最短。
2. **避让规则**:改 `is_passable` 插槽 → 实现未开战不得入境/绕开驻军省。
3. **航点编辑 UI**:前端加航点列表显示/删除按钮,引擎暴露 path 给 JS。
4. **地形速度**:不同地形影响每段行军速度(MOVE_RATE 按地形取值),与寻路解耦。

---

## 附:与状态机宪法的关系

本次设计**不修改** `2026-06-22-order-state-semantics.md` 的任何语义:
- Moving 组的"占领/Pending/索敌"规则不变
- Retreating 组完全不动(不加 remaining)
- 战败规则不变(自动取消路径是副作用,非新规则)

`remaining` 是 Moving 变体的**实现细节字段**,不构成新的状态机状态。宪法第 1 条"进军/移动同一指令两种状态"仍然成立——dest 的归属地+敌军判定决定 hostile,只是 dest 现在是"多段路径的当前段终点"而非"唯一终点"(宪法 §核心概念 dest 定义本就支持此解读)。
