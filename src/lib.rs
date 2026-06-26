//! hoi4-clone 核心引擎: HOI4 风格脚本运行时
pub mod ast;
pub mod combat;
pub mod commands;
pub mod data;
pub mod economy;
pub mod parser;
pub mod runtime;

// WASM 桥接层: 仅 wasm target 编译(避免桌面环境编译 FFI)
#[cfg(target_arch = "wasm32")]
pub mod wasm_api;
