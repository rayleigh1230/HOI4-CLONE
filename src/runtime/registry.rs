//! Registry: 命令注册表
use crate::ast::Arg;
use crate::runtime::World;
use std::collections::HashMap;

pub type EffectFn = fn(&mut World, &[Arg]);

#[derive(Default)]
pub struct Registry {
    pub effects: HashMap<String, EffectFn>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, name: &str, f: EffectFn) {
        self.effects.insert(name.to_string(), f);
    }
    pub fn get(&self, name: &str) -> Option<&EffectFn> {
        self.effects.get(name)
    }
}
