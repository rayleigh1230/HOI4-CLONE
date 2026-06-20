//! Trigger: 返回 bool 的条件。对应原版 trigger/limit 块。
use crate::ast::Arg;

#[derive(Debug, Clone)]
pub enum Trigger {
    /// 基础判定: has_dlc("X"), is_major() 等
    Check { name: String, args: Vec<Arg> },
    And(Vec<Trigger>),
    Or(Vec<Trigger>),
    Not(Box<Trigger>),
    /// 原版: tag = GER 这种比较
    Compare { lhs: String, op: CompareOp, rhs: Arg },
    Always(bool),
}

#[derive(Debug, Clone, Copy)]
pub enum CompareOp {
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
}
