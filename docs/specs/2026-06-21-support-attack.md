# 支援攻击(Support Attack)设计

> **创建**: 2026-06-21
> **状态**: 设计定稿, 待实现
> **需求来源**: 用户提出, 参考原版 HOI4 支援攻击机制

---

## 0. 一句话定义

支援攻击 = **师不移动, 但作为攻方远程参与目标省战斗**。本质是"原地打支援"。

---

## 1. 核心规则(用户多轮确认)

### 规则1: 目标省必须已有战斗(下单时判定)
- `support_attack` 下令瞬间检查: 目标省此刻**是否已有 battle**。
- **无战斗** → 指令**无效**(不设 supporting, 不显示箭头, 静默取消)。
- **有战斗** → 设 `supporting = Some(target)`, 师加入该战斗的攻方, 显示蓝色箭头。

判定时机精确定义(用户场景):
- 先发支援攻击(C→B): 此时 B 无战斗 → 指令无效, 蓝箭头不出现。
- 再发移动攻击(A→B): A 下令时 B 有敌军 → 立刻开战, B 省有战斗了(红箭头)。
- 之后发支援攻击(C→B): 此时 B 有战斗 → 蓝箭头出现, 生效。

即: "目标省有战斗" = 下单瞬间 `world.battles` 里是否有 `province == target` 的战斗。
**不包含**下单时同帧稍后才会创建的战斗(严格按下单顺序)。

### 规则2: 师不移动
- 支援攻击师的 `location`、`destination`、`move_progress` **不变**。
- 师留在出发省(C 省), 但作为攻方参与 B 省战斗。
- UI 显示蓝色箭头(从 C 指向 B), **无进度填充**(因为不移动)。

### 规则3: 其他判定与移动攻击一致
- 加入战斗的前线/预备队判定: 复用 move_division 逻辑(同 origin 判定 + 战斗宽度)。
- 伤害结算: 与普通攻方完全相同(resolve 正常打)。
- 战斗宽度占用: 占用 B 省战斗的攻方宽度。

### 规则4: 战斗结束判定不变(已满足, 无需改动)
战斗结束条件: **一方前线(含支援攻方)全退/全灭**。
- 场景3: 移动攻方被打退撤退, 但支援攻方还在前线 → 战斗继续(atk_alive 非空)。
- 这已被现有 cleanup 逻辑满足(它遍历所有 attackers, 不区分移动/支援)。

### 规则5: 支援攻方不占领目标省(已满足, 无需改动)
敌方全灭/全退后, 若我方只剩支援攻方:
- 占领条件要求 `attacker_present`(攻方师 location==province 且 destination.is_none())。
- 支援攻方 location = C 省 ≠ B 省 → `attacker_present = false` → **不占地**。
- 这已被现有 cleanup 占领逻辑满足。

### 规则6: 战败撤退原地不动
- 支援攻方被打退(org 归零) → 走攻方撤退逻辑。
- 因为支援师 location 没变(=出发省 C), origin = C, 撤退回 origin = 原地。
- 清除 `supporting` 标记。
- 注: 当前攻方撤退逻辑是"回 origin_province 瞬间"。支援师 origin=location, 回 origin = 不动, 符合要求。

### 规则7: 自动取消(战斗消失时)
- 每小时检查: 若 `supporting` 目标省的战斗已结束(不在 world.battles) → 清 `supporting`(支援师退出战斗状态)。
- 这对应"如果没战斗支援攻击就自动取消"。

---

## 2. 数据结构变更

### Division 新增字段
```rust
/// 支援攻击目标省(有值 = 正在支援攻击该省)。师不移动, 作为攻方远程参战。
/// 下令时目标省须已有战斗, 否则指令无效。
/// 战斗结束/战败时自动清除。
pub supporting: Option<u32>,
```

位置: `src/runtime/entities.rs` 的 Division 结构, 与 destination/attacking 并列。
默认 `None`(Default 派生)。

---

## 3. 新命令: support_attack

注册位置: `src/combat/commands.rs`

```
support_attack = { division = <id> target = <省id> }
```

### 下令逻辑(伪代码)
```
1. 取 div_id, target
2. 取 owner(释放借用)
3. 检查目标省是否已有战斗:
     existing = battles.find(|b| b.province == target)
   无战斗 → 返回 Ok(())(静默取消, 不设 supporting, 不报错)
4. 有战斗 → 设 d.supporting = Some(target)
5. 加入战斗(复用 move_division 的加入逻辑):
     - 同 origin(from_prov = d.location_province)已有攻方师 → 预备队
     - 超宽 → 预备队
     - 否则 → 前线(attackers.push)
6. 注意: 不改 location/destination/move_progress/attacking(师不移动)
```

### WASM 桥接
`src/wasm_api.rs` 新增:
```rust
#[no_mangle]
pub extern "C" fn engine_support_attack(division_id: u32, target: u32)
```
构造 `support_attack = { division = {division_id} target = {target} }` 脚本, 走 interp.run(同 engine_move_division 模式)。

---

## 4. 主循环各阶段对 supporting 师的处理

现有主循环(clock.rs):
```
1. check_engagements
2. resolve_all_battles
3. reinforce_reserves
4. advance_movement
5. recover_org
```

### 4.1 check_engagements(movement.rs)
**无需改动**, 但需确认:
- supporting 师无 destination → 不在 moving 列表 → 不重复触发开战。 ✓
- supporting 师在 enemy 查询时: 若其 location 省被进攻, 它可能被当守方? 
  - 不会: supporting 师作为攻方在 battle.attackers 里, in_battle 集合包含它 → 跳过。 ✓
  - 但若战斗结束了(它没被清理 supporting), 它可能被当守方 → 需要 §4.5 自动取消先行。

### 4.2 resolve_all_battles(resolve.rs)
**无需改动**。supporting 师在 battle.attackers/defenders 里, resolve 正常结算。
cleanup 的撤退处理: supporting 师作攻方撤退(is_attacker=true), 走"回 origin"逻辑(origin=location=原地), 但需**额外清除 supporting**。

cleanup 改动: 攻方撤退分支加 `d.supporting = None`。

### 4.3 reinforce_reserves(width.rs)
**无需改动**。supporting 师若在预备队, 按 2%/h 补位, 与普通师一致。

### 4.4 advance_movement(movement.rs)
**无需改动**。supporting 师无 destination → 不在 moving 列表 → 不推进进度。 ✓
到达判定、占领都不涉及 supporting 师。

### 4.5 新增: 自动取消支援(战斗消失时)
**新逻辑**, 加在哪? 最自然的位置是 check_engagements 之后(战斗判定阶段)或独立函数。
建议加一个小函数 `cancel_finished_supports(world)`, 在 check_engagements 后调用:

```rust
/// 清理支援攻击: 若目标省战斗已结束, 清除 supporting 标记
pub fn cancel_finished_supports(world: &mut World) {
    let active_provinces: HashSet<u32> = world.battles.iter()
        .map(|b| b.province).collect();
    for d in world.divisions.values_mut() {
        if let Some(t) = d.supporting {
            if !active_provinces.contains(&t) {
                d.supporting = None;
            }
        }
    }
}
```

主循环顺序更新(clock.rs):
```
1. check_engagements
2. cancel_finished_supports   ← 新增
3. resolve_all_battles
4. reinforce_reserves
5. advance_movement
6. recover_org
```

注: 放在 resolve 之前, 让"战斗已结束的支援师"在本 tick resolve 时不再被当攻方(避免它已被移出 battle 但 supporting 还在的瞬态)。

### 4.6 recover_org(recovery.rs)
**需小改**。当前 recover_org 对"非战斗师"恢复 org。supporting 师在 battle 里 → in_combat 集合包含它 → 不恢复。 ✓
但 in_combat 集合的构建需确认包含 supporting 师:
- supporting 师在 battle.attackers/reserve_attackers 里 → 已被 in_combat 覆盖。 ✓
无需改动。

---

## 5. UI 变更(web/index.html)

### 5.1 时间停止时显示箭头(已满足, 无需改动)
当前: 下令后 JS 调用 refresh()(click 监听 line 533), drawMap 基于 state 画箭头。
- 移动攻击(红箭头): d.attacking=true, d.destination=目标, WASM 已暴露 → 已显示。 ✓
- 普通移动(绿箭头): 同理已显示。 ✓
- 撤退(灰箭头): 已显示。 ✓
时间停止时下令, refresh() 立即触发, 箭头立即出现。**无需改动**, 只需新增支援攻击的蓝箭头(§5.2)。

### 5.2 支援攻击蓝色箭头(无进度填充)
WASM 序列化加 `supporting` 字段(wasm_api.rs serialize_state):
```rust
"supporting":{},  // d.supporting.unwrap_or(0)
```

UI drawMap 部队渲染, 在移动箭头逻辑后加:
```javascript
// 支援攻击: 蓝色箭头(无进度填充, 师不移动)
if (d.supporting && pos[d.supporting]) {
    const tx = pos[d.supporting].x, ty = pos[d.supporting].y;
    ctx.strokeStyle = '#3a86ff';  // 蓝色
    ctx.lineWidth = 2; ctx.setLineDash([6, 3]);
    ctx.beginPath(); ctx.moveTo(bx, by); ctx.lineTo(tx, ty); ctx.stroke();
    ctx.setLineDash([]);
    // 箭头头(在目标端)
    const ang = Math.atan2(ty - by, tx - bx);
    ctx.fillStyle = '#3a86ff';
    ctx.beginPath();
    ctx.moveTo(tx, ty);
    ctx.lineTo(tx - 8*Math.cos(ang-0.4), ty - 8*Math.sin(ang-0.4));
    ctx.lineTo(tx - 8*Math.cos(ang+0.4), ty - 8*Math.sin(ang+0.4));
    ctx.closePath(); ctx.fill();
}
```

### 5.3 UI 操作: 发起支援攻击(参考 HOI4 原版按键)

操作语义参考原版 HOI4(查证结果):
- 原版支援攻击 = **Ctrl + 右键**点击战斗气泡
- 原版战略部署 = 另一个修饰键(本项目暂不实现, 留空)

本项目按键映射:
| 操作 | 动作 |
|---|---|
| **左键点师图标** | 选中师 |
| **左键点空地块** | 取消选中 / 选地块看部队 |
| **右键点省** | 移动/进攻(红/绿箭头, 自动判断敌军) |
| **Ctrl + 右键点省** | 支援攻击(蓝箭头) |

实现改动(web/index.html 的 click 监听):
- 当前: `click` 事件统一处理(左键选中师→点省移动)。
- 改为:
  - `click`(左键): 点师=选中; 点省(有选中师)= 不再触发移动(改用右键); 点空地块=选地块。
  - `contextmenu`(右键): 若有选中师 → 调 engine_move_division(普通)。
  - `contextmenu` + `e.ctrlKey`: 若有选中师 → 调 engine_support_attack(支援)。
  - 右键默认行为 `e.preventDefault()`(屏蔽浏览器右键菜单)。
- 地图提示文字更新: "左键选中/看部队 | 右键移动进攻 | Ctrl+右键支援攻击"

---

## 6. 测试计划

### 单元测试(combat/commands.rs 或 battle.rs)
1. `support_attack_requires_existing_battle`: 目标省无战斗 → 指令无效, supporting 不设。
2. `support_attack_joins_existing_battle`: 目标省有战斗 → 加入攻方, supporting=target。
3. `support_attack_same_origin_goes_reserve`: 同省已有攻方 → 进预备队。
4. `support_attack_does_not_move`: 下令后 location/destination/move_progress 不变。

### 集成测试(battle.rs, 用 GameClock)
5. `move_then_support_both_engage`: 先 move(开战) 再 support → 两者都在战斗, 都造成伤害。
6. `support_then_move_support_invalid`: 先 support(无战斗,无效) 再 move → 只有 move 攻方参战。
7. `support_attacker_survives_after_move_attacker_retreats`: 移动攻方退, 支援攻方在场 → 战斗继续。
8. `support_only_does_not_capture`: 敌方全灭只剩支援攻方 → 目标省归属不变。
9. `support_auto_cancel_when_battle_ends`: 战斗结束 → supporting 自动清除。
10. `support_attacker_retreat_in_place`: 支援攻方战败 → 原地不动, supporting 清除。

---

## 7. 实现顺序(建议)

1. Division 加 `supporting` 字段(entities.rs)
2. support_attack 命令(commands.rs) + 复用 move_division 加入逻辑
3. cancel_finished_supports 函数(movement.rs) + 主循环接入(clock.rs)
4. cleanup 攻方撤退清 supporting(resolve.rs)
5. WASM 序列化 supporting + engine_support_attack(wasm_api.rs)
6. UI 蓝色箭头 + 支援攻击操作模式(web/index.html)
7. 测试(单元 + 集成)

---

## 8. 已确认无需改动的部分(依赖现有逻辑)

| 需求 | 现有逻辑 | 状态 |
|---|---|---|
| 战斗结束条件(一方前线全退) | cleanup 遍历所有 attackers, 不区分类型 | ✓ 已满足场景3 |
| 支援攻方不占地 | 占领需 attacker_present(location==province), 支援师 location≠province | ✓ 已满足场景4 |
| 支援攻方伤害结算 | resolve 处理 battle.attackers 里所有师 | ✓ 无需改 |
| 预备队补位 | reinforce_reserves 处理 reserve_attackers | ✓ 无需改 |
| recover_org 排除战斗师 | in_combat 集合包含 attackers | ✓ 无需改 |
