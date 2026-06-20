# M4b — 生产系统 + 原版装备数据

> **创建日期**: 2026-06-20
> **前置**: M4a (`m4a-complete`) + M3 遗留修复

---

## 0. 目标

M4a 实现了装备库存/消耗/增援,但装备来源是 `add_equipment` 命令(凭空生成)。
M4b 补全两件事:
1. **生产系统** —— 工厂(IC)→生产线→产装备入仓库(装备的真实来源)
2. **原版装备数据** —— 从 49 个装备文件转译真实数值,建师用真实属性

形成完整循环: 造装备 → 入仓库 → 增援到师 → 战斗消耗 → 再造。

---

## 1. 生产系统(基于原版 NProduction defines)

### 核心公式(全部实证自 defines)
```
每日产出 = factories × BASE_FACTORY_SPEED_MIL(4.5) × efficiency × (1 - 资源惩罚)
效率: 起始 10%(BASE_FACTORY_START_EFFICIENCY_FACTOR)
      上限 50%(BASE_FACTORY_MAX_EFFICIENCY_FACTOR)
      每日 +1%(BASE_FACTORY_EFFICIENCY_GAIN)
资源惩罚: 每缺 1 资源/厂 -5%(PRODUCTION_RESOURCE_LACK_PENALTY=-0.05)
切换保留: variant 90% / family 70% / archetype 20%
```

### 数据模型
```rust
pub struct ProductionLine {
    pub equipment_type: String,
    pub factories: u32,           // 分配的军工厂数
    pub efficiency: f64,          // 0.10 - 0.50
    pub target_efficiency: f64,   // 通常 0.50
}
pub struct Country {
    pub civ_factories: u32,       // 民工厂(造建筑, M4b 暂只计数)
    pub mil_factories: u32,       // 军工厂(产装备)
    pub production_lines: Vec<ProductionLine>,
    pub resources: HashMap<String, f64>,  // steel/aluminum 等
}
```

### 每日生产 tick
```rust
fn produce_daily(world: &mut World) {
    for country in world.countries.values_mut() {
        let total_mil_used: u32 = country.production_lines.iter().map(|l| l.factories).sum();
        for line in &mut country.production_lines {
            // 效率增长(每日 +1%, 上限 50%)
            line.efficiency = (line.efficiency + 0.01).min(0.50);
            // 资源惩罚(简化: 缺资源按总厂比例)
            let output = line.factories as f64 * 4.5 * line.efficiency;
            // 产出入仓库
            *country.equipment_stockpile.entry(line.equipment_type.clone()).or_insert(0.0) += output;
        }
    }
}
```

挂在 on_daily。

### 命令
- `set_factories = { owner = GER mil = 10 civ = 15 }` 设置工厂数
- `add_production_line = { owner = GER type = infantry_equipment factories = 5 }` 建生产线

---

## 2. 原版装备数据转译

### 装备文件结构(实证自 infantry.txt)
```
infantry_equipment_1 = {
    soft_attack = 6
    defense = 22
    armor_value = 0
    ap_attack = 4          # 穿甲
    hardness = 0
    build_cost_ic = 0.50   # IC 成本
    resources = { steel = 1 }
}
```

### M4b 转译方式
不做完整 49 文件自动转译(那是 M5+ 内容转译的工作)。
M4b 用**手工精选 5 种核心装备**(步/炮/轻坦/中坦/重坦),数值取自原版文件,硬编码到 `equipment_data.rs`。

```rust
pub struct EquipmentDef {
    pub name: &'static str,
    pub soft_attack: f64, pub hard_attack: f64,
    pub defense: f64, pub breakthrough: f64,
    pub armor: f64, pub piercing: f64, pub hardness: f64,
    pub build_cost_ic: f64,
}
pub static EQUIPMENT: &[EquipmentDef] = &[
    EquipmentDef { name: "infantry_equipment", soft_attack: 6, hard_attack: 1, defense: 22, breakthrough: 3, armor: 0, piercing: 4, hardness: 0, build_cost_ic: 0.5 },
    EquipmentDef { name: "artillery", soft_attack: 25, hard_attack: 4, defense: 8, breakthrough: 12, armor: 0, piercing: 8, hardness: 0, build_cost_ic: 3.5 },
    EquipmentDef { name: "light_tank", soft_attack: 8, hard_attack: 6, defense: 4, breakthrough: 26, armor: 20, piercing: 20, hardness: 0.9, build_cost_ic: 5 },
    EquipmentDef { name: "medium_tank", soft_attack: 19, hard_attack: 14, defense: 5, breakthrough: 36, armor: 60, piercing: 61, hardness: 0.9, build_cost_ic: 12 },
    EquipmentDef { name: "heavy_tank", soft_attack: 20, hard_attack: 22, defense: 8, breakthrough: 48, armor: 100, piercing: 100, hardness: 0.95, build_cost_ic: 18 },
];
```

### 建师按装备汇总
`create_division` 改为: 接 `equipment = medium_tank amount = 50` 时, 从 EQUIPMENT 表查属性, 师属性 = 装备属性 × amount/100。

---

## 3. 范围边界(YAGNI)

**M4b 做**:
- Country 加工厂/生产线/资源字段
- produce_daily 生产逻辑(挂 on_daily)
- set_factories / add_production_line 命令
- 5 种核心装备数据(手工, 取自原版)
- create_division 按装备汇总属性
- 生产成本: 产装备时按 build_cost_ic 折算产出件数

**M4b 不做**(M5+):
- 民工厂造建筑(M4b 民工只计数)
- 资源开采/贸易(M4b 资源固定)
- 完整 49 文件自动转译
- 工厂效率切换(改产线时的保留比例)
- 建筑系统

---

## 4. 验收
- [ ] set_factories + add_production_line 能建工厂和产线
- [ ] 每日生产产出装备入仓库(数值符合 4.5×效率)
- [ ] 效率从 10% 增长到 50%
- [ ] create_division 用原版装备数值建师
- [ ] 端到端: 建工厂→产装备→建师→打仗→消耗→再造, 完整循环
- [ ] UI 显示工厂/产线/仓库
