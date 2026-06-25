//! 运行时模块
pub mod clock;
pub mod date;
pub mod entities;
pub mod error;
pub mod interp;
pub mod registry;
pub mod world;

pub use clock::GameClock;
pub use crate::data::GameData;
pub use date::GameDate;
pub use entities::{Battle, Country, Division, Province, Scope, State, War};
pub use error::CmdError;
pub use interp::Interpreter;
pub use registry::{EffectFn, ParamGet, Registry, TriggerFn};
pub use world::World;
