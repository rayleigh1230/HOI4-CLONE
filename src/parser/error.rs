//! 解析错误类型(标准库实现,避免外部依赖)
use std::fmt;

#[derive(Debug)]
pub enum ParseError {
    Syntax { line: usize, msg: String },
    UnexpectedEof,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Syntax { line, msg } => write!(f, "第 {line} 行: {msg}"),
            ParseError::UnexpectedEof => write!(f, "意外的文件结束"),
        }
    }
}

impl std::error::Error for ParseError {}
