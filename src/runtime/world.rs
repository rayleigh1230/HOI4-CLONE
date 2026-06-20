//! World: 游戏状态(M3: 加实体存储 + 作用域栈)
use crate::ast::Effect;
use crate::runtime::entities::{Battle, Country, Division, Province, Scope};
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
    pub countries: HashMap<String, Country>,
    pub divisions: HashMap<u64, Division>,
    pub battles: Vec<Battle>,
    pub scope_stack: Vec<Scope>,
    pub next_division_id: u64,
    pub next_battle_id: u64,
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
            countries: Default::default(),
            divisions: Default::default(),
            battles: Vec::new(),
            scope_stack: vec![Scope::Root],
            next_division_id: 1,
            next_battle_id: 1,
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

    // 行军辅助(陆战循环)
    /// 找某省的邻接己方省(撤退目标)。无则返回 None(被包围)
    pub fn friendly_neighbor(&self, province: u32, tag: &str) -> Option<u32> {
        let prov = self.provinces.get(&province)?;
        prov.neighbors.iter().copied().find(|n| {
            self.provinces.get(n).map(|p| p.controller == tag).unwrap_or(false)
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
}
