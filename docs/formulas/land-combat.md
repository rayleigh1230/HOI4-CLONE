# 陆战伤害计算公式(从原版 defines 推导)

> 来源: `common/defines/00_defines.lua` 的 `NMilitary` (733-1191行)
> 验证: 已基于源文件逐字段推导

## 1. 单营属性

```
BattalionStat = EquipmentStat × Need/100 × (1 + Σ营修正) × Σ地形修正
```

## 2. 师团属性汇总

求和项: soft_attack, hard_attack, air_attack, HP, combat_width
加权混合(60%平均+40%最高): armor_value, ap_attack(穿甲)
加权平均(按combat_width): hardness
加权平均(按权重,支援连权重=1): organisation

## 3. 每小时攻击点数

```
软攻击点 = 软攻击 × (1 - 敌硬度)
硬攻击点 = 硬攻击 × 敌硬度
× 地形修正 × 计划修正(+30%max) × 将领修正 × 补给修正
```

## 4. 防御池消耗(核心机制)

```
命中概率:
  有防御剩余 → 10%  (BASE_CHANCE_TO_AVOID_HIT=90%)
  防御耗尽后 → 40%  (CHANCE_TO_AVOID_HIT_AT_NO_DEF=60%)
```

## 5. 装甲碾压

我装甲 > 敌穿甲时:
- +6 组织度骰 (LAND_COMBAT_ORG_ARMOR_ON_SOFT_DICE_SIZE)
- +2 强度骰 (LAND_COMBAT_STR_ARMOR_ON_SOFT_DICE_SIZE)
- 敌方对己伤害 ×0.5

## 6. 穿甲系数表

| 我穿甲/敌装甲 | ≥1.0 | ≥0.75 | ≥0.50 | ≥0 |
|---|---|---|---|---|
| 系数 | 1.0 | 0.8 | 0.65 | 0.5 |

## 7. 掷骰结算

```
单命中:
  组织度伤害 = 1d4 × 0.053  (LAND_COMBAT_ORG_DICE_SIZE=4, ...MODIFIER=0.053)
  强度伤害   = 1d2 × 0.060  (LAND_COMBAT_STR_DICE_SIZE=2, ...MODIFIER=0.060)
```

## 8. 战斗修正

| 修正 | 值 |
|---|---|
| 夜间攻击 | -50% |
| 渡河小/大 | -30%/-60% |
| 要塞(每级) | -15% |
| 多方向被攻(防) | -50% |
| 堑壕(每级) | +2% |
| 被包围 | -30% |
| 两栖登陆 | -50% |
| 敌方制空 | -35% |
| 缺补给(攻方攻) | -25% |
| 超宽 | -1%/%(max -33%) |
| 堆叠(>5师) | -2%/师 |
| CAS支援 | +25%基础, 3CAS/敌宽 |

## 9. 损失转化

```
EQUIPMENT_COMBAT_LOSS_FACTOR = 0.70  (强度损1→0.7件装备真损)
EXPERIENCE_LOSS_FACTOR = 1.00
RETREAT_SPEED_FACTOR = 0.25
```

## 10. 多师分摊

```
DAMAGE_SPLIT_ON_FIRST_TARGET = 0.35
首要目标(装甲权重×1.2): 承受 35% 总伤害
其余目标: 均分 65%
```
