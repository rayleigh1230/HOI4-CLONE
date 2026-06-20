# 战斗宽度 + 增援队列 — 机制与设计

> **创建**: 2026-06-21
> **前置**: 陆战循环 + 行军 + 占领完成(67测试)

---

## 0. 机制(查证自原版)

### 战斗宽度
- 基础宽度 **70**(地形文件 00_terrain.txt)
- 每师占 `combat_width`(7步师=14)
- 一个省的战斗最多容纳 70 宽度的师参战
- `ENGAGEMENT_WIDTH_PER_WIDTH=2.0`: 每点己方宽度可接战 2 点敌方宽度
- 超宽: `COMBAT_OVER_WIDTH_PENALTY=-1%/每%`, 上限 -33%

### 增援队列(预备队)
- 省内多于宽度容纳的师 → 进入**预备队(reserve)**
- 前线师退下(撤退/歼灭)后, 预备队师按 `REINFORCE_CHANCE=0.02`(2%/小时)概率补上
- `RESERVE_TO_COMMITTED_BALANCE=0.3`: 预备队:参战队 ≈ 0.3:1

### 堆叠惩罚
- `COMBAT_STACKING_START=5`: 超 5 个师开始惩罚
- `COMBAT_STACKING_PENALTY=-2%/师`

---

## 1. 数据模型

### 战斗扩展
```rust
pub struct Battle {
    pub id: u64,
    pub province: u32,
    pub attackers: Vec<u64>,      // 前线攻方
    pub defenders: Vec<u64>,      // 前线守方
    pub reserve_attackers: Vec<u64>, // 预备队攻方(M4宽新增)
    pub reserve_defenders: Vec<u64>, // 预备队守方
}
```

### 宽度计算
```rust
impl World {
    /// 战斗当前已用宽度(攻方或守方)
    fn used_width(&self, div_ids: &[u64]) -> f64 {
        div_ids.iter()
            .filter_map(|id| self.divisions.get(id))
            .map(|d| d.combat_width)
            .sum()
    }
}
```

---

## 2. 进入战斗时的宽度分配

`move_division` 进军 + `start_battle` 时:
```
新师加入战斗:
  当前前线宽度 + 新师宽度 <= 70 → 直接进前线
  否则 → 进预备队(reserve)
```

---

## 3. 增援(每小时)

新增 `reinforce_reserves(world)`:
```
对每场战斗:
  前线有空位(宽度<70) 且 预备队非空:
    对每个预备队师, REINFORCE_CHANCE(2%) 概率:
      若加入后宽度<=70 → 移入前线
```

挂在 on_hourly(战斗结算后)。

---

## 4. 前线师退下后补位

cleanup_battles 移除撤退/歼灭师后:
- 前线出现空位 → 下一小时 reinforce_reserves 自动补

---

## 5. 范围
做: Battle 加 reserve, 宽度分配, reinforce_reserves, 堆叠惩罚
不做: 多方向进攻(FLANKED_PROVINCES), 地形宽度差异(都用70)
