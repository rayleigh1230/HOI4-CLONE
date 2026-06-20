//! Effect: 改变世界状态的命令。对应原版 effect 块。
use crate::ast::trigger::Trigger;

#[derive(Debug, Clone)]
pub enum Effect {
    /// 基础命令: name(args...)。如 add_stability(0.05), add_political_power(150)
    Command { name: String, args: Vec<Arg> },
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
}

#[derive(Debug, Clone)]
pub enum RandomPick {
    EventId(String),
    Nested(Vec<Effect>),
}
