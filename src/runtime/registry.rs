//! Registry: effect 和 trigger 命令注册表(M2: 加 triggers + Result 签名)
use crate::ast::Arg;
use crate::runtime::error::CmdError;
use crate::runtime::World;
use std::collections::HashMap;

pub type EffectFn = fn(&mut World, &[(String, Arg)]) -> Result<(), CmdError>;
pub type TriggerFn = fn(&World, &[(String, Arg)]) -> Result<bool, CmdError>;

/// 命令参数辅助取值
pub trait ParamGet {
    fn pos(&self, i: usize) -> Option<&Arg>;
    fn get(&self, key: &str) -> Option<&Arg>;
}
impl ParamGet for [(String, Arg)] {
    fn pos(&self, i: usize) -> Option<&Arg> {
        self.get(i).map(|(_, v)| v)
    }
    fn get(&self, key: &str) -> Option<&Arg> {
        self.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

#[derive(Default)]
pub struct Registry {
    pub effects: HashMap<String, EffectFn>,
    pub triggers: HashMap<String, TriggerFn>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, name: &str, f: EffectFn) {
        self.effects.insert(name.to_string(), f);
    }
    pub fn register_trigger(&mut self, name: &str, f: TriggerFn) {
        self.triggers.insert(name.to_string(), f);
    }
    pub fn get_effect(&self, name: &str) -> Option<&EffectFn> {
        self.effects.get(name)
    }
    pub fn get_trigger(&self, name: &str) -> Option<&TriggerFn> {
        self.triggers.get(name)
    }
}
