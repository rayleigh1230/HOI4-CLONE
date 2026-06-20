//! 命令执行错误类型(M2 P0-3)
use std::fmt;

#[derive(Debug)]
pub enum CmdError {
    UnknownCommand(String),
    BadParam { cmd: String, key: String, reason: String },
    RuntimeError(String),
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CmdError::UnknownCommand(c) => write!(f, "未注册的命令: {c}"),
            CmdError::BadParam { cmd, key, reason } => {
                write!(f, "命令 {cmd} 参数 {key} 错误: {reason}")
            }
            CmdError::RuntimeError(m) => write!(f, "运行时错误: {m}"),
        }
    }
}

impl std::error::Error for CmdError {}
