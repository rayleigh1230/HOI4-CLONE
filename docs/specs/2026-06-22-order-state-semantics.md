# OrderState 状态机语义(2026-06-22)

> 本文件是行动状态机的"宪法",所有 combat 逻辑必须遵守。后续改动以此为基准。

## 核心概念

- **dest(当前段终点)**:正在进入的那一个省份。长途移动逐段推进,dest 始终是"下一段的终点",不是最终目标。
- **归属地(location_province)**:师"完完全全进入"的省 = 已占领的省。只有满足特定条件才更新。
- **两组状态独立**:Moving 组(进军/移动)和 Retreating 组(撤退)各自有自己的判定和动作,互不套用逻辑。

---

## Moving 组(进军/移动)

同一指令 `move_division`,两种状态由"dest 归属地是否己方 + 是否有敌军"区分:

### 状态转换

```
Idle ──下令──► Moving{dest, progress, hostile, origin}
                  │
                  ├─ 进度满 + dest 己方/无敌军(占领) ──► location=dest, Idle
                  │   (己方省直接归属; 敌方空省占领+org掉血)
                  │
                  ├─ 进度满 + dest 有战斗/敌军 ──► Pending{dest}
                  │   (location 不变! 保持上一个归属地)
                  │
                  └─ 途中每小时索敌(check_engagements):
                      dest 出现敌军 ──► 加入/创建战斗(当攻方)
                      归属地被攻 ──► 自动当守方(被动)
```

### Pending 的处理

```
Pending{dest} ──战斗胜 + 无敌人──► location=dest(占领), Idle
             ──战斗败──────────► 转入 Retreating 组(战败撤退)
```

### 关键规则(规则3)

**归属地(location)变更必须同时满足**:
1. 移动条到达新地区 100%(进度满)
2. 自身未处于任何战场(无战斗 + 无敌军)

→ Pending 时 location **不变**(师在战场里)。UI 上师显示在归属地,向 dest 有进度箭头表示正在攻打。

---

## Retreating 组(撤退)

独立状态,独立判定。**只去邻近省份**(规则4),不长距离移动。

### 进入 Retreating 的途径

1. 守方战败(org归零+HP有余)→ 自动撤向邻省
2. 攻方战败 → 转入 Retreating(战败撤退)
3. 玩家主动下令(战斗地块 + 目标己方省)→ 撤退

### 撤退到达邻省后的分支(独立判定)

```
Retreating{dest} ──进度满──► 检查 dest 省状态:
                                │
                                ├─ dest 己方(无敌军) ──► location=dest, Idle(恢复)
                                │
                                ├─ dest 敌方 + 无敌军 ──► 强制 location=dest(占领), Idle
                                │
                                └─ dest 敌方 + 有敌军 ──► 强制 location=dest, 进入战场(攻方)
                                    │   ├─ 战胜 ──► 占领(归属地已是 dest), Idle
                                    │   └─ 战败 ──► 找周围己方领地撤退(转 Retreating)
                                    │            └─ 周围无己方 ──► 歼灭(包围)
```

### 战败回归属地的统一规则

无论 Moving 组还是 Retreating 组,战败时:
- **归属地(location)仍己方** → 瞬间回归属地(转 Idle,因为师"本就属于那里")
- **归属地已非己方** → 找邻省撤退(转 Retreating);无邻省 → 歼灭

---

## 违规清单(当前代码 vs 本语义)

| # | 违规点 | 位置 | 修复 |
|---|---|---|---|
| 1 | Pending 时改 location | movement.rs:179 | 删掉 `d.location_province = dest` |
| 2 | 主动撤退不限邻省 | commands.rs:260 | 加邻接检查 |
| 3 | 攻方战败用 Retreating 行军(应瞬间回归属地) | resolve.rs:367-388 | 改回瞬间:己方则 Idle,非己方找邻省(也瞬间) |
| 4 | 撤退到达逻辑未区分己方/敌方省 | movement.rs(到达分支) | 按上面"撤退到达分支"表实现 |

注:#3 与之前的"Retreating 行军回撤"改动冲突 — 那是基于旧理解(Pending 改 location 导致瞬移)。新语义下 Pending 不改 location,师始终在归属地,战败"回归属地"=原地转 Idle,无瞬移。
