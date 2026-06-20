//! 命令注册入口
use crate::runtime::Registry;

pub mod control;
pub mod scope;
pub mod vars;

pub fn register_all(reg: &mut Registry) {
    vars::register(reg);
    control::register(reg);
    scope::register(reg);
    crate::combat::commands::register(reg);
}
