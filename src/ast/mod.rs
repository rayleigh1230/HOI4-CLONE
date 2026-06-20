//! AST 模块 (Task 4-5 实现)
pub mod effect;
pub mod trigger;
pub mod lower;

pub use effect::{Arg, Effect, RandomPick};
pub use trigger::{CompareOp, Trigger};
