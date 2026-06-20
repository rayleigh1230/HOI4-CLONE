//! 词法分析器: HOI4 脚本 → token 流
use crate::parser::error::ParseError;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    Str(String),
    Num(f64),
    Bool(bool),
    Eq,
    LBrace,
    RBrace,
    Lt, Gt, Le, Ge, Ne, // < > <= >= <>
}

impl Token {
    pub fn kind_str(&self) -> &'static str {
        match self {
            Token::Ident(_) => "ident", Token::Str(_) => "str", Token::Num(_) => "num",
            Token::Bool(_) => "bool", Token::Eq => "eq", Token::LBrace => "lbrace",
            Token::RBrace => "rbrace", Token::Lt => "lt", Token::Gt => "gt",
            Token::Le => "le", Token::Ge => "ge", Token::Ne => "ne",
        }
    }
}

pub fn lex(src: &str) -> Result<Vec<Token>, ParseError> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut out = Vec::new();

    while i < chars.len() {
        let c = chars[i];
        match c {
            '#' => { while i < chars.len() && chars[i] != '\n' { i += 1; } }
            '\n' => { line += 1; i += 1; }
            ws if ws.is_whitespace() => { i += 1; }
            '"' => {
                i += 1;
                let mut s = String::new();
                while i < chars.len() && chars[i] != '"' { s.push(chars[i]); i += 1; }
                if i >= chars.len() { return Err(ParseError::UnexpectedEof); }
                i += 1; // 跳过闭合引号
                out.push(Token::Str(s));
            }
            '=' => { out.push(Token::Eq); i += 1; }
            '{' => { out.push(Token::LBrace); i += 1; }
            '}' => { out.push(Token::RBrace); i += 1; }
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' { out.push(Token::Le); i += 2; }
                else { out.push(Token::Lt); i += 1; }
            }
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' { out.push(Token::Ge); i += 2; }
                else { out.push(Token::Gt); i += 1; }
            }
            d if d.is_ascii_digit()
                || (d == '-' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
                || d == '.' =>
            {
                let start = i;
                if chars[i] == '-' { i += 1; }
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') { i += 1; }
                let s: String = chars[start..i].iter().collect();
                let n: f64 = s
                    .parse()
                    .map_err(|_| ParseError::Syntax { line, msg: format!("非法数字: {s}") })?;
                out.push(Token::Num(n));
            }
            a if a.is_ascii_alphabetic() || a == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                match s.as_str() {
                    "yes" => out.push(Token::Bool(true)),
                    "no" => out.push(Token::Bool(false)),
                    _ => out.push(Token::Ident(s)),
                }
            }
            _ => {
                return Err(ParseError::Syntax {
                    line,
                    msg: format!("意外字符: {c}"),
                })
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_simple_assignment() {
        let toks = lex("id = AFG_expand_telegraph_network").unwrap();
        assert_eq!(toks.len(), 3);
        assert!(matches!(&toks[0], Token::Ident(s) if s == "id"));
        assert!(matches!(&toks[1], Token::Eq));
        assert!(matches!(&toks[2], Token::Ident(s) if s == "AFG_expand_telegraph_network"));
    }

    #[test]
    fn t_block_and_string() {
        let src = r#"country = { factor = 0 has_dlc = "Graveyard of Empires" }"#;
        let toks = lex(src).unwrap();
        let kinds: Vec<&str> = toks.iter().map(|t| t.kind_str()).collect();
        assert_eq!(
            kinds,
            vec!["ident", "eq", "lbrace", "ident", "eq", "num", "ident", "eq", "str", "rbrace"]
        );
    }

    #[test]
    fn t_negative_and_bool() {
        let toks = lex("x = -21 active = yes").unwrap();
        assert!(matches!(toks[2], Token::Num(n) if n == -21.0));
        assert!(matches!(toks[5], Token::Bool(true)));
    }

    #[test]
    fn t_comment_stripped() {
        let toks = lex("id = AFG # comment\nx = 1").unwrap();
        assert!(toks.iter().all(|t| !matches!(t, Token::Ident(s) if s.contains("comment"))));
        assert!(toks.iter().any(|t| matches!(t, Token::Num(n) if *n == 1.0)));
    }
}
