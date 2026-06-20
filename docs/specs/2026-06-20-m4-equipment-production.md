# M4 — 装备系统 + 生产 + 原版数据

> **创建日期**: 2026-06-20
> **状态**: 设计中
> **前置**: M3 (`m3-complete`) + UI demo

---

## 0. 背景与概念澄清(用户指正)

HOI4 里 **HP / 组织度 / 装备是三个独立量**(我之前误把 strength 当装备):

| 量 | 原版字段 | wiki 定义 | 归零后果 |
|---|---|---|---|
| 组织度 Org | `max_organisation` | 战斗意志 | 撤退(不灭) |
| HP | `max_strength` | "HP. Known as strength in previous games" | 属性衰减、经验损失 |
| 装备 | (库存) | 师仓库装备充足度 | 缺装备→属性按比例下降 |

M3 只实现了 org + HP 两条。**M4 补全装备这第三条**,并实现"生产→库存→增援→消耗"完整闭环。

---

## 1. 目标(分两阶段,各自可验证)

### M4a: 装备战斗闭环(先做)
- 装备库存(国家仓库: equipment_type → 数量)
- 师的装备需求(division need: 该师满编需要多少装备)
- 师的装备充足度(实际/需求, 0-100%)
- 战斗消耗: HP 损失 → 装备损失(`EQUIPMENT_COMBAT_LOSS_FACTOR=0.70`)
- 属性按比例: 攻防属性 × 装备充足度
- 增援: 师从仓库领装备补满(按优先级)
- UI: 第三条"装备"血条

### M4b: 生产系统 + 原版数据
- 工厂(IC) → 生产线 → 产装备入库存
- 工厂效率曲线(起始10%/上限50%/切换保留)
- 资源需求与惩罚(缺资源 -5%/厂)
- 从原版转译装备数据(49 文件 → 装备属性)
- 建师时按营 need 算装备需求和属性

---

## 2. M4a 数据模型

```rust
// 国家仓库
pub struct Country {
    // ... 现有字段
    pub equipment_stockpile: HashMap<String, f64>,  // equipment_type → 数量
}

pub struct Division {
    // ... 现有字段
    pub equipment_need: HashMap<String, f64>,       // 满编需求
    pub equipment_held: HashMap<String, f64>,       // 当前持有
}

impl Division {
    /// 装备充足度(0-1), 影响属性
    pub fn equipment_ratio(&self) -> f64 {
        let need: f64 = self.equipment_need.values().sum();
        let held: f64 = self.equipment_held.values().sum();
        if need > 0.0 { (held / need).min(1.0) } else { 1.0 }
    }
    /// 实际软攻击 = 面板值 × 装备充足度
    pub fn effective_soft_attack(&self) -> f64 {
        self.soft_attack * self.equipment_ratio()
    }
    // 同理 effective_hard_attack/defense/breakthrough/...
}
```

战斗结算改用 `effective_*`(M3 用的是面板值)。

---

## 3. 战斗消耗公式(原版 defines)

```
HP 损失 → 装备损失:
  equipment_loss = hp_loss × EQUIPMENT_COMBAT_LOSS_FACTOR (0.70)
  即每掉 1 HP, 损失 0.7 件主装备
```

消耗的装备从 division.equipment_held 扣减。

---

## 4. 增援机制(简化版)

每天 tick 时,每个师尝试从国家仓库补满装备:
```
对每个 equipment_type:
  shortage = need - held
  available = stockpile[type]
  transfer = min(shortage, available)
  held += transfer
  stockpile -= transfer
```
M4a 不做优先级队列(M4b 或 M5 加),按师 id 顺序补。

---

## 5. M4b 生产系统

```rust
pub struct ProductionLine {
    pub equipment_type: String,
    pub factories: u32,
    pub efficiency: f64,  // 0.1-0.5
}
pub struct Country {
    pub production_lines: Vec<ProductionLine>,
    pub civ_factories: u32,
    pub mil_factories: u32,
}
```

每日:
```
每条生产线: 产出 = factories × BASE_FACTORY_SPEED_MIL(4.5) × efficiency × (1 - 资源惩罚)
efficiency 每日 += BASE_FACTORY_EFFICIENCY_GAIN(1), 上限 50%
装备入 stockpile
```

---

## 6. 原版数据转译

从 `common/units/equipment/*.txt` 解析装备属性:
```
infantry_equipment_1: soft_attack=6 defense=22 armor=0 build_cost_ic=0.50 ...
```
建师时按营 `need` 聚合:
```
师需求 = Σ(营.need[装备] × 营数量)
师属性 = Σ(装备.属性 × 营.need/100)
```

---

## 7. 验收

- [ ] M4a: 战斗中装备下降, 缺装备的师属性衰减, 增援补满, UI 三条血条
- [ ] M4b: 工厂产装备入库存, 建师消耗库存, 原版装备数值正确
- [ ] 端到端: 打仗→损耗→等生产→补装备→再打仗, 完整循环
