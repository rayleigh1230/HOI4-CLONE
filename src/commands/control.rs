//! 控制流命令(M1 预留,大部分控制流由 Interpreter 直接处理)
use crate::runtime::Registry;

pub fn register(_reg: &mut Registry) {
    // 预留: custom_effect_tooltip, hidden_effect 等无副作用命令在 M2 扩展
}
