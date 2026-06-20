# 主动行军 + 进攻移动 — 机制澄清与设计

> **触发**: 用户要求做行军/进攻状态, 指出"地块有敌军立刻开战, 非到达才开战"
> **状态**: 机制已查证(原版 defines + wiki)

---

## 0. 原版机制(查证自 defines)

### 下令即判定, 非到达判定
玩家下令师 A→B 移动的瞬间:
- **B 无敌军/是己方** → 绿色箭头, 普通移动
- **B 有敌军** → **红色箭头, 立刻开战** + 同时移动(战斗+移动并行)

### 战斗中移动(进攻移动)
- 进攻方在战斗中**继续推进** move_progress(但速度 ×0.33)
- `COMBAT_MOVEMENT_SPEED = 0.33`(战斗中移动速度降至 33%)
- `LOW_ORG_FOR_ATTACK = 0.3`(org<30% 进一步减速)
- `ZERO_ORG_MOVEMENT_MODIFIER = -0.8`(0 org 时速度-80%)

### 战斗结局与移动
- 守方全退/歼灭 → 战斗结束 → 进攻方移动进度**加速恢复**(无战斗减速) → 到达占领
- `ORG_LOSS_FACTOR_ON_CONQUER = 0.2`(占领时掉 20% max_org)
- 攻方全退 → 战斗结束 → 进攻方移动**中止**(没打下来)

### 状态机(每个有 destination 的师)
```
移动中(绿): 目标无敌军, 正常推进
进攻中(红): 目标有敌军, 战斗+移动并行(慢)
  ├ 守方败 → 加速移动 → 到达占领
  └ 攻方败 → 移动中止
```

---

## 1. 数据模型扩展

```rust
pub struct Division {
    // ... 现有
    pub destination: Option<u32>,       // 目标省
    pub move_progress: f64,             // 0-1
    pub attacking: bool,                // 进攻移动(红箭头): 目标有敌军
}
```

`attacking` 区分普通移动(绿)和进攻移动(红)。

---

## 2. move_division 命令

```
move_division = { division = 1 target = 3 }
```
下令时:
1. 设 destination = target
2. 查 target 省有无敌军(非己方的师)
3. 有敌军 → attacking=true, 同时 start_battle(该师 vs 敌军)
4. 无敌军 → attacking=false, 普通移动

---

## 3. 主循环改动

### advance_movement 扩展
```
对每个有 destination 的师:
  if attacking(在战斗中):
    rate = MOVE_RATE × COMBAT_MOVEMENT_SPEED(0.33) × org减速
  else:
    rate = MOVE_RATE (普通) 或 MOVE_RATE×1.25(撤退)
  
  progress += rate
  if progress >= 1.0:
    到达:
      if attacking: 占领省份(已在战斗结束时处理)
      else: 进驻(location=dest)
    清 destination/attacking
    占领: org -= max_org × 0.2
```

### 战斗结束联动 cleanup_battles
当一场进攻战斗结束:
- 守方败 → 攻方 attacking 保持, 移动加速(无战斗减速)
- 攻方败 → 攻方 attacking=false, destination 清空(移动中止)

---

## 4. UI
- 绿箭头: 普通移动的师
- 红箭头: 进攻移动的师(战斗中)
- 地图点击: 选中师 → 点目标省 → 下移动令

---

## 5. 范围
做: move_division 命令, attacking 标志, 战斗中减速移动, 占领掉org, 攻方败中止移动
不做: 多段路径寻路(只支持邻接单步), 战线绘制(只单师)
