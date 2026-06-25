//! World: 游戏状态(M3: 加实体存储 + 作用域栈)
use crate::ast::Effect;
use crate::data::GameData;
use crate::runtime::date::GameDate;
use crate::runtime::entities::{Battle, Country, Division, Province, Scope, State, War};
use crate::runtime::error::CmdError;
use std::collections::HashMap;

#[derive(Debug)]
pub struct World {
    // M1/M2 字段
    pub vars: HashMap<String, f64>,
    pub flags: HashMap<String, bool>,
    pub strings: HashMap<String, String>,
    pub hour: u64,
    pub player_tag: String,
    pub error_log: Vec<CmdError>,
    pub event_bus: HashMap<String, Vec<Effect>>,
    // M3 实体存储
    pub provinces: HashMap<u32, Province>,
    pub states: HashMap<u32, State>,
    pub countries: HashMap<String, Country>,
    pub divisions: HashMap<u64, Division>,
    pub battles: Vec<Battle>,
    /// 战略级战争状态(外交级; 与 battles 战术级不同)
    pub wars: Vec<War>,
    pub next_war_id: u64,
    pub scope_stack: Vec<Scope>,
    pub next_division_id: u64,
    pub next_battle_id: u64,
    /// 游戏是否已开始(首次 tick 后置 true)。
    /// started=false 时(部署阶段), 同方向进攻师都进前线; started=true 后同 origin 后到的进预备队。
    pub started: bool,
    /// 只读静态定义数据库(数据驱动层)
    pub data: std::sync::Arc<GameData>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            vars: Default::default(),
            flags: Default::default(),
            strings: Default::default(),
            hour: 0,
            player_tag: String::new(),
            error_log: Vec::new(),
            event_bus: Default::default(),
            provinces: Default::default(),
            states: Default::default(),
            countries: Default::default(),
            divisions: Default::default(),
            battles: Vec::new(),
            wars: Vec::new(),
            next_war_id: 1,
            scope_stack: vec![Scope::Root],
            next_division_id: 1,
            next_battle_id: 1,
            started: false,
            data: crate::data::cached_game_data(),
        }
    }
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }
    // M1/M2 方法
    pub fn set_var(&mut self, k: &str, v: f64) {
        self.vars.insert(k.to_string(), v);
    }
    pub fn get_var(&self, k: &str) -> f64 {
        *self.vars.get(k).unwrap_or(&0.0)
    }
    pub fn add_var(&mut self, k: &str, v: f64) {
        let cur = self.get_var(k);
        self.vars.insert(k.to_string(), cur + v);
    }
    pub fn set_flag(&mut self, k: &str) {
        self.flags.insert(k.to_string(), true);
    }
    pub fn has_flag(&self, k: &str) -> bool {
        *self.flags.get(k).unwrap_or(&false)
    }
    pub fn set_string(&mut self, k: &str, v: &str) {
        self.strings.insert(k.to_string(), v.to_string());
    }
    pub fn get_string(&self, k: &str) -> &str {
        self.strings.get(k).map(|s| s.as_str()).unwrap_or("")
    }

    // M2 事件钩子
    pub fn on(&mut self, event: &str, effs: Vec<Effect>) {
        self.event_bus.entry(event.to_string()).or_default().extend(effs);
    }
    pub fn fire_event(&mut self, interp: &crate::runtime::Interpreter, event: &str) {
        if let Some(effs) = self.event_bus.get(event) {
            let effs = effs.clone();
            interp.run(&effs, self);
        }
    }

    // M3 作用域辅助
    pub fn current_scope(&self) -> Scope {
        self.scope_stack.last().cloned().unwrap_or(Scope::Root)
    }
    /// 从栈顶往下找最近的国家作用域; 无则回退到 player_tag(顶层默认玩家国家)
    pub fn current_country(&self) -> Option<&str> {
        if let Some(t) = self.scope_stack.iter().rev().find_map(|s| s.country_tag()) {
            Some(t)
        } else if !self.player_tag.is_empty() {
            Some(&self.player_tag)
        } else {
            None
        }
    }

    // M3 实体管理
    pub fn add_division(&mut self, mut d: Division) -> u64 {
        d.id = self.next_division_id;
        self.next_division_id += 1;
        let id = d.id;
        self.divisions.insert(id, d);
        id
    }
    pub fn divisions_of(&self, tag: &str) -> Vec<u64> {
        self.divisions
            .values()
            .filter(|d| d.owner_tag == tag)
            .map(|d| d.id)
            .collect()
    }

    // ===== 战争状态(War 关系判定) =====

    /// 判定两个 tag 是否处于战争状态(分属某场 war 的对立两侧)
    pub fn are_at_war(&self, a: &str, b: &str) -> bool {
        self.wars.iter().any(|w| {
            (w.attackers.contains(a) && w.defenders.contains(b))
                || (w.defenders.contains(a) && w.attackers.contains(b))
        })
    }

    /// 取某 tag 的所有交战国(在任一 war 的对立侧)
    pub fn enemies_of(&self, tag: &str) -> Vec<String> {
        use std::collections::HashSet;
        let mut enemies = HashSet::new();
        for w in &self.wars {
            if w.attackers.contains(tag) {
                enemies.extend(w.defenders.iter().cloned());
            } else if w.defenders.contains(tag) {
                enemies.extend(w.attackers.iter().cloned());
            }
        }
        enemies.into_iter().collect()
    }

    /// 宣战: 建立一场新战争, 双方阵营成员自动加入
    pub fn declare_war(&mut self, attacker: &str, defender: &str) -> u64 {
        use std::collections::HashSet;
        let id = self.next_war_id;
        self.next_war_id += 1;
        let mut atk: HashSet<String> = HashSet::new();
        atk.insert(attacker.into());
        atk.extend(self.faction_members(attacker));
        let mut def: HashSet<String> = HashSet::new();
        def.insert(defender.into());
        def.extend(self.faction_members(defender));
        self.wars.push(War { id, attackers: atk, defenders: def });
        id
    }

    /// 取某 tag 的同阵营成员(不含自己)
    fn faction_members(&self, tag: &str) -> Vec<String> {
        let faction = self.countries.get(tag).and_then(|c| c.faction.as_ref());
        match faction {
            None => vec![],
            Some(f) => self.countries.iter()
                .filter(|(t, c)| t.as_str() != tag && c.faction.as_deref() == Some(f.as_str()))
                .map(|(t, _)| t.clone())
                .collect(),
        }
    }

    // ===== 日期派生(从 hour 算, 不存状态) =====

    /// 当前游戏日期(从 hour 派生)
    pub fn date(&self) -> GameDate {
        GameDate::from_hours(self.hour)
    }

    /// 从开局起经过的总天数(用于"N 天后"判定)
    pub fn total_days(&self) -> u64 {
        self.hour / 24
    }

    // ===== State 派生查询(Province 归属从 State 派生) =====

    /// 省份 → 所属 State id
    pub fn province_state(&self, province_id: u32) -> Option<u32> {
        self.provinces.get(&province_id).map(|p| p.state_id)
    }

    /// 省份的实际控制者。优先读省份级 controller(占领覆盖); None 则从所属 State 派生。
    /// 对齐 HOI4 省份级占领: 占领一省只改该省, 不蔓延到同 State 其他省。
    pub fn province_controller(&self, province_id: u32) -> Option<&str> {
        let prov = self.provinces.get(&province_id)?;
        if let Some(c) = prov.controller.as_deref() {
            return Some(c);
        }
        let sid = prov.state_id;
        self.states.get(&sid).map(|s| s.controller.as_str())
    }

    /// 省份的法理归属者(从 State 派生; 占领不改 owner)
    pub fn province_owner(&self, province_id: u32) -> Option<&str> {
        let sid = self.province_state(province_id)?;
        self.states.get(&sid).map(|s| s.owner.as_str())
    }

    /// 设置省份级实际控制者(只改该省, 不碰 State, 不蔓延)。
    /// 占领用: 对齐 HOI4 省份级占领。
    pub fn set_province_controller(&mut self, province_id: u32, new_controller: &str) {
        if let Some(prov) = self.provinces.get_mut(&province_id) {
            prov.controller = Some(new_controller.into());
        }
    }

    /// [兼容旧名] 设置省份控制者 — 现改为省份级(原 set_state_controller 是 State 级, 会蔓延)。
    /// 保留旧名供旧调用点, 但内部委托 set_province_controller 实现省份级占领。
    pub fn set_state_controller(&mut self, province_id: u32, new_controller: &str) {
        self.set_province_controller(province_id, new_controller);
    }

    // 行军辅助(陆战循环)
    /// 找某省的邻接己方省(撤退目标)。无则返回 None(被包围)
    pub fn friendly_neighbor(&self, province: u32, tag: &str) -> Option<u32> {
        let prov = self.provinces.get(&province)?;
        prov.neighbors.iter().copied().find(|n| {
            self.province_controller(*n).map(|c| c == tag).unwrap_or(false)
        })
    }

    // 战斗宽度(陆战循环)
    /// 一组师占用的战斗宽度总和
    pub fn used_width(&self, div_ids: &[u64]) -> f64 {
        div_ids.iter()
            .filter_map(|id| self.divisions.get(id))
            .map(|d| d.combat_width)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_set_get_var() {
        let mut w = World::new();
        w.set_var("stability", 0.5);
        assert!((w.get_var("stability") - 0.5).abs() < 1e-9);
    }
    #[test]
    fn t_add_var() {
        let mut w = World::new();
        w.set_var("pp", 100.0);
        w.add_var("pp", 50.0);
        assert!((w.get_var("pp") - 150.0).abs() < 1e-9);
    }
    #[test]
    fn t_flag() {
        let mut w = World::new();
        assert!(!w.has_flag("done"));
        w.set_flag("done");
        assert!(w.has_flag("done"));
    }
    #[test]
    fn t_m2_fields_default() {
        let w = World::new();
        assert!(w.error_log.is_empty());
        assert_eq!(w.hour, 0);
        assert!(w.player_tag.is_empty());
    }
    #[test]
    fn t_province_level_occupation_no_spread() {
        // 占领一省不应蔓延到同 State 的其他省(对齐 HOI4 省份级占领)。
        let mut w = World::new();
        // state 1 (owner/controller = GER), 含省1 省2
        w.states.insert(1, crate::runtime::entities::State {
            id: 1, owner: "GER".into(), controller: "GER".into(),
            ..Default::default()
        });
        w.provinces.insert(1, crate::runtime::entities::Province {
            id: 1, state_id: 1, ..Default::default()
        });
        w.provinces.insert(2, crate::runtime::entities::Province {
            id: 2, state_id: 1, ..Default::default()
        });
        // 占领省2 → FRA
        w.set_state_controller(2, "FRA");
        // 省2 应是 FRA
        assert_eq!(w.province_controller(2), Some("FRA"));
        // 省1 必须仍是 GER(不蔓延!)
        assert_eq!(w.province_controller(1), Some("GER"), "占领省2 不应蔓延到省1");
    }
    #[test]
    fn t_province_controller_fallback_to_state() {
        // Province.controller = None 时, 从所属 State 派生(向后兼容)。
        let mut w = World::new();
        w.states.insert(1, crate::runtime::entities::State {
            id: 1, owner: "GER".into(), controller: "GER".into(),
            ..Default::default()
        });
        w.provinces.insert(1, crate::runtime::entities::Province {
            id: 1, state_id: 1, ..Default::default()
        });
        assert_eq!(w.province_controller(1), Some("GER")); // 派生
        w.set_province_controller(1, "FRA");
        assert_eq!(w.province_controller(1), Some("FRA")); // 省份级覆盖
    }
    #[test]
    fn t_m3_scope_stack_starts_root() {
        let w = World::new();
        assert!(matches!(w.current_scope(), Scope::Root));
    }
    #[test]
    fn t_add_division_assigns_id() {
        let mut w = World::new();
        let d = Division {
            id: 0, owner_tag: "GER".into(), location_province: 1,
            soft_attack: 10.0, hard_attack: 1.0, defense: 20.0, breakthrough: 5.0,
            armor: 0.0, piercing: 5.0, hardness: 0.0, combat_width: 10.0,
            max_org: 60.0, org: 60.0, max_strength: 20.0, strength: 20.0,
            ..Default::default()
        };
        let id = w.add_division(d);
        assert_eq!(id, 1);
        assert_eq!(w.next_division_id, 2);
        assert_eq!(w.divisions_of("GER").len(), 1);
    }

    #[test]
    fn t_world_carries_game_data() {
        let w = World::new();
        assert!(!w.data.equipment.is_empty(), "World 应持有非空 GameData");
        assert!(!w.data.sub_units.is_empty(), "应含营定义");
    }

    /// 辅助: 用 Registry + Interpreter 跑脚本, 返回 World(命令测试用)
    fn run_script_world(scripts: &[&str]) -> World {
        use crate::ast::lower::lower_effects;
        use crate::runtime::{Interpreter, Registry};
        let mut reg = Registry::new();
        crate::commands::register_all(&mut reg);
        crate::combat::commands::register(&mut reg);
        let mut interp = Interpreter::new(reg);
        let mut w = World::new();
        for s in scripts {
            let b = crate::parser::parse(s).unwrap();
            let effs = lower_effects(&b);
            interp.run(&effs, &mut w);
        }
        w
    }

    #[test]
    fn t_create_division_records_template_name() {
        // create_division template= 应把模板名记进 Division.template_name
        let w = run_script_world(&[
            "create_state = { id = 1 owner = GER }",
            "create_province = { id = 1 state = 1 }",
            "create_division = { owner = GER location = 1 template = \"Infanterie-Division\" }",
        ]);
        let div = w.divisions.values().next().expect("应建出师");
        assert_eq!(div.template_name.as_deref(), Some("Infanterie-Division"),
            "create_division template= 应记录 template_name");
    }

    #[test]
    fn t_change_template_updates_stats_keeps_runtime() {
        // 换模板: 数值更新 + template_name 更新 + 运行态(org/strength)保留
        let mut w = run_script_world(&[
            "create_state = { id = 1 owner = GER }",
            "create_province = { id = 1 state = 1 }",
            "create_division = { owner = GER location = 1 template = \"Infanterie-Division\" }",
        ]);
        let div_id = w.divisions.values().next().unwrap().id;
        let inf_armor_before = w.divisions.get(&div_id).unwrap().armor;
        // 模拟战斗后运行态
        {
            let d = w.divisions.get_mut(&div_id).unwrap();
            d.org = 30.0;
            d.strength = 100.0;
        }
        // 换装甲模板
        run_script_world_on(&mut w, "change_template = { division = 1 template = \"Panzer-Division\" }");
        let d = w.divisions.get(&div_id).unwrap();
        assert!(d.armor > inf_armor_before, "换装甲模板后 armor 应升高(前{inf_armor_before} 后{})", d.armor);
        assert_eq!(d.template_name.as_deref(), Some("Panzer-Division"), "template_name 应更新");
        assert!((d.org - 30.0).abs() < 1e-9, "换模板应保留 org, 实际 {}", d.org);
        assert!((d.strength - 100.0).abs() < 1e-9, "换模板应保留 strength, 实际 {}", d.strength);
    }

    /// 辅助: 在已有 World 上跑单个脚本(原地修改)
    fn run_script_world_on(w: &mut World, script: &str) {
        use crate::ast::lower::lower_effects;
        use crate::runtime::{Interpreter, Registry};
        let mut reg = Registry::new();
        crate::commands::register_all(&mut reg);
        crate::combat::commands::register(&mut reg);
        let mut interp = Interpreter::new(reg);
        let b = crate::parser::parse(script).unwrap();
        let effs = lower_effects(&b);
        interp.run(&effs, w);
    }
}
