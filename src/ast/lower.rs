//! Block → AST 降级转换
//!
//! 识别规则:
//! - `limit = { ... }` → Trigger
//! - `if` / `every_*` / `random_events` → 特殊 Effect
//! - 其他 `key = scalar` → Command; 在 limit 上下文 → Check/Compare
use crate::ast::{Arg, CompareOp, Effect, RandomPick, Trigger};
use crate::parser::{Block, Field, Value};

/// 把顶层 Block 的每个 field 当作一条 effect
pub fn lower_effects(b: &Block) -> Vec<Effect> {
    b.fields.iter().flat_map(lower_field_as_effect).collect()
}

/// 把 limit 块转成 trigger
pub fn lower_trigger(b: &Block) -> Trigger {
    let parts: Vec<Trigger> = b.fields.iter().map(lower_field_as_trigger).collect();
    match parts.len() {
        0 => Trigger::Always(true),
        1 => parts.into_iter().next().unwrap(),
        _ => Trigger::And(parts),
    }
}

fn lower_field_as_effect(f: &Field) -> Vec<Effect> {
    let mut out = Vec::new();
    match (&f.key, &f.value) {
        (k, Value::Block(inner)) if k == "if" => {
            let (cond, then, els) = split_if(inner);
            out.push(Effect::If { cond, then, els });
        }
        (k, Value::Block(inner))
            if k.starts_with("every_") || k.starts_with("random_") || k.starts_with("all_") =>
        {
            let (filter, body) = split_scoped(inner);
            out.push(Effect::ForEach { scope: k.clone(), filter, body });
        }
        (k, Value::Block(inner)) if k == "random_events" => {
            let table = inner
                .fields
                .iter()
                .filter_map(|f| {
                    let w: f64 = f.key.parse().ok()?;
                    if let Value::Scalar(ev) = &f.value {
                        Some((w, RandomPick::EventId(ev.clone())))
                    } else {
                        None
                    }
                })
                .collect();
            out.push(Effect::Random { table });
        }
        (k, Value::Scalar(s)) => {
            // 位置参数用空 key
            out.push(Effect::Command { name: k.clone(), params: vec![(String::new(), parse_arg(s))] });
        }
        // 命令带块参数,如 add_to_variable = { x = 0.05 }: 命名字段, 嵌套块递归(P0-1 修复)
        (k, Value::Block(inner)) => {
            let params = inner
                .fields
                .iter()
                .map(|f| (f.key.clone(), parse_value(&f.value)))
                .collect();
            out.push(Effect::Command { name: k.clone(), params });
        }
    }
    out
}

fn lower_field_as_trigger(f: &Field) -> Trigger {
    match (&f.key, &f.value) {
        (k, Value::Block(b)) if k == "AND" => {
            Trigger::And(b.fields.iter().map(lower_field_as_trigger).collect())
        }
        (k, Value::Block(b)) if k == "OR" => {
            Trigger::Or(b.fields.iter().map(lower_field_as_trigger).collect())
        }
        (k, Value::Block(b)) if k == "NOT" => Trigger::Not(Box::new(lower_trigger(b))),
        (k, Value::Scalar(s)) => {
            // 检测 parser 标记的裸比较: value 形如 ">=150" "<=10" ">x" 等
            if let Some((op, rhs)) = parse_compare_scalar(s) {
                let cmp_op = match op {
                    ">=" => CompareOp::Ge,
                    "<=" => CompareOp::Le,
                    ">" => CompareOp::Gt,
                    "<" => CompareOp::Lt,
                    "<>" => CompareOp::Ne,
                    _ => return Trigger::Check { name: k.clone(), args: vec![(String::new(), parse_arg(s))] },
                };
                return Trigger::Compare {
                    lhs: k.clone(),
                    op: cmp_op,
                    rhs: parse_arg(rhs),
                };
            }
            // 简单形式: tag = GER → Check
            Trigger::Check { name: k.clone(), args: vec![(String::new(), parse_arg(s))] }
        }
        _ => Trigger::Always(true),
    }
}

/// 尝试从 "op rhs" 格式的 scalar 解析出比较运算。
/// parser 把裸比较 `var >= 150` 存成 value=">=150"
fn parse_compare_scalar(s: &str) -> Option<(&str, &str)> {
    for op in &["<=", ">=", "<>", ">", "<"] {
        if let Some(rest) = s.strip_prefix(op) {
            if !rest.is_empty() {
                return Some((op, rest));
            }
        }
    }
    None
}

fn split_if(b: &Block) -> (Trigger, Vec<Effect>, Vec<Effect>) {
    let mut cond = Trigger::Always(true);
    let mut then = Vec::new();
    let mut els = Vec::new();
    // else_if 需要单独收集:它本身是嵌套 If(带自己的 limit),不能平铺
    let mut else_if_block: Option<&Block> = None;
    for f in &b.fields {
        if f.key == "limit" {
            if let Value::Block(lb) = &f.value {
                cond = lower_trigger(lb);
            }
        } else if f.key == "else" {
            if let Value::Block(eb) = &f.value {
                els = lower_effects(eb);
            }
        } else if f.key == "else_if" {
            // HOI4: else_if = { limit={...} <body> } 本身是一个嵌套条件
            if let Value::Block(eb) = &f.value {
                else_if_block = Some(eb);
            }
        } else {
            then.extend(lower_field_as_effect(f));
        }
    }
    // 把 else_if 转成嵌套 If 放入 els(它有自己的 limit)
    if let Some(eb) = else_if_block {
        let (ei_cond, ei_then, ei_els) = split_if(eb);
        els = vec![Effect::If { cond: ei_cond, then: ei_then, els: ei_els }];
    }
    (cond, then, els)
}

fn split_scoped(b: &Block) -> (Option<Trigger>, Vec<Effect>) {
    let mut filter = None;
    let mut body = Vec::new();
    for f in &b.fields {
        if f.key == "limit" {
            if let Value::Block(lb) = &f.value {
                filter = Some(lower_trigger(lb));
            }
        } else {
            body.extend(lower_field_as_effect(f));
        }
    }
    (filter, body)
}

fn parse_arg(s: &str) -> Arg {
    if s == "yes" {
        return Arg::Bool(true);
    }
    if s == "no" {
        return Arg::Bool(false);
    }
    if let Ok(n) = s.parse::<f64>() {
        // 排除 inf/nan: Rust 把 "inf"/"nan" 解析为无穷/NaN,
        // 但 HOI4 里这些是 ident(如装备缩写 inf)。只接受有限数字。
        if n.is_finite() {
            return Arg::Num(n);
        }
    }
    Arg::Str(s.trim_matches('"').to_string())
}

/// 把 Value 递归转 Arg, 嵌套块 → Arg::Block (P0-1: 不再扁平化丢数据)
fn parse_value(v: &Value) -> Arg {
    match v {
        Value::Scalar(s) => parse_arg(s),
        Value::Block(b) => Arg::Block(
            b.fields
                .iter()
                .map(|f| (f.key.clone(), parse_value(&f.value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn t_lower_simple_command() {
        let b = parse("add_stability = 0.05").unwrap();
        let effs = lower_effects(&b);
        assert_eq!(effs.len(), 1);
        match &effs[0] {
            Effect::Command { name, params } => {
                assert_eq!(name, "add_stability");
                assert!(matches!(params[0].1, Arg::Num(n) if (n - 0.05).abs() < 1e-9));
            }
            _ => panic!("应为 Command"),
        }
    }

    #[test]
    fn t_lower_if_block() {
        let src = "if = { limit = { has_government = fascism } add_stability = 0.05 }";
        let b = parse(src).unwrap();
        let effs = lower_effects(&b);
        assert_eq!(effs.len(), 1);
        assert!(matches!(effs[0], Effect::If { .. }));
    }

    #[test]
    fn t_lower_foreach() {
        let src = "every_owned_state = { limit = { is_owned_and_controlled_by = AFG } add_to_variable = { x = 0.05 } }";
        let b = parse(src).unwrap();
        let effs = lower_effects(&b);
        match &effs[0] {
            Effect::ForEach { scope, filter, body } => {
                assert_eq!(scope, "every_owned_state");
                assert!(filter.is_some());
                assert!(body
                    .iter()
                    .any(|e| matches!(e, Effect::Command { name, .. } if name == "add_to_variable")));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn t_lower_string_arg() {
        let b = parse(r#"set_country_name = "Germany""#).unwrap();
        let effs = lower_effects(&b);
        match &effs[0] {
            Effect::Command { params, .. } => {
                assert!(matches!(&params[0].1, Arg::Str(s) if s == "Germany"))
            }
            _ => panic!(),
        }
    }

    #[test]
    fn t_lower_nested_block_param_no_data_loss() {
        // P0-1 回归: 嵌套块参数不能丢数据
        let src = "add_equipment_production = { equipment_type = infantry_weapons amount = 10 }";
        let b = parse(src).unwrap();
        let effs = lower_effects(&b);
        match &effs[0] {
            Effect::Command { name, params } => {
                assert_eq!(name, "add_equipment_production");
                assert_eq!(params.len(), 2);
                let amount = params.iter().find(|(k, _)| k == "amount");
                assert!(matches!(amount, Some((_, Arg::Num(n))) if (*n - 10.0).abs() < 1e-9));
            }
            _ => panic!("应为 Command"),
        }
    }
}
