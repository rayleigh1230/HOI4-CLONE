# Demo 彻底改造 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 demo 从旧脚本路径彻底改造成"地图全屏+浮层、声明式数据绑定、图层化 Canvas、触屏适配"的扩展型 UI 架构,接通数据驱动建师/显式宣战/换模板,并修复实测暴露的基础构造 bug。

**Architecture:** 四层结构(engine/core/views+map/ui,原生 ES Modules,无构建)。引擎(Rust/WASM)侧补模板引用+绑定式数据视图+changeset;UI 侧用 bind.js 绑定框架对齐原版 scripted_gui 模型;Canvas 拆 6 图层 + 管家(相机/坐标转换/脏标记);PointerEvent 统一鼠标+触屏。

**Tech Stack:** Rust(stable-x86_64-pc-windows-gnu)→ wasm32-unknown-unknown;原生 ES Modules(浏览器 `<script type="module">`);python http.server 提供服务。无 npm/Vite/框架。

**Spec:** `docs/superpowers/specs/2026-06-25-demo-overhaul-design.md`

---

## File Structure

### Rust 引擎层(修改/新增)
- `src/data_raw/units/light_armor.txt`(新增)— 装甲营定义
- `src/data_raw/history/FRA.txt`(新增)— FRA OOB 模板
- `src/data/loader.rs`(修改)— `load_all` 加 light_armor + FRA
- `src/data/template.rs`(修改)— `to_division_stats` 改返回 `(DivisionStats, Vec<String>)` 告警
- `src/data/equipment.rs`(修改)— 修模块汇总 hardness/soft_attack 异常(根因待定位)
- `src/runtime/entities.rs`(修改)— `Division` 加 `template_name` 字段
- `src/combat/commands.rs`(修改)— `create_division` 记 template_name;新增 `change_template`/`edit_template` 命令
- `src/wasm_api.rs`(修改)— 新增 FFI + `get_state` 补字段 + `engine_get_templates`

### 前端 UI(新建 web/js/ 四层 + css)
- `web/index.html`(重写)— 根容器 + 加载 main.js
- `web/css/app.css`(新建)— 移动优先全屏布局
- `web/js/main.js`(新建)— 启动装配
- `web/js/engine/{wasm,commands,state}.js`(新建)— WASM 封装
- `web/js/core/{store,bind,router,canvas,input,el}.js`(新建)— 通用框架
- `web/js/views/{deployPanel,unitPanel,combatPanel,diplomacyPanel}.js`(新建)— 面板内容
- `web/js/map/{layout,layerTerrain,layerProvince,layerUnit,layerOrder,layerCombat,layerOverlay}.js`(新建)— 图层
- `web/js/ui/{topbar,panelHost,drawer,orderMenu,statbar}.js`(新建)— 复用组件

**Spec:** `docs/superpowers/specs/2026-06-25-demo-overhaul-design.md`

> **范围说明(YAGNI):** spec §4.3 的 `edit_template`(改模板联动所有师)标注为"可选",本次 plan **不实现**。理由:demo 只需 `change_template`(单师换模板)即可验证数据流联动;`edit_template` 的"改模板影响多师"语义在 demo 无多师共享同模板的测试场景,做了是债。后续做"师设计师"系统时再补(那时模板编辑有真实 UI 入口)。

---

**Files:**
- Create: `src/data_raw/units/light_armor.txt`

- [ ] **Step 1: 拷贝原版 light_armor 定义**

从原版 `G:/steam/steamapps/common/Hearts of Iron IV/common/units/light_armor.txt` 拷贝全文到 `src/data_raw/units/light_armor.txt`。保留原始内容(含 `sub_units = { light_armor = {...} }` 块)。无需改动。

- [ ] **Step 2: 确认 need 字段**

打开新文件,确认存在:
```
need = {
    light_tank_chassis = 60
}
```
(底盘 `light_tank_chassis` 已在 `tank_chassis.txt` 加载,无需补底盘)

- [ ] **Step 3: Commit**

```bash
git add src/data_raw/units/light_armor.txt
git commit -m "data: 补 light_armor 营定义(原版 common/units/light_armor.txt)"
```

---

## Task 2: 补 FRA OOB 模板文件

**Files:**
- Create: `src/data_raw/history/FRA.txt`

- [ ] **Step 1: 拷贝原版 FRA 1936 OOB**

从原版 `G:/steam/steamapps/common/Hearts of Iron IV/history/units/FRA_1936.txt` 拷贝全文到 `src/data_raw/history/FRA.txt`(改名规范,与 `GER.txt` 一致)。保留所有 `division_template` 块。

- [ ] **Step 2: 确认模板名**

打开新文件,确认含至少这些模板:
- `Division d'Infanterie`(9 步兵营)
- `Division Légère Mécanique`(含 light_armor)

- [ ] **Step 3: Commit**

```bash
git add src/data_raw/history/FRA.txt
git commit -m "data: 补 FRA OOB 模板(原版 FRA_1936.txt)"
```

---

## Task 3: load_all 加载新数据文件

**Files:**
- Modify: `src/data/loader.rs:298-318`(`load_all` 函数)

- [ ] **Step 1: 写失败测试**

在 `src/data/loader.rs` 的 `tests` 模块加测试:

```rust
#[test]
fn t_load_all_has_light_armor_and_fra_templates() {
    let data = crate::data::loader::load_all();
    // light_armor 营应加载
    assert!(data.sub_units.contains_key("light_armor"), "应加载 light_armor 营");
    // FRA 模板应加载
    assert!(data.templates.contains_key("Division d'Infanterie"), "应加载 FRA 步兵模板");
    // 装甲师汇总应能算出非零 armor(关键: light_armor 营不再被丢)
    let panzer = data.templates.get("Panzer-Division").expect("GER 装甲模板应存在");
    let stats = panzer.to_division_stats(&data);
    assert!(stats.armor > 0.0, "Panzer-Division armor 应 > 0 (light_armor 营已加载), 实际 {}", stats.armor);
}
```

> **注意:** 此测试在 Task 4 改 `to_division_stats` 签名前会编译失败(返回值变了)。先在 Step 3 改 loader,Task 4 再改签名。为避免编译失败,本 Task 先加 `assert!(data.sub_units.contains_key("light_armor"))` 这一行(不调用 to_division_stats),Task 4 完成后再补 armor 断言。

实际 Step 1 只加:
```rust
#[test]
fn t_load_all_has_light_armor_subunit() {
    let data = crate::data::loader::load_all();
    assert!(data.sub_units.contains_key("light_armor"), "应加载 light_armor 营");
}
#[test]
fn t_load_all_has_fra_templates() {
    let data = crate::data::loader::load_all();
    assert!(data.templates.contains_key("Division d'Infanterie"), "应加载 FRA 步兵模板");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --lib t_load_all_has_light_armor t_load_all_has_fra`
Expected: FAIL(light_armor 未加载 / FRA 模板不存在)

- [ ] **Step 3: 改 load_all 加载新文件**

修改 `src/data/loader.rs` 的 `load_all`(约 298 行),在阶段3 加 light_armor,阶段4 加 FRA:

```rust
    // 阶段3: 营定义(依赖装备)
    load_sub_units(&mut data, include_str!("../data_raw/units/infantry.txt"));
    load_sub_units(&mut data, include_str!("../data_raw/units/artillery.txt"));
    load_sub_units(&mut data, include_str!("../data_raw/units/medium_armor.txt"));
    load_sub_units(&mut data, include_str!("../data_raw/units/light_armor.txt"));

    // 阶段4: 模板(依赖营) — OOB 文件(history/units/*.txt)
    load_templates(&mut data, include_str!("../data_raw/history/GER.txt"));
    load_templates(&mut data, include_str!("../data_raw/history/FRA.txt"));
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test --lib t_load_all_has_light_armor t_load_all_has_fra`
Expected: PASS

- [ ] **Step 5: 跑全量测试确认无回归**

Run: `cargo test --lib`
Expected: 全部 PASS(180+ 测试,加新增 2 个)

- [ ] **Step 6: Commit**

```bash
git add src/data/loader.rs
git commit -m "feat(data): load_all 加载 light_armor 营 + FRA OOB 模板"
```

---

## Task 4: 修 to_division_stats 静默丢弃未知营 → 告警+跳过

**Files:**
- Modify: `src/data/template.rs:46-135`(`to_division_stats`)、`src/combat/commands.rs:168-176`、`src/combat/commands.rs:20-50`(`build_division_from_stats`)

- [ ] **Step 1: 写失败测试**

在 `src/data/template.rs` 的 `tests` 模块加测试:

```rust
#[test]
fn t_unknown_subunit_warned_not_silent() {
    use crate::data::equipment::EquipmentDef;
    let mut data = test_data();  // 只有 infantry 营
    // 模板引用了不存在的营 ghost_battalion
    let tmpl = DivisionTemplate {
        name: "mixed".into(),
        regiments: vec![
            RegimentEntry { sub_unit: "infantry".into(), x: 0, y: 0 },
            RegimentEntry { sub_unit: "ghost_battalion".into(), x: 1, y: 0 },
        ],
        support: vec![],
    };
    let (stats, warnings) = tmpl.to_division_stats(&data);
    // infantry 营正常汇总
    assert!((stats.soft_attack - 3.0).abs() < 1e-9, "已知营应正常汇总");
    // 未知营进告警列表
    assert_eq!(warnings.len(), 1, "应产生 1 条告警");
    assert!(warnings[0].contains("ghost_battalion"), "告警应含未知营名");
}
```

- [ ] **Step 2: 跑测试确认失败(编译错误)**

Run: `cargo test --lib t_unknown_subunit_warned_not_silent`
Expected: 编译失败(`to_division_stats` 返回 `DivisionStats` 不是元组)

- [ ] **Step 3: 改 to_division_stats 签名 + 收集告警**

修改 `src/data/template.rs:46` 的 `to_division_stats`:

```rust
    /// 汇总成 Division 所需属性。返回 (统计, 未知营告警列表)。
    /// 未知营(不在 sub_units 里)进告警列表并跳过, 不 panic(对齐 Paradox 容错哲学)。
    pub fn to_division_stats(&self, data: &GameData) -> (DivisionStats, Vec<String>) {
        let mut warnings = Vec::new();

        // 收集战斗营: 已知营进入汇总, 未知营告警+跳过
        let regiments: Vec<(&SubUnitDef, EquipStats)> = self
            .regiments
            .iter()
            .filter_map(|r| {
                match data.sub_units.get(&r.sub_unit) {
                    Some(su) => {
                        let stats = su.combat_stats(data);
                        Some((su, stats))
                    }
                    None => {
                        warnings.push(format!(
                            "模板 \"{}\" 引用未知营 \"{}\", 已跳过",
                            self.name, r.sub_unit
                        ));
                        None
                    }
                }
            })
            .collect();

        let mut stats = DivisionStats::default();

        // 求和类: soft/hard/defense/breakthrough/combat_width/max_strength/manpower
        for (su, cs) in &regiments {
            stats.soft_attack += cs.soft_attack;
            stats.hard_attack += cs.hard_attack;
            stats.defense += cs.defense;
            stats.breakthrough += cs.breakthrough;
            stats.combat_width += su.combat_width;
            stats.max_strength += su.max_strength;
            stats.manpower_need += su.manpower;
        }

        // 加权混合(60%平均 + 40%最高): armor / piercing
        let n = regiments.len() as f64;
        if n > 0.0 {
            let armor_sum: f64 = regiments.iter().map(|(_, cs)| cs.armor).sum();
            let armor_max = regiments.iter().map(|(_, cs)| cs.armor).fold(0.0f64, f64::max);
            stats.armor = 0.6 * (armor_sum / n) + 0.4 * armor_max;

            let pierce_sum: f64 = regiments.iter().map(|(_, cs)| cs.piercing).sum();
            let pierce_max = regiments.iter().map(|(_, cs)| cs.piercing).fold(0.0f64, f64::max);
            stats.piercing = 0.6 * (pierce_sum / n) + 0.4 * pierce_max;
        }

        // 加权平均(按 combat_width): hardness
        let total_cw: f64 = regiments.iter().map(|(su, _)| su.combat_width).sum();
        if total_cw > 0.0 {
            stats.hardness = regiments
                .iter()
                .map(|(su, cs)| cs.hardness * su.combat_width)
                .sum::<f64>()
                / total_cw;
        }

        // 加权平均(按权重, 战斗营权重=combat_width): org
        let total_w: f64 = regiments.iter().map(|(su, _)| su.combat_width).sum();
        if total_w > 0.0 {
            stats.max_org = regiments
                .iter()
                .map(|(su, _)| su.max_organisation * su.combat_width)
                .sum::<f64>()
                / total_w;
        }

        // 支援连: 已知营汇总, 未知营告警
        for se in &self.support {
            match data.sub_units.get(&se.sub_unit) {
                Some(su) => {
                    let cs = su.combat_stats(data);
                    stats.soft_attack += cs.soft_attack;
                    stats.hard_attack += cs.hard_attack;
                    stats.defense += cs.defense;
                    stats.breakthrough += cs.breakthrough;
                    stats.max_strength += su.max_strength;
                    stats.manpower_need += su.manpower;
                }
                None => {
                    warnings.push(format!(
                        "模板 \"{}\" 支援连引用未知营 \"{}\", 已跳过",
                        self.name, se.sub_unit
                    ));
                }
            }
        }

        // 装备需求聚合(已知营)
        for r in &self.regiments {
            if let Some(su) = data.sub_units.get(&r.sub_unit) {
                for (eq, qty) in &su.need {
                    *stats.equipment_need.entry(eq.clone()).or_insert(0.0) += qty;
                }
            }
        }
        for s in &self.support {
            if let Some(su) = data.sub_units.get(&s.sub_unit) {
                for (eq, qty) in &su.need {
                    *stats.equipment_need.entry(eq.clone()).or_insert(0.0) += qty;
                }
            }
        }

        (stats, warnings)
    }
```

- [ ] **Step 4: 修 build_division_from_stats 调用点**

修改 `src/combat/commands.rs:168-176`(create_division 的 template 路径):

```rust
        if let Some(tmpl_name) = ParamGet::get(p, "template").and_then(Arg::as_str) {
            // 新路径: 数据驱动汇总
            let (stats, warnings) = match w.data.templates.get(tmpl_name) {
                Some(t) => t.to_division_stats(&w.data),
                None => return Err(CmdError::RuntimeError(format!("未知模板: {tmpl_name}"))),
            };
            // 告警透传到 stderr(不阻断建师)
            for warn in &warnings {
                eprintln!("[create_division] ⚠️ {warn}");
            }
            let d = build_division_from_stats(owner, loc, stats);
            w.add_division(d);
            return Ok(());
        }
```

- [ ] **Step 5: 修 template.rs 内其他 to_division_stats 调用点**

搜索 `src/` 下所有 `to_division_stats` 调用,改为解构元组:
```bash
grep -rn "to_division_stats" src/
```
对每个调用点(测试代码 `t_seven_infantry_division_stats`、`t_armor_weighted_blend`、`t_support_zero_width`),改:
```rust
let s = tmpl.to_division_stats(&data);
```
为:
```rust
let (s, _warnings) = tmpl.to_division_stats(&data);
```

- [ ] **Step 6: 跑测试确认通过**

Run: `cargo test --lib t_unknown_subunit_warned_not_silent`
Expected: PASS

- [ ] **Step 7: 跑全量测试确认无回归**

Run: `cargo test --lib`
Expected: 全部 PASS

- [ ] **Step 8: Commit**

```bash
git add src/data/template.rs src/combat/commands.rs
git commit -m "fix(template): to_division_stats 未知营告警+跳过(不再静默归零)"
```

---

## Task 5: 验证并修复模块汇总数值(light_tank_chassis hardness/soft_attack)

**Files:**
- Modify: `src/data/equipment.rs`(`compute_equipment_stats` / `extract_stats`)

> 本 Task 根因需定位,先写诊断测试暴露问题,再读代码定位修复。

- [ ] **Step 1: 写诊断测试**

在 `src/data/equipment.rs` 的 `tests` 模块加:

```rust
#[test]
fn t_light_tank_chassis_hardness_not_zero() {
    // 加载真实 tank_chassis.txt, 验证 light_tank 型号的 hardness/soft_attack 正确
    let mut data = crate::data::GameData::default();
    crate::data::loader::load_modules(
        &mut data,
        include_str!("../data_raw/modules/00_tank_modules.txt"),
    );
    crate::data::loader::load_chassis(
        &mut data,
        include_str!("../data_raw/equipment/tank_chassis.txt"),
    );
    // light_tank_chassis archetype hardness=0.8(文件里写的)
    // light_tank_chassis_2 是 1936 型号, 应继承合理 hardness
    let e = data.equipment.get("light_tank_chassis_2")
        .expect("应产出 light_tank_chassis_2 型号");
    // 坦克硬度应较高(>0.5), soft_attack 应 > 0(有主武器模块)
    assert!(
        e.stats.hardness > 0.5,
        "light_tank_chassis_2 hardness 应 > 0.5, 实际 {} (疑似模块汇总 bug)",
        e.stats.hardness
    );
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --lib t_light_tank_chassis_hardness_not_zero`
Expected: FAIL(hardness 实际为 0 或很小)

- [ ] **Step 3: 读 compute_equipment_stats + extract_stats 定位根因**

读 `src/data/equipment.rs` 的 `compute_equipment_stats`(加法后乘法)和 `extract_stats`(从 Block 提取 EquipStats)。对照 `tank_chassis.txt` 的 `light_tank_chassis` 块:
- archetype 有 `hardness = 0.8`、`armor_value = 10` 等(在块顶层,非 add_stats)
- 型号 `light_tank_chassis_2` 有 `archetype = light_tank_chassis` + 可能覆盖数值

确认 `extract_stats` 是否提取了顶层的 `hardness`/`armor_value`/`soft_attack`(字段名映射: `armor_value` → `armor`, `ap_attack` → `piercing`)。确认 `build_equipment`(loader.rs)的 base_stats 来源。

- [ ] **Step 4: 修根因**

根据 Step 3 定位的根因修复 `extract_stats` 或 `build_equipment`。常见可能:
- `extract_stats` 没提取顶层 `hardness`(只看 add_stats/multiply_stats 块)→ 加顶层提取
- 字段名映射缺失(`armor_value` 没映射到 `armor`)→ 补映射
- `build_equipment` 的 base_stats 取错(型号无 own stats 时没继承 archetype 的)

- [ ] **Step 5: 跑诊断测试确认通过**

Run: `cargo test --lib t_light_tank_chassis_hardness_not_zero`
Expected: PASS

- [ ] **Step 6: 跑全量测试确认无回归**

Run: `cargo test --lib`
Expected: 全部 PASS

- [ ] **Step 7: Commit**

```bash
git add src/data/equipment.rs
git commit -m "fix(equipment): 修模块汇总 hardness/soft_attack 异常(light_tank_chassis)"
```

---

## Task 6: Division 加 template_name 字段

**Files:**
- Modify: `src/runtime/entities.rs:82-111`(Division struct)、`src/combat/commands.rs:20-50`(build_division_from_stats)、`src/combat/commands.rs:168-176`(create_division template 路径)

- [ ] **Step 1: 写失败测试**

在 `src/combat/commands.rs` 的 `tests` 模块(若无则在 `src/runtime/entities.rs` 加)写:

```rust
#[test]
fn t_create_division_records_template_name() {
    use crate::runtime::Registry;
    let mut reg = Registry::new();
    crate::combat::commands::register(&mut reg);
    let mut w = crate::runtime::World::new();
    // 建省(建师需要 location 存在)
    let setup = "create_state = { id = 1 owner = GER }
                 create_province = { id = 1 state = 1 }";
    let b = crate::parser::parse(setup).unwrap();
    let effs = crate::ast::lower::lower_effects(&b);
    let mut interp = crate::runtime::Interpreter::new(reg);
    interp.run(&effs, &mut w);
    // 用 template 建师
    let cmd = "create_division = { owner = GER location = 1 template = \"Infanterie-Division\" }";
    let b2 = crate::parser::parse(cmd).unwrap();
    let effs2 = crate::ast::lower::lower_effects(&b2);
    interp.run(&effs2, &mut w);
    // 师应记录 template_name
    let div = w.divisions.values().next().expect("应建出师");
    assert_eq!(div.template_name.as_deref(), Some("Infanterie-Division"));
}
```

- [ ] **Step 2: 跑测试确认失败(编译错误)**

Run: `cargo test --lib t_create_division_records_template_name`
Expected: 编译失败(Division 无 template_name 字段)

- [ ] **Step 3: Division 加字段**

修改 `src/runtime/entities.rs`,在 Division struct 的 `modifiers` 字段后加:

```rust
    /// modifier 汇总(堑壕/计划/经验等师自身修正)
    pub modifiers: crate::combat::modifier::ModifierStack,
    /// 师所用模板名(None = 旧路径 battalions/手填建的, 无模板引用)
    /// 换模板(change_template)时更新, 用于 edit_template 联动
    pub template_name: Option<String>,
```

- [ ] **Step 4: 改 build_division_from_stats 签名**

修改 `src/combat/commands.rs:20`:

```rust
/// 从汇总属性构建 Division(新路径: 数据驱动)
fn build_division_from_stats(owner: &str, loc: u32, stats: DivisionStats, template: Option<&str>) -> Division {
    let mut eq_need = std::collections::HashMap::new();
    let mut eq_held = std::collections::HashMap::new();
    for (eq, qty) in &stats.equipment_need {
        eq_need.insert(eq.clone(), *qty);
        eq_held.insert(eq.clone(), *qty);  // 建师时满编
    }
    Division {
        id: 0,
        owner_tag: owner.into(),
        location_province: loc,
        soft_attack: stats.soft_attack,
        hard_attack: stats.hard_attack,
        defense: stats.defense,
        breakthrough: stats.breakthrough,
        armor: stats.armor,
        piercing: stats.piercing,
        hardness: stats.hardness,
        combat_width: stats.combat_width,
        max_org: stats.max_org,
        org: stats.max_org,
        max_strength: stats.max_strength,
        strength: stats.max_strength,
        equipment_need: eq_need,
        equipment_held: eq_held,
        manpower_need: stats.manpower_need,
        manpower_held: stats.manpower_need,
        order: OrderState::Idle,
        modifiers: Default::default(),
        template_name: template.map(|s| s.to_string()),
    }
}
```

- [ ] **Step 5: 改 create_division template 路径调用**

修改 `src/combat/commands.rs` 的 template 分支(Task 4 改过的):

```rust
        if let Some(tmpl_name) = ParamGet::get(p, "template").and_then(Arg::as_str) {
            let (stats, warnings) = match w.data.templates.get(tmpl_name) {
                Some(t) => t.to_division_stats(&w.data),
                None => return Err(CmdError::RuntimeError(format!("未知模板: {tmpl_name}"))),
            };
            for warn in &warnings {
                eprintln!("[create_division] ⚠️ {warn}");
            }
            let d = build_division_from_stats(owner, loc, stats, Some(tmpl_name));
            w.add_division(d);
            return Ok(());
        }
```

同时改旧 battalions/手填路径的 `build_division_from_stats` 调用(传 `None`):
```rust
let d = build_division_from_stats(owner, loc, /* stats struct */, None);
```

- [ ] **Step 6: 跑测试确认通过**

Run: `cargo test --lib t_create_division_records_template_name`
Expected: PASS

- [ ] **Step 7: 跑全量测试确认无回归**

Run: `cargo test --lib`
Expected: 全部 PASS

- [ ] **Step 8: Commit**

```bash
git add src/runtime/entities.rs src/combat/commands.rs
git commit -m "feat(division): Division 加 template_name 引用(师↔模板从拷贝改引用)"
```

---

## Task 7: 新增 change_template 命令

**Files:**
- Modify: `src/combat/commands.rs`(register 函数内加新命令)

- [ ] **Step 1: 写失败测试**

在 `src/combat/commands.rs` 的 `tests` 模块加:

```rust
#[test]
fn t_change_template_updates_stats_keeps_runtime() {
    use crate::runtime::Registry;
    let mut reg = Registry::new();
    crate::combat::commands::register(&mut reg);
    let mut w = crate::runtime::World::new();
    let setup = "create_state = { id = 1 owner = GER }
                 create_province = { id = 1 state = 1 }";
    let b = crate::parser::parse(setup).unwrap();
    let effs = crate::ast::lower::lower_effects(&b);
    let mut interp = crate::runtime::Interpreter::new(reg);
    interp.run(&effs, &mut w);
    // 建步兵师
    interp.run(&crate::ast::lower::lower_effects(
        &crate::parser::parse("create_division = { owner = GER location = 1 template = \"Infanterie-Division\" }").unwrap()
    ), &mut w);
    let div_id = w.divisions.values().next().unwrap().id;
    // 改它的 org/strength(模拟战斗后运行态)
    {
        let d = w.divisions.get_mut(&div_id).unwrap();
        d.org = 30.0;
        d.strength = 100.0;
    }
    let inf_armor_before = w.divisions.get(&div_id).unwrap().armor;
    // 换装甲模板
    interp.run(&crate::ast::lower::lower_effects(
        &crate::parser::parse("change_template = { division = 1 template = \"Panzer-Division\" }").unwrap()
    ), &mut w);
    let d = w.divisions.get(&div_id).unwrap();
    // 装甲师 armor 应变高(> 步兵师)
    assert!(d.armor > inf_armor_before, "换装甲模板后 armor 应升高");
    // template_name 更新
    assert_eq!(d.template_name.as_deref(), Some("Panzer-Division"));
    // 运行态保留(org/strength 不变)
    assert!((d.org - 30.0).abs() < 1e-9, "换模板应保留 org");
    assert!((d.strength - 100.0).abs() < 1e-9, "换模板应保留 strength");
}
```

> 注意:`division = 1` 假设第一个师 id=1。`add_division` 从 1 开始编号,验证 `world.rs:119` 的 next_division_id 逻辑。

- [ ] **Step 2: 跑测试确认失败(命令不存在)**

Run: `cargo test --lib t_change_template_updates_stats_keeps_runtime`
Expected: FAIL(change_template 未注册)

- [ ] **Step 3: 注册 change_template 命令**

在 `src/combat/commands.rs` 的 `register` 函数内(其他命令注册后)加:

```rust
    // 换师的模板(重新汇总数值, 保留运行态 location/org/strength)
    reg.register("change_template", |w, p| {
        let div_id = num_of(np(p, "change_template", "division")?)? as u64;
        let tmpl_name = np(p, "change_template", "template")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("template 应为字符串".into()))?;
        let (stats, warnings) = match w.data.templates.get(tmpl_name) {
            Some(t) => t.to_division_stats(&w.data),
            None => return Err(CmdError::RuntimeError(format!("未知模板: {tmpl_name}"))),
        };
        for warn in &warnings {
            eprintln!("[change_template] ⚠️ {warn}");
        }
        let d = w.divisions.get_mut(&div_id)
            .ok_or_else(|| CmdError::RuntimeError(format!("师 #{div_id} 不存在")))?;
        // 覆盖战斗属性 + 装备需求(满编重算)
        d.soft_attack = stats.soft_attack;
        d.hard_attack = stats.hard_attack;
        d.defense = stats.defense;
        d.breakthrough = stats.breakthrough;
        d.armor = stats.armor;
        d.piercing = stats.piercing;
        d.hardness = stats.hardness;
        d.combat_width = stats.combat_width;
        d.max_org = stats.max_org;
        d.max_strength = stats.max_strength;
        d.manpower_need = stats.manpower_need;
        // 装备需求更新(held 保持当前持有, 不强制满编——换模板可能缺装备)
        d.equipment_need = stats.equipment_need.keys().map(|k| (k.clone(), 0.0)).collect();
        for (eq, qty) in &stats.equipment_need {
            d.equipment_need.insert(eq.clone(), *qty);
        }
        d.template_name = Some(tmpl_name.to_string());
        // 运行态保留: location_province / org / strength / order / modifiers 不动
        Ok(())
    });
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test --lib t_change_template_updates_stats_keeps_runtime`
Expected: PASS

- [ ] **Step 5: 跑全量测试确认无回归**

Run: `cargo test --lib`
Expected: 全部 PASS

- [ ] **Step 6: Commit**

```bash
git add src/combat/commands.rs
git commit -m "feat(command): change_template 命令(换模板重汇总+保留运行态)"
```

---

## Task 8: WASM 契约扩展(change_template + get_templates + get_state 补字段)

**Files:**
- Modify: `src/wasm_api.rs`

- [ ] **Step 1: 加 engine_change_template FFI**

在 `src/wasm_api.rs` 加(参照现有 `engine_move_division` 模式):

```rust
/// 换师的模板(前端"换模板"功能)
#[no_mangle]
pub unsafe extern "C" fn engine_change_template(
    division_id: u32,
    template_ptr: *const u8,
    template_len: usize,
) {
    let template = unsafe { ptr_to_str(template_ptr, template_len) };
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Engine { interp, world } = &mut *e;
        let script = format!("change_template = {{ division = {division_id} template = \"{template}\" }}");
        if let Ok(b) = crate::parser::parse(&script) {
            let effs = crate::ast::lower::lower_effects(&b);
            interp.run(&effs, world);
        }
    });
}
```

- [ ] **Step 2: 加 engine_declare_war / create_faction / join_faction / white_peace FFI**

参照 `engine_supply` 的 ptr_to_str 模式,各加一个 FFI。例:

```rust
/// 宣战(前端外交面板)
#[no_mangle]
pub unsafe extern "C" fn engine_declare_war(
    attacker_ptr: *const u8, attacker_len: usize,
    defender_ptr: *const u8, defender_len: usize,
) {
    let attacker = unsafe { ptr_to_str(attacker_ptr, attacker_len) };
    let defender = unsafe { ptr_to_str(defender_ptr, defender_len) };
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Engine { interp, world } = &mut *e;
        let script = format!("declare_war = {{ attacker = {attacker} defender = {defender} }}");
        if let Ok(b) = crate::parser::parse(&script) {
            let effs = crate::ast::lower::lower_effects(&b);
            interp.run(&effs, world);
        }
    });
}
```

create_faction / join_faction / white_peace 同理(参数:tag 或 双 tag)。

- [ ] **Step 3: 加 engine_get_templates FFI**

```rust
/// 取所有可用模板名(JSON 数组, null 终止)。部署面板下拉用, 启动后不变。
#[no_mangle]
pub extern "C" fn engine_get_templates() -> *const u8 {
    let json = ENGINE.with(|e| {
        let names: Vec<&String> = e.borrow().world.data.templates.keys().collect();
        let mut s = String::from("[");
        let mut first = true;
        for n in names {
            if !first { s.push(','); }
            first = false;
            s.push_str(&format!("\"{}\"", n.replace('"', "\\\"")));
        }
        s.push(']');
        s
    });
    STATE_BUF.with(|buf| {
        let mut b = buf.borrow_mut();
        *b = json.into_bytes();
        b.push(0);
        b.as_ptr()
    })
}
```

- [ ] **Step 4: get_state 补 date/wars/factions 字段 + division 补 template**

修改 `serialize_state`(约 279 行)。

date(用 World.date() 派生,替代裸 hour 的补充):在 hour 后加
```rust
    s.push_str(&world.hour.to_string());
    // 日期(精确公历, 从 hour 派生)
    let date = world.date();
    s.push_str(&format!(",\"date\":{{\"y\":{},\"m\":{},\"d\":{}}}", date.year, date.month, date.day));
```

division 序列化加 template(在 path 后):
```rust
            // template_name(数据驱动建师有, 旧路径无)
            let tmpl = d.template_name.as_deref().map(|s| s.replace('"', "\\\"")).unwrap_or_default();
            // ... format 里加 "template":"{}"
```
(把 template 加进现有的 format! 字符串)

wars + factions(在 provinces 块后):
```rust
    // 战争列表
    s.push_str(",\"wars\":[");
    let mut wfirst = true;
    for war in &world.wars {
        if !wfirst { s.push(','); }
        wfirst = false;
        let atk: Vec<String> = war.attackers.iter().cloned().collect();
        let def: Vec<String> = war.defenders.iter().cloned().collect();
        s.push_str(&format!(
            "{{\"id\":{},\"atk\":[{}],\"def\":[{}]}}",
            war.id,
            atk.iter().map(|t| format!("\"{t}\"")).collect::<Vec<_>>().join(","),
            def.iter().map(|t| format!("\"{t}\"")).collect::<Vec<_>>().join(",")
        ));
    }
    s.push_str("]");
    // 阵营映射(tag → faction 名)
    s.push_str(",\"factions\":{");
    let mut ffirst = true;
    for (tag, country) in &world.countries {
        if let Some(fac) = &country.faction {
            if !ffirst { s.push(','); }
            ffirst = false;
            s.push_str(&format!("\"{tag}\":\"{}\"", fac.replace('"', "\\\"")));
        }
    }
    s.push_str("}");
```

- [ ] **Step 5: 编译 WASM 确认无误**

Run: `cargo build --target wasm32-unknown-unknown --lib --release`
Expected: 编译成功

- [ ] **Step 6: 拷贝 wasm 到 web/**

```bash
cp target/wasm32-unknown-unknown/release/hoi4_clone.wasm web/
```

- [ ] **Step 7: Commit**

```bash
git add src/wasm_api.rs web/hoi4_clone.wasm
git commit -m "feat(wasm): change_template/declare_war/get_templates FFI + get_state 补 date/wars/factions"
```

---

## Task 9: UI 骨架 — index.html + main.js + 四层目录 + 空壳跑通

**Files:**
- Rewrite: `web/index.html`
- Create: `web/css/app.css`、`web/js/main.js`、`web/js/engine/wasm.js`、`web/js/engine/state.js`、`web/js/engine/commands.js`

> 备份旧 index.html(其逻辑在 Task 11-14 迁移到新模块)。

- [ ] **Step 1: 备份旧 UI**

```bash
cp web/index.html web/index.html.bak
```

- [ ] **Step 2: 写 index.html(根容器 + module 加载)**

`web/index.html`:
```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1, user-scalable=no">
<title>hoi4-clone</title>
<link rel="stylesheet" href="css/app.css">
</head>
<body>
<div id="app">
  <div id="loading">正在加载 WASM 引擎...</div>
  <div id="game" style="display:none">
    <div id="topbar"></div>
    <canvas id="map"></canvas>
    <div id="bottombar"></div>
    <div id="drawer"></div>
    <div id="panel-host"></div>
    <div id="order-menu"></div>
  </div>
</div>
<script type="module" src="js/main.js"></script>
</body>
</html>
```

- [ ] **Step 3: 写 app.css(移动优先全屏布局)**

`web/css/app.css`(核心):
```css
* { box-sizing: border-box; margin: 0; padding: 0; }
html, body { width: 100%; height: 100%; overflow: hidden;
  font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
  background: #1a1a2e; color: #e0e0e0; }
#app { width: 100vw; height: 100vh; position: relative; }
#map { position: absolute; inset: 0; width: 100%; height: 100%; touch-action: none; }
#topbar { position: absolute; top: 0; left: 0; right: 0; height: 48px;
  background: rgba(22,33,62,0.92); border-bottom: 1px solid #0f3460;
  display: flex; align-items: center; padding: 0 8px; gap: 6px; z-index: 10; }
#bottombar { position: absolute; bottom: 0; left: 0; right: 0; height: 56px;
  background: rgba(22,33,62,0.92); border-top: 1px solid #0f3460;
  display: flex; align-items: center; justify-content: center; gap: 8px; z-index: 10; }
#drawer { position: absolute; bottom: 56px; left: 0; right: 0; max-height: 50vh;
  background: #16213e; border-top: 1px solid #e94560; overflow-y: auto; z-index: 9;
  transform: translateY(100%); transition: transform 0.25s ease; }
#drawer.open { transform: translateY(0); }
#panel-host { position: absolute; top: 48px; left: 0; bottom: 0; width: 360px;
  background: #16213e; border-right: 1px solid #0f3460; z-index: 8;
  transform: translateX(-100%); transition: transform 0.25s ease; overflow-y: auto; }
#panel-host.open { transform: translateX(0); }
button { background: #e94560; color: white; border: none; border-radius: 4px;
  cursor: pointer; font-size: 13px; min-height: 44px; min-width: 44px; padding: 0 12px; }
button.secondary { background: #0f3460; }
button.secondary:hover { background: #1a4a80; }
```

- [ ] **Step 4: 写 engine/wasm.js(迁移现有 WASM 封装)**

`web/js/engine/wasm.js`:从 `index.html.bak` 提取 `loadWasm`/`readCString`/`passStr`,导出:
```js
let wasm = null;
export async function loadWasm() {
  const resp = await fetch('hoi4_clone.wasm?v=' + Date.now());
  const bytes = await resp.arrayBuffer();
  const result = await WebAssembly.instantiate(bytes, { env: {} });
  wasm = result.instance;
  return wasm;
}
export function getWasm() { return wasm; }
export function readCString(ptr) {
  const mem = new Uint8Array(wasm.exports.memory.buffer);
  let end = ptr;
  while (mem[end] !== 0) end++;
  return new TextDecoder('utf-8').decode(mem.subarray(ptr, end));
}
export function passStr(str) {
  const bytes = new TextEncoder().encode(str);
  const ptr = wasm.exports.engine_alloc(bytes.length);
  new Uint8Array(wasm.exports.memory.buffer).set(bytes, ptr);
  return { ptr, len: bytes.length };
}
```

- [ ] **Step 5: 写 engine/state.js(getState 解析)**

`web/js/engine/state.js`:
```js
import { getWasm, readCString } from './wasm.js';
export function getState() {
  const ptr = getWasm().exports.engine_get_state();
  return JSON.parse(readCString(ptr));
}
export function getTemplates() {
  const ptr = getWasm().exports.engine_get_templates();
  return JSON.parse(readCString(ptr));
}
```

- [ ] **Step 6: 写 engine/commands.js(命令封装)**

`web/js/engine/commands.js`:
```js
import { getWasm, passStr } from './wasm.js';
const e = () => getWasm().exports;
export function tick(h) { e().engine_tick(h); }
export function reset() { e().engine_reset(); }
export function setPlayer(tag) {
  const t = passStr(tag); e().engine_set_player(t.ptr, t.len);
}
export function runSetup(script) {
  const s = passStr(script); return e().engine_run_setup(s.ptr, s.len);
}
export function deployTemplate(owner, loc, template) {
  const o = passStr(owner), t = passStr(template);
  e().engine_deploy_template(o.ptr, o.len, loc, t.ptr, t.len);
}
export function changeTemplate(divId, template) {
  const t = passStr(template);
  e().engine_change_template(divId, t.ptr, t.len);
}
export function declareWar(attacker, defender) {
  const a = passStr(attacker), d = passStr(defender);
  e().engine_declare_war(a.ptr, a.len, d.ptr, d.len);
}
export function moveDivision(divId, target) { e().engine_move_division(divId, target); }
export function supportAttack(divId, target) { e().engine_support_attack(divId, target); }
export function queueMove(divId, target) { e().engine_queue_move(divId, target); }
export function stopOrder(divId) { e().engine_stop_order(divId); }
```

> 注意:`engine_deploy_template` 是 Task 8 新增的 FFI。若 Task 8 尚未编译含此函数的 wasm,这里会运行时报错——确保 Task 8 在前。

- [ ] **Step 7: 写 main.js(启动装配,空壳先跑通)**

`web/js/main.js`:
```js
import { loadWasm } from './engine/wasm.js';
import { getState, getTemplates } from './engine/state.js';
import { setPlayer, runSetup } from './engine/commands.js';

async function main() {
  await loadWasm();
  document.getElementById('loading').style.display = 'none';
  document.getElementById('game').style.display = 'block';
  // 初始化场景(占位: 后续 Task 加完整 setup)
  setPlayer('GER');
  const setup = `create_state = { id = 1 owner = GER }
create_province = { id = 1 state = 1 }`;
  runSetup(setup);
  console.log('templates:', getTemplates());
  console.log('state:', getState());
}
main();
```

- [ ] **Step 8: 跑起来验证空壳**

```bash
cd web && python -m http.server 8765
```
浏览器开 `http://127.0.0.1:8765`,确认:
- 加载成功(不卡在 loading)
- 控制台打印 templates 列表(含 Infanterie-Division 等)和 state(含 date/wars/factions 字段)
- 页面显示空地图 canvas + 空顶栏/底栏

- [ ] **Step 9: Commit**

```bash
git add web/index.html web/css/ web/js/
git commit -m "feat(ui): 骨架(index.html + 四层目录 + WASM 封装空壳跑通)"
```

---

## Task 10: core 框架层 — store/bind/router/el

**Files:**
- Create: `web/js/core/store.js`、`web/js/core/bind.js`、`web/js/core/router.js`、`web/js/core/el.js`

- [ ] **Step 1: 写 core/el.js(hyperscript 造 DOM)**

`web/js/core/el.js`:
```js
// h(tag, props, children) — 创建 DOM 元素
export function h(tag, props = {}, children = []) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (k === 'class') el.className = v;
    else if (k === 'style' && typeof v === 'object') Object.assign(el.style, v);
    else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2), v);
    else if (k === 'text') el.textContent = v;
    else el.setAttribute(k, v);
  }
  for (const c of [].concat(children)) {
    if (c == null) continue;
    el.append(c.nodeType ? c : document.createTextNode(c));
  }
  return el;
}
export function clear(el) { while (el.firstChild) el.removeChild(el.firstChild); }
```

- [ ] **Step 2: 写 core/store.js(视图状态容器 + changeset 应用)**

`web/js/core/store.js`:
```js
// 视图状态容器: 持有完整 viewModel, 接收 changeset 打补丁, 通知订阅者
export const store = {
  state: null,           // 完整 viewModel
  listeners: new Map(),  // path → Set<fn>
  setState(next) {
    const prev = this.state;
    this.state = next;
    // 简化: 全量通知所有订阅(优化留 Task 12 脏标记)
    for (const fns of this.listeners.values()) {
      for (const fn of fns) fn(next);
    }
  },
};
// bind: 订阅 path, 数据变时调用 fn(newValue, fullState)
export function bind(path, fn) {
  if (!store.listeners.has(path)) store.listeners.set(path, new Set());
  store.listeners.get(path).add(fn);
  // 立即触发一次(初始化)
  if (store.state) fn(store.state);
  return () => store.listeners.get(path).delete(fn);  // 返回取消订阅
}
```

> 注:Task 12 会把"全量通知"优化为"changeset 路径级脏标记"。本 Task 先跑通绑定链路。

- [ ] **Step 3: 写 core/bind.js(绑定原语)**

`web/js/core/bind.js`:
```js
import { bind } from './store.js';
import { h } from './el.js';

// bindText(path, fn): 数据变 → fn 收到值
export function bindText(path, fn) { return bind(path, (s) => fn(resolve(s, path))); }
// bindWhen(path, pred): 满足 pred 显示元素, 否则隐藏
export function bindWhen(el, path, pred) {
  return bind(path, (s) => { el.style.display = pred(resolve(s, path)) ? '' : 'none'; });
}
// bindEnabled(el, path, pred): 满足 pred 启用, 否则灰掉
export function bindEnabled(el, path, pred) {
  return bind(path, (s) => { el.disabled = !pred(resolve(s, path)); });
}
// bindList(path, renderItem): path 解析为数组, 每项渲染
export function bindList(container, path, renderItem) {
  return bind(path, (s) => {
    const arr = resolve(s, path) || [];
    container.innerHTML = '';
    for (const item of arr) container.append(renderItem(item));
  });
}
// resolve({a:{b:[1]}}, 'a.b.0') → 1
function resolve(obj, path) {
  return path.split('.').reduce((o, k) => (o == null ? o : o[k]), obj);
}
```

- [ ] **Step 4: 写 core/router.js(面板路由)**

`web/js/core/router.js`:
```js
const panels = new Map();  // name → { open(), close() }
let current = null;
const host = () => document.getElementById('panel-host');

export function register(name, panel) { panels.set(name, panel); }
export function open(name) {
  if (current && panels.has(current)) panels.get(current).close();
  current = name;
  const p = panels.get(name);
  if (p) { p.open(); host().classList.add('open'); }
}
export function close() {
  if (current && panels.has(current)) panels.get(current).close();
  current = null;
  host().classList.remove('open');
}
export function names() { return [...panels.keys()]; }
```

- [ ] **Step 5: 在 main.js 接入 store + tick 循环**

改 `web/js/main.js`,加 tick 刷新 store:
```js
import { store } from './core/store.js';
import { getState } from './engine/state.js';
import { tick } from './engine/commands.js';

// 主刷新循环: tick 后拉 state 灌入 store
let autoTimer = null;
export function refresh() {
  store.setState(getState());
}
export function doTick(hours) {
  tick(hours);
  refresh();
}
export function toggleTime() {
  if (autoTimer) { clearInterval(autoTimer); autoTimer = null; return false; }
  autoTimer = setInterval(() => doTick(1), 200);
  return true;
}
```
在 main() 末尾加 `refresh();` 初始化 store。

- [ ] **Step 6: 验证绑定链路(临时测试)**

在 main.js 临时加:
```js
import { bindText } from './core/bind.js';
bindText('date', (d) => console.log('date changed:', d));
```
浏览器刷新,确认控制台打印 date 对象。tick 一次确认 date 变化触发。

- [ ] **Step 7: Commit**

```bash
git add web/js/core/ web/js/main.js
git commit -m "feat(ui): core 框架层(store/bind/router/el + tick 刷新循环)"
```

---

## Task 11: core/canvas.js 管家 + core/input.js 统一输入

**Files:**
- Create: `web/js/core/canvas.js`、`web/js/core/input.js`

- [ ] **Step 1: 写 core/canvas.js(相机 + 图层注册 + 坐标转换)**

`web/js/core/canvas.js`:
```js
// Canvas 管家: 相机(pan/zoom) + 图层注册 + 坐标转换 + 脏标记重绘
const layers = [];  // [{ name, z, draw, dirty }]
const camera = { x: 0, y: 0, zoom: 1 };
let canvas, ctx, dpr;
let fullRedraw = true;

export function init() {
  canvas = document.getElementById('map');
  dpr = window.devicePixelRatio || 1;
  ctx = canvas.getContext('2d');
  resize();
  window.addEventListener('resize', resize);
}
function resize() {
  const W = canvas.clientWidth, H = canvas.clientHeight;
  canvas.width = W * dpr; canvas.height = H * dpr;
  fullRedraw = true;
}
export function addLayer(name, z, drawFn) {
  layers.push({ name, z, draw: drawFn, dirty: true });
  layers.sort((a, b) => a.z - b.z);
}
export function markDirty(layerName) {
  const l = layers.find(l => l.name === layerName);
  if (l) l.dirty = true;
}
export function markAllDirty() { fullRedraw = true; }
export function pan(dx, dy) { camera.x += dx; camera.y += dy; fullRedraw = true; }
export function zoomBy(f, cx, cy) {
  // 以屏幕点 (cx,cy) 为锚点缩放
  const wx = (cx - camera.x) / camera.zoom;
  const wy = (cy - camera.y) / camera.zoom;
  camera.zoom = Math.max(0.3, Math.min(5, camera.zoom * f));
  camera.x = cx - wx * camera.zoom;
  camera.y = cy - wy * camera.zoom;
  fullRedraw = true;
}
export function resetCamera() { camera.x = 0; camera.y = 0; camera.zoom = 1; fullRedraw = true; }
// 世界坐标 → 屏幕坐标
export function worldToScreen(p) {
  return { x: p.x * camera.zoom + camera.x, y: p.y * camera.zoom + camera.y };
}
// 屏幕坐标 → 世界坐标(hit-test 用)
export function screenToWorld(p) {
  return { x: (p.x - camera.x) / camera.zoom, y: (p.y - camera.y) / camera.zoom };
}
export function render(view) {
  const W = canvas.clientWidth, H = canvas.clientHeight;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, W, H);
  for (const l of layers) {
    if (fullRedraw || l.dirty) {
      l.draw(ctx, view, { worldToScreen, camera, W, H });
      l.dirty = false;
    }
  }
  fullRedraw = false;
}
export { worldToScreen, screenToWorld };
```

- [ ] **Step 2: 写 core/input.js(PointerEvent 统一 + 手势)**

`web/js/core/input.js`:
```js
// 统一输入: PointerEvent 归一化鼠标+触屏 + 手势识别
import { pan, zoomBy, resetCamera, screenToWorld } from './canvas.js';

let pointers = new Map();  // pointerId → {x,y}
let lastPan = null;
let pinchStart = null;
const HIT_RADIUS = 44;
const onProvinceHit = [];  // 注册的命中回调
const onBackgroundClick = [];

export function init() {
  const canvas = document.getElementById('map');
  canvas.addEventListener('pointerdown', onDown);
  canvas.addEventListener('pointermove', onMove);
  canvas.addEventListener('pointerup', onUp);
  canvas.addEventListener('pointercancel', onUp);
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    zoomBy(e.deltaY < 0 ? 1.1 : 0.9, e.clientX - canvas.getBoundingClientRect().left,
           e.clientY - canvas.getBoundingClientRect().top);
  }, { passive: false });
}
function onDown(e) {
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
  if (pointers.size === 1) {
    lastPan = { x: e.clientX, y: e.clientY, moved: false };
  } else if (pointers.size === 2) {
    const pts = [...pointers.values()];
    pinchStart = { dist: Math.hypot(pts[0].x - pts[1].x, pts[0].y - pts[1].y),
                   cx: (pts[0].x + pts[1].x) / 2, cy: (pts[0].y + pts[1].y) / 2 };
  }
}
function onMove(e) {
  if (!pointers.has(e.pointerId)) return;
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
  if (pointers.size === 1 && lastPan) {
    const dx = e.clientX - lastPan.x, dy = e.clientY - lastPan.y;
    if (Math.hypot(dx, dy) > 3) lastPan.moved = true;
    pan(dx, dy);
    lastPan.x = e.clientX; lastPan.y = e.clientY;
  } else if (pointers.size === 2 && pinchStart) {
    const pts = [...pointers.values()];
    const d = Math.hypot(pts[0].x - pts[1].x, pts[0].y - pts[1].y);
    zoomBy(d / pinchStart.dist, pinchStart.cx, pinchStart.cy);
    pinchStart.dist = d;
  }
}
function onUp(e) {
  const start = pointers.get(e.pointerId);
  pointers.delete(e.pointerId);
  // 单指短按未移动 = 点击
  if (pointers.size === 0 && lastPan && !lastPan.moved) {
    handleClick(e.clientX, e.clientY);
  }
  if (pointers.size === 1) { lastPan = [...pointers.values()][0]; lastPan.moved = true; pinchStart = null; }
  if (pointers.size === 0) { lastPan = null; pinchStart = null; }
}
function handleClick(cx, cy) {
  const canvas = document.getElementById('map');
  const rect = canvas.getBoundingClientRect();
  const sx = cx - rect.left, sy = cy - rect.top;
  const wp = screenToWorld({ x: sx, y: sy });
  // 命中回调(按注册顺序,第一个命中者消费)
  for (const fn of [...onProvinceHit].reverse()) {
    if (fn(wp, sx, sy)) return;
  }
  for (const fn of onBackgroundClick) fn();
}
export function onProvinceHit(fn) { onProvinceHit.push(fn); }
export function onBackgroundClick(fn) { onBackgroundClick.push(fn); }
```

- [ ] **Step 3: main.js 接入 canvas + input + 渲染循环**

在 main.js 加:
```js
import * as canvas from './core/canvas.js';
import * as input from './core/input.js';
// 在 refresh() 末尾加: canvas.render(store.state);
// main() 里加: canvas.init(); input.init();
```
改 refresh:
```js
export function refresh() {
  store.setState(getState());
  canvas.render(store.state);
}
```

- [ ] **Step 4: 验证空地图可平移缩放**

浏览器刷新,单指/鼠标拖拽地图应平移(canvas 暂空,看 console 无报错);滚轮/双指应缩放(camera.zoom 在 console 可查)。

- [ ] **Step 5: Commit**

```bash
git add web/js/core/canvas.js web/js/core/input.js web/js/main.js
git commit -m "feat(ui): canvas 管家(相机/坐标转换) + input(PointerEvent 统一手势)"
```

---

## Task 12: 地图图层 — layout + 6 图层(迁移现有 drawMap 逻辑)

**Files:**
- Create: `web/js/map/layout.js`、`web/js/map/layerTerrain.js`、`web/js/map/layerProvince.js`、`web/js/map/layerUnit.js`、`web/js/map/layerOrder.js`、`web/js/map/layerCombat.js`、`web/js/map/layerOverlay.js`

> 现有 `index.html.bak` 的 `provincePos` + `drawMap` 逻辑拆分迁移到这些图层。行军箭头车道算法完整保留。

- [ ] **Step 1: 写 map/layout.js(省份坐标)**

从 `index.html.bak` 提取 `provincePos` 函数到 `web/js/map/layout.js`,导出:
```js
export function provincePos(id, allIds, w, h) {
  // 两军对垒布局(沿用现有算法, 完整保留)
  const sorted = [...allIds].sort((a, b) => a - b);
  const half = Math.ceil(sorted.length / 2);
  const topIds = sorted.slice(0, half);
  const botIds = sorted.slice(half);
  const row = topIds.includes(id) ? 'top' : 'bottom';
  const rowIds = row === 'top' ? topIds : botIds;
  const colIdx = rowIds.indexOf(id);
  const colN = rowIds.length;
  const margin = 40;
  const usable = w - margin * 2;
  const x = colN <= 1 ? w / 2 : margin + (usable * colIdx / (colN - 1));
  const y = row === 'top' ? h * 0.27 : h * 0.73;
  return { x, y };
}
```

- [ ] **Step 2: 写 layerTerrain.js(地形底 + 邻接虚线)**

```js
import { provincePos } from './layout.js';
export function draw(ctx, view, { worldToScreen, W, H }) {
  if (!view.provinces?.length) return;
  const ids = view.provinces.map(p => p.id);
  // 邻接虚线
  ctx.strokeStyle = '#0f3460'; ctx.lineWidth = 1.5; ctx.setLineDash([4, 4]);
  view.provinces.forEach(p => {
    p.neighbors.forEach(n => {
      const a = worldToScreen(provincePos(p.id, ids, W, H));
      const nb = view.provinces.find(x => x.id === n);
      if (!nb) return;
      const b = worldToScreen(provincePos(n, ids, W, H));
      ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.lineTo(b.x, b.y); ctx.stroke();
    });
  });
  ctx.setLineDash([]);
}
```

- [ ] **Step 3: 写 layerProvince.js(政治着色 + 选中)**

从 `index.html.bak` 的省份圆圈逻辑迁移(含 TAG_COLORS、选中高亮)。

- [ ] **Step 4: 写 layerUnit.js(NATO 部队牌)**

参照原版 mapicons.gui 设计:省份位置画兵种符号 + 数量。简化版:
```js
import { provincePos } from './layout.js';
const SYMBOLS = { infantry: '▦', armor: '◆', artillery: '◎' };
export function draw(ctx, view, { worldToScreen, W, H }) {
  if (!view.divisions) return;
  const ids = view.provinces.map(p => p.id);
  // 按省聚合部队
  const byProv = {};
  view.divisions.forEach(d => { (byProv[d.loc] ||= []).push(d); });
  for (const [provId, divs] of Object.entries(byProv)) {
    const pos = worldToScreen(provincePos(+provId, ids, W, H));
    // 画 NATO 牌(简化: 兵种符号 + 数量)
    divs.slice(0, 3).forEach((d, i) => {
      const sym = d.template?.includes('Panzer') ? SYMBOLS.armor : SYMBOLS.infantry;
      ctx.fillStyle = d.owner === 'GER' ? '#e94560' : '#16c79a';
      ctx.font = 'bold 14px sans-serif';
      ctx.fillText(sym, pos.x - 20 + i * 14, pos.y + 30);
    });
    if (divs.length > 3) ctx.fillText('+' + (divs.length - 3), pos.x + 22, pos.y + 30);
  }
}
```

- [ ] **Step 5: 写 layerOrder.js(命令箭头,完整迁移车道算法)**

从 `index.html.bak:519-620` 完整迁移多段行军折线 + 车道偏移算法 + 支援箭头。这是最复杂的图层,逐行迁移,不改逻辑。迁移后改用 `worldToScreen` 转坐标。

- [ ] **Step 6: 写 layerCombat.js(战斗气泡)**

简化:交战省画进度环 + VS 标记(数据从 view.battles 取)。

- [ ] **Step 7: 写 layerOverlay.js(选中/tooltip)**

选中省/师的金色高亮(从 layerProvince 分离出交互态绘制)。

- [ ] **Step 8: main.js 注册 6 图层**

```js
import * as canvas from './core/canvas.js';
import * as L from './map/layerTerrain.js';  // ... 等
canvas.addLayer('terrain', 0, L.draw);
canvas.addLayer('province', 1, provinceDraw);
canvas.addLayer('unit', 2, unitDraw);
canvas.addLayer('order', 3, orderDraw);
canvas.addLayer('combat', 4, combatDraw);
canvas.addLayer('overlay', 5, overlayDraw);
canvas.markAllDirty();
```

- [ ] **Step 9: 验证地图渲染完整**

浏览器刷新,确认:10 省对垒图渲染(邻接线/省圆圈/部队牌/命令箭头)。平移缩放正常。tick 后部队移动箭头更新。

- [ ] **Step 10: Commit**

```bash
git add web/js/map/
git commit -m "feat(ui): 地图 6 图层(terrain/province/unit/order/combat/overlay)"
```

---

## Task 13: UI 组件 — topbar/drawer/orderMenu/panelHost/statbar

**Files:**
- Create: `web/js/ui/topbar.js`、`web/js/ui/drawer.js`、`web/js/ui/orderMenu.js`、`web/js/ui/panelHost.js`、`web/js/ui/statbar.js`

- [ ] **Step 1: 写 ui/statbar.js(状态条组件,复用)**

```js
import { h } from '../core/el.js';
// statbar(org, maxOrg, str, maxStr) → DOM 元素(4 条迷你状态条)
export function statbar(org, maxOrg, str, maxStr, eqRatio, mpRatio) {
  const bar = (cls, pct) => h('div', { class: 'mini-bar' }, [h('div', { class: cls, style: { width: pct + '%' } })]);
  return h('div', { class: 'unit-card' }, [
    bar('org', maxOrg > 0 ? org / maxOrg * 100 : 0),
    bar('str', maxStr > 0 ? str / maxStr * 100 : 0),
    bar('eq', (eqRatio || 0) * 100),
    bar('mp', (mpRatio || 0) * 100),
  ]);
}
```

- [ ] **Step 2: 写 ui/topbar.js(顶栏 + 系统按钮 + [切控制权] 测试按钮)**

```js
import { h, clear } from '../core/el.js';
import { bindText } from '../core/bind.js';
import { open as openPanel, names as panelNames } from '../core/router.js';
export function render() {
  const el = document.getElementById('topbar');
  clear(el);
  // 日期/速度
  const dateLabel = h('span', { class: 'topbar-date' });
  bindText('date', (d) => { dateLabel.textContent = `📅 ${d.y}.${d.m}.${d.d}`; });
  el.append(dateLabel);
  // 系统按钮(按注册的 panel)
  for (const name of panelNames()) {
    el.append(h('button', { class: 'secondary', onclick: () => openPanel(name) }, name));
  }
  // [切控制权] 测试按钮(上帝模式)
  el.append(h('button', { class: 'secondary', onclick: () => toggleControlMode() }, '切控制权'));
}
let controlMode = false;
export function isControlMode() { return controlMode; }
function toggleControlMode() { controlMode = !controlMode; console.log('control mode:', controlMode); }
```

- [ ] **Step 3: 写 ui/drawer.js(底部抽屉)**

```js
import { h, clear } from '../core/el.js';
export function open(content) {
  const el = document.getElementById('drawer');
  clear(el);
  el.append(content);
  el.classList.add('open');
}
export function close() { document.getElementById('drawer').classList.remove('open'); }
```

- [ ] **Step 4: 写 ui/orderMenu.js(下令菜单,底部弹出)**

```js
import { h, clear } from '../core/el.js';
import { moveDivision, queueMove, supportAttack } from '../engine/commands.js';
let pending = null;  // { divId, targetProv }
export function show(divId, targetProv) {
  pending = { divId, targetProv };
  const el = document.getElementById('order-menu');
  clear(el);
  el.append(
    h('button', { onclick: () => { moveDivision(divId, targetProv); hide(); } }, '⚔️ 进军攻击'),
    h('button', { onclick: () => { queueMove(divId, targetProv); hide(); } }, '➕ 追加航点'),
    h('button', { onclick: () => { supportAttack(divId, targetProv); hide(); } }, '🎯 支援攻击'),
    h('button', { class: 'secondary', onclick: hide }, '✖️ 取消'),
  );
  el.classList.add('open');
}
function hide() { document.getElementById('order-menu').classList.remove('open'); pending = null; }
```
(order-menu CSS:底部弹出,参考 drawer)

- [ ] **Step 5: 写 ui/panelHost.js(通用面板容器)**

panelHost 的 open/close 由 router 调用。各 panel 实现 `open()`(渲染内容到 #panel-host)/`close()`。

- [ ] **Step 6: main.js 接入 + 两段式下令交互**

在 main.js 接入 input 的命中回调,实现"选师→点省→弹命令菜单"两段式:
```js
import * as input from './core/input.js';
import * as orderMenu from './ui/orderMenu.js';
import { open as openDrawer } from './ui/drawer.js';
import { h } from './core/el.js';
import { provincePos } from './map/layout.js';
import { isControlMode } from './ui/topbar.js';
import { setProvinceController } from './engine/commands.js';

let selectedDiv = null;

// 找最近省份 id(世界坐标 wp, 半径 44 世界单位内)
function nearestProvince(wp, view) {
  if (!view.provinces?.length) return null;
  const ids = view.provinces.map(p => p.id);
  let best = null, bestD = 44;
  for (const p of view.provinces) {
    const pos = provincePos(p.id, ids, window.innerWidth, window.innerHeight);
    const d = Math.hypot(pos.x - wp.x, pos.y - wp.y);
    if (d < bestD) { bestD = d; best = p.id; }
  }
  return best;
}
// 找该省驻扎的第一个师(若玩家选师则优先该方)
function divAtProvince(provId, view) {
  return view.divisions?.find(d => d.loc === provId) || null;
}

input.onProvinceHit((wp, sx, sy) => {
  const view = store.state;
  const provId = nearestProvince(wp, view);
  if (provId == null) return false;
  // 切控制权模式(测试上帝模式): 点省切 GER/FRA
  if (isControlMode()) {
    const p = view.provinces.find(x => x.id === provId);
    setProvinceController(provId, p.controller === 'GER' ? 'FRA' : 'GER');
    refresh();
    return true;
  }
  // 两段式下令: 已选师 → 点省弹命令菜单
  if (selectedDiv) {
    orderMenu.show(selectedDiv, provId);
    selectedDiv = null;
    return true;
  }
  // 点师选中 / 点空省弹抽屉显示部队
  const div = divAtProvince(provId, view);
  if (div) {
    selectedDiv = div.id;
    openDrawer(drawProvDetail(provId, view));  // 抽屉显示该省部队
  } else {
    openDrawer(drawProvDetail(provId, view));  // 空省也弹抽屉(显示"无部队")
  }
  return true;
});
input.onBackgroundClick(() => { selectedDiv = null; openDrawer(close()); });

// 抽屉内容: 该省部队列表
function drawProvDetail(provId, view) {
  const divs = view.divisions?.filter(d => d.loc === provId) || [];
  const p = view.provinces.find(x => x.id === provId);
  return h('div', {}, [
    h('h3', { text: `📍 省${provId} [${p?.controller || '?'}]` }),
    ...divs.map(d => h('div', { class: 'div-card ' + (d.owner === 'GER' ? 'attacker' : 'defender') }, [
      h('div', { text: `${d.owner} 师#${d.id} ${d.template || ''}` }),
      statbar(d.org, d.max_org, d.str, d.max_str, d.eq_ratio, d.mp_ratio),
    ])),
    divs.length === 0 ? h('div', { text: '该省无部队' }) : null,
  ]);
}
```
> 需在 commands.js 加 `setProvinceController(id, tag)` 封装(调 `engine_set_province_controller`)。

- [ ] **Step 7: 验证交互(手动)**

浏览器:点师选中 → 点省弹命令菜单 → 进军。tick 后部队移动。底部抽屉点省弹部队列表。

- [ ] **Step 8: Commit**

```bash
git add web/js/ui/ web/js/main.js
git commit -m "feat(ui): 复用组件(topbar/drawer/orderMenu/panelHost/statbar) + 两段式下令"
```

---

## Task 14: 面板内容 — deploy/diplomacy/unit/combat + 完整 setup

**Files:**
- Create: `web/js/views/deployPanel.js`、`web/js/views/diplomacyPanel.js`、`web/js/views/unitPanel.js`、`web/js/views/combatPanel.js`
- Modify: `web/js/main.js`(完整 setup 脚本 + 注册面板)

- [ ] **Step 1: 写 deployPanel.js(选模板→选省→建师 + 换模板)**

```js
import { h, clear } from '../core/el.js';
import { getTemplates } from '../engine/state.js';
import { deployTemplate, changeTemplate } from '../engine/commands.js';
import { open } from '../ui/drawer.js';
import { register } from '../core/router.js';

let selectedTemplate = null;
export function init() {
  register('部署', {
    open() {
      const host = document.getElementById('panel-host');
      clear(host);
      const templates = getTemplates();
      const sel = h('select', {});
      templates.forEach(t => sel.append(h('option', { value: t }, t)));
      host.append(
        h('h3', { text: '部署师' }),
        h('label', { text: '模板' }), sel,
        h('button', { onclick: () => {
          const tmpl = sel.value;
          // 进入"选省部署"模式(点地图选省)
          openDeployMode(tmpl);
        } }, '选省部署'),
      );
    },
    close() {},
  });
}
function openDeployMode(tmpl) {
  // 设标志, input 命中时调 deployTemplate
  deployMode = tmpl;  // 模块级变量 let deployMode = null;
  // 提示用户点地图选省(抽屉显示提示)
  import('../ui/drawer.js').then(({ open }) =>
    open(h('div', { text: `已选模板「${tmpl}」, 点地图省份部署(ESC 取消)` })));
}
// 在 main.js 的 input.onProvinceHit 回调最前面加:
//   if (deployMode) {
//     const provId = nearestProvince(wp, view);
//     if (provId != null) { deployTemplate(player, provId, deployMode); deployMode = null; refresh(); }
//     return true;
//   }
// ESC 取消: document.addEventListener('keydown', e => { if (e.key === 'Escape') deployMode = null; });
```

- [ ] **Step 2: 写 diplomacyPanel.js(宣战/阵营/和谈)**

```js
import { declareWar, createFaction, joinFaction, whitePeace } from '../engine/commands.js';
export function init() {
  register('外交', {
    open() {
      // 渲染: 当前 wars 列表 + "GER 宣战 FRA" 按钮 + 阵营操作
    },
    close() {},
  });
}
```

- [ ] **Step 3: 写 unitPanel.js(全部队列表,bindList)**

用 `bindList` 绑定 divisions 数组,每项渲染师卡片(statbar + 模板名 + 换模板按钮)。

- [ ] **Step 4: 写 combatPanel.js(交战视窗)**

迁移 `index.html.bak` 的 `battleInfo` 渲染逻辑(攻守双方 + 预备队)。

- [ ] **Step 5: 写完整 setup 脚本(main.js)**

替换 Task 9 的占位 setup:
```js
const setup = `
create_state = { id = 1 owner = GER name = "GER Front" }
create_state = { id = 2 owner = FRA name = "FRA Front" }
create_province = { id = 1 state = 1 neighbors = { 2 6 7 } }
create_province = { id = 2 state = 1 neighbors = { 1 3 6 7 8 } }
create_province = { id = 3 state = 1 neighbors = { 2 4 7 8 9 } }
create_province = { id = 4 state = 1 neighbors = { 3 5 8 9 10 } }
create_province = { id = 5 state = 1 neighbors = { 4 9 10 } }
create_province = { id = 6 state = 2 neighbors = { 1 2 7 } }
create_province = { id = 7 state = 2 neighbors = { 1 2 3 6 8 } }
create_province = { id = 8 state = 2 neighbors = { 2 3 4 7 9 } }
create_province = { id = 9 state = 2 neighbors = { 3 4 5 8 10 } }
create_province = { id = 10 state = 2 neighbors = { 4 5 9 } }
declare_war = { attacker = GER defender = FRA }
`;
```

- [ ] **Step 6: main.js 注册所有面板**

```js
import { init as initDeploy } from './views/deployPanel.js';
import { init as initDiplo } from './views/diplomacyPanel.js';
import { init as initUnit } from './views/unitPanel.js';
import { init as initCombat } from './views/combatPanel.js';
initDeploy(); initDiplo(); initUnit(); initCombat();
import * as topbar from './ui/topbar.js';
topbar.render();
```

- [ ] **Step 7: 端到端验证(手动完整对战)**

浏览器:
1. 部署 GER Infanterie 师(选模板→选省)→ 部署 Panzer 师
2. 外交面板宣战(declare_war,setup 已含也可手动)
3. tick 流逝 → 观察战斗(交战视窗 + 战斗气泡)
4. 部队面板换模板 → 观察数值变化(NATO 牌/面板联动刷新)
5. 触屏(手机浏览器):拖地图/捏缩放/两段式下令/抽屉

- [ ] **Step 8: Commit**

```bash
git add web/js/views/ web/js/main.js
git commit -m "feat(ui): 面板内容(deploy/diplomacy/unit/combat) + 完整 setup + 端到端验证"
```

---

## Task 15: 删旧文件 + 清理 + 最终验证

**Files:**
- Delete: `web/index.html.bak`(确认新 UI 完全替代后)

- [ ] **Step 1: 全量 Rust 测试**

Run: `cargo test --lib`
Expected: 全部 PASS(180+ 测试 + 本次新增)

- [ ] **Step 2: 重新编译 WASM 确认最终版**

Run: `cargo build --target wasm32-unknown-unknown --lib --release && cp target/wasm32-unknown-unknown/release/hoi4_clone.wasm web/`

- [ ] **Step 3: 浏览器全流程验证**

完整对战 + 触屏 + 换模板(参照 Task 14 Step 7 清单)。确认:
- 数据流联动(换模板后 NATO 牌/面板/交战视窗都刷新)
- 脏标记生效(师移动时顶栏不重算)
- 移动端手势(拖/捏/两段式)
- 44px 目标可点

- [ ] **Step 4: 删旧备份**

```bash
rm web/index.html.bak
```

- [ ] **Step 5: 更新 HANDOFF.md**

在 `docs/HANDOFF.md` 加 demo 改造完成的里程碑记录。

- [ ] **Step 6: 最终 Commit**

```bash
git add -A
git commit -m "feat: demo 彻底改造完成(全屏地图+绑定式数据流+触屏+template引用)"
```

---

## Task 16: demo 改造后修复 + Playwright 自动化验证(2026-06-25)

> Task 1-15 实现完成后, 实际运行发现 demo 一启动即崩。系统性排查定位 9 个问题(主因: canvas.js 的 `fullRedraw` 未声明导致 `init()` 在严格模式抛 `ReferenceError`, 中断整个 main —— 这正是此前 6 个 `fix(ui)` commit 没找到的真根因)。本 Task 全部修复, 并引入 Playwright 真机验证, 13/13 通过。详细根因/修复/spec 对齐见 `docs/HANDOFF.md`「demo 改造后修复」小节。

**Files:**
- Modify: `web/js/core/canvas.js`(声明 fullRedraw + 恢复脏标记语义)
- Modify: `web/js/core/store.js`(重写: diffKeys + subscribeKeys 路径级脏标记)
- Modify: `web/js/core/bind.js`(bindList/bindText 改路径订阅)
- Create: `web/js/ui/bottombar.js`(时间控制移入底栏)
- Modify: `web/js/ui/topbar.js`(移除时间按钮, 改路径订阅 date)
- Modify: `web/js/map/layerProvince.js`(选中高亮分离出去)
- Rewrite: `web/js/map/layerOverlay.js`(承接选中高亮)
- Modify: `web/index.html`(加 #log + favicon data-uri)
- Modify: `web/css/app.css`(#log 浮层样式)
- Modify: `web/js/main.js`(接入 bottombar)
- Modify: `src/wasm_api.rs`(engine_supply 补 light_tank_chassis)
- Modify: `src/parser/block.rs`(删 unreachable dead code)
- Create: `tests/web_demo.mjs`(Playwright 验证脚本)
- Modify: `.gitignore`(忽略 node_modules)

- [x] **Step 1: 修复 #1 canvas fullRedraw 未声明**(致命, demo 启动崩溃根因)
- [x] **Step 2: 修复 #4/#5 store 路径级脏标记 + bind 路径订阅**(对齐 spec §3.3)
- [x] **Step 3: 修复 #3 新建 bottombar, 时间控制移入**(对齐 spec §7.1)
- [x] **Step 4: 修复 #6 overlay 承接选中高亮**(对齐 spec §6.1)
- [x] **Step 5: 修复 #7 补 #log 元素 + 样式**
- [x] **Step 6: 修复 #8 engine_supply 补 light_tank_chassis**(对齐 spec §8.2)
- [x] **Step 7: 修复 #9 删 parser unreachable dead code**(wasm 0 警告)
- [x] **Step 8: 编译 wasm + cargo test --lib 122 全绿 + wasm 0 警告**
- [x] **Step 9: 装 playwright-chromium, 写 tests/web_demo.mjs**
- [x] **Step 10: 真机验证 13/13 通过(系统 Chrome channel:'chrome')**
- [x] **Step 11: 更新 HANDOFF(修复小节)+ 本 PLAN(Task 16)**
