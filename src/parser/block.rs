//! Block 解析器: token 流 → 嵌套块树
use crate::parser::error::ParseError;
use crate::parser::lexer::{lex, Token};

#[derive(Debug, Clone, Default)]
pub struct Block {
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub enum Value {
    Scalar(String),
    Block(Block),
}

struct Cursor {
    toks: Vec<Token>,
    pos: usize,
}

pub fn parse(src: &str) -> Result<Block, ParseError> {
    let toks = lex(src)?;
    let mut cur = Cursor { toks, pos: 0 };
    parse_block(&mut cur, false)
}

fn parse_block(cur: &mut Cursor, expect_rbrace: bool) -> Result<Block, ParseError> {
    let mut fields = Vec::new();
    while cur.pos < cur.toks.len() {
        match &cur.toks[cur.pos] {
            Token::RBrace => {
                if expect_rbrace {
                    cur.pos += 1;
                    return Ok(Block { fields });
                } else {
                    return Err(ParseError::Syntax { line: 0, msg: "意外的 }".into() });
                }
            }
            Token::Ident(key) => {
                let key = key.clone();
                cur.pos += 1;
                // HOI4 trigger 里常见裸比较: political_power >= 150
                // 此时 ident 后跟比较运算符而非 =
                if let Some(op) = match cur.toks.get(cur.pos) {
                    Some(Token::Ge) => Some(">="),
                    Some(Token::Le) => Some("<="),
                    Some(Token::Gt) => Some(">"),
                    Some(Token::Lt) => Some("<"),
                    Some(Token::Ne) => Some("<>"),
                    _ => None,
                } {
                    cur.pos += 1; // 消费比较符
                    // 期望右侧是 num/ident/str
                    let rhs = match cur.toks.get(cur.pos) {
                        Some(Token::Num(n)) => n.to_string(),
                        Some(Token::Ident(s)) => s.clone(),
                        Some(Token::Str(s)) => s.clone(),
                        _ => return Err(ParseError::Syntax {
                            line: 0,
                            msg: format!("比较运算符 {op} 后期望值"),
                        }),
                    };
                    cur.pos += 1;
                    // 用特殊前缀标记比较: key 不变, value 存 "op rhs"
                    fields.push(Field { key, value: Value::Scalar(format!("{op}{rhs}")) });
                    continue;
                }
                // 期望 =
                match cur.toks.get(cur.pos) {
                    Some(Token::Eq) => cur.pos += 1,
                    _ => {
                        return Err(ParseError::Syntax {
                            line: 0,
                            msg: format!("期望 = 在 {key} 之后"),
                        })
                    }
                }
                let value = match cur.toks.get(cur.pos) {
                    Some(Token::LBrace) => {
                        cur.pos += 1;
                        Value::Block(parse_block(cur, true)?)
                    }
                    Some(Token::Str(s)) => {
                        let v = s.clone();
                        cur.pos += 1;
                        Value::Scalar(v)
                    }
                    Some(Token::Num(n)) => {
                        let v = n.to_string();
                        cur.pos += 1;
                        Value::Scalar(v)
                    }
                    Some(Token::Bool(b)) => {
                        let v = b.to_string();
                        cur.pos += 1;
                        Value::Scalar(v)
                    }
                    Some(Token::Ident(s)) => {
                        let v = s.clone();
                        cur.pos += 1;
                        Value::Scalar(v)
                    }
                    _ => return Err(ParseError::UnexpectedEof),
                };
                fields.push(Field { key, value });
            }
            other => {
                return Err(ParseError::Syntax {
                    line: 0,
                    msg: format!("意外的 token: {other:?}"),
                })
            }
        }
    }
    if expect_rbrace {
        return Err(ParseError::UnexpectedEof);
    }
    Ok(Block { fields })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_scalar_block() {
        let b = parse("id = AFG_expand").unwrap();
        assert_eq!(b.fields.len(), 1);
        match &b.fields[0].value {
            Value::Scalar(s) => assert_eq!(s, "AFG_expand"),
            _ => panic!("应为标量"),
        }
    }

    #[test]
    fn t_nested_block() {
        let src = "focus = { id = GER_r x = 0 ai_will_do = { factor = 10 } }";
        let b = parse(src).unwrap();
        assert_eq!(b.fields.len(), 1);
        match &b.fields[0].value {
            Value::Block(inner) => {
                assert_eq!(inner.fields.len(), 3);
                assert!(inner.fields.iter().any(|f| f.key == "ai_will_do"));
            }
            _ => panic!("应为块"),
        }
    }

    #[test]
    fn t_focus_tree_from_real_file() {
        // 来自 afghanistan.txt 真实片段
        let src = r#"focus_tree = {
            id = afghanistan_tree
            country = { factor = 0 }
            focus = { id = AFG_telegraph x = -21 cost = 5 }
        }"#;
        let b = parse(src).unwrap();
        let tree = &b.fields[0]; // focus_tree
        assert_eq!(tree.key, "focus_tree");
    }
}
