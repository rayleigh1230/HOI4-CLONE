//! World: 游戏状态(M2: 加 hour/player_tag/error_log/event_bus)
use crate::ast::Effect;
use crate::runtime::error::CmdError;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct World {
    pub vars: HashMap<String, f64>,
    pub flags: HashMap<String, bool>,
    pub strings: HashMap<String, String>,
    pub hour: u64,
    pub player_tag: String,
    pub error_log: Vec<CmdError>,
    pub event_bus: HashMap<String, Vec<Effect>>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }
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

    /// 注册事件钩子
    pub fn on(&mut self, event: &str, effs: Vec<Effect>) {
        self.event_bus.entry(event.to_string()).or_default().extend(effs);
    }
    /// 触发事件钩子
    pub fn fire_event(&mut self, interp: &crate::runtime::Interpreter, event: &str) {
        if let Some(effs) = self.event_bus.get(event) {
            let effs = effs.clone();
            interp.run(&effs, self);
        }
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
}
