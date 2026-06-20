//! Effect: 改变世界状态的命令。对应原版 effect 块。
use crate::ast::trigger::Trigger;

#[derive(Debug, Clone)]
pub enum Effect {
    /// 基础命令。params 为命名字段;位置参数用空 key ("", Arg)
    Command { name: String, params: Vec<(String, Arg)> },
    /// if = { limit = { ... } <then> else = { ... } }
    If { cond: Trigger, then: Vec<Effect>, els: Vec<Effect> },
    /// 作用域遍历: every_owned_state = { limit = {...} <body> }
    ForEach { scope: String, filter: Option<Trigger>, body: Vec<Effect> },
    /// random_events = { 100 = xxx 100 = yyy }
    Random { table: Vec<(f64, RandomPick)> },
}

#[derive(Debug, Clone)]
pub enum Arg {
    Num(f64),
    Str(String),
    Bool(bool),
    /// 嵌套块参数: add_equipment_production = { equipment=... count=10 }
    Block(Vec<(String, Arg)>),
}

impl Arg {
    pub fn as_num(&self) -> Option<f64> {
        if let Arg::Num(n) = self { Some(*n) } else { None }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Arg::Str(s) = self { Some(s) } else { None }
    }
}

#[derive(Debug, Clone)]
pub enum RandomPick {
    EventId(String),
    Nested(Vec<Effect>),
}
