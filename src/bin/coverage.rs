//! 覆盖度诊断: 对真实 HOI4 脚本跑 parse → lower → exec 三阶段,统计通过率与失败点
//!
//! 用法:
//!   cargo run --release --bin coverage <文件或目录>...
//!   cargo run --release --bin coverage "G:/steam/.../common/national_focus/germany.txt"
//!   cargo run --release --bin coverage "G:/steam/.../events"
//!
//! 只读,不修改任何游戏文件。
use hoi4_clone::ast::lower::lower_effects;
use hoi4_clone::ast::{Effect, Trigger};
use hoi4_clone::commands::register_all;
use hoi4_clone::parser::parse;
use hoi4_clone::runtime::{Interpreter, Registry, World};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct Stats {
    files: usize,
    parse_ok: usize,
    parse_ok_partial: usize,
    parse_fail: usize,
    effects_total: usize,
    unknown_effects: HashMap<String, usize>,
    unknown_triggers: HashMap<String, usize>,
    exec_error_count: usize,
    parse_fail_samples: Vec<(String, String)>,
    scope_keywords: HashMap<String, usize>,
}

/// 容错预处理: 剥离已知会卡住 lexer 的 HOI4 语法结构,用于探测深层 effect/trigger 覆盖度。
/// 注意: 这只用于"能解析多深"的探测,不代表真实兼容。
fn clean_for_probe(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    for raw in src.lines() {
        let line = raw.strip_prefix('\u{feff}').unwrap_or(raw);
        let trimmed = line.trim_start();
        // 1. 跳过文件级常量定义 @xxx = N
        if trimmed.starts_with('@') {
            continue;
        }
        // 2. 引号外把 |xxx (本地化子键分隔) 削掉
        let mut in_str = false;
        let mut cleaned_line = String::with_capacity(line.len());
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    in_str = !in_str;
                    cleaned_line.push(c);
                }
                '|' if !in_str => {
                    // 吃掉 | 后到下一个空白/引号的内容
                    while let Some(&nc) = chars.peek() {
                        if nc.is_whitespace() || nc == '"' { break; }
                        chars.next();
                    }
                }
                _ => cleaned_line.push(c),
            }
        }
        out.push_str(&cleaned_line);
        out.push('\n');
    }
    out
}

fn collect_txt(root: &Path, out: &mut Vec<PathBuf>) {
    if root.is_file() {
        if root.extension().and_then(|e| e.to_str()) == Some("txt") {
            out.push(root.to_path_buf());
        }
        return;
    }
    let Ok(rd) = fs::read_dir(root) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_txt(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("txt") {
            out.push(p);
        }
    }
}

/// 走查 AST,统计未知 effect/trigger/作用域关键字(只读,基于 registry 引用)
/// 对 Command 的 Block 参数递归走查,因为 HOI4 里嵌套命令(asd={inner_cmd=...})很常见
fn walk_effects(effs: &[Effect], stats: &mut Stats, reg: &Registry) {
    for e in effs {
        match e {
            Effect::Command { name, params } => {
                stats.effects_total += 1;
                if reg.get_effect(name).is_none() {
                    *stats.unknown_effects.entry(name.clone()).or_default() += 1;
                }
                // 递归进 Block 参数:很多 effect 体是 { 嵌套 effect 块 }
                for (_, arg) in params {
                    if let hoi4_clone::ast::Arg::Block(fields) = arg {
                        // 把 fields 当成 Block 复用 lower 逻辑
                        let inner = hoi4_clone::parser::Block {
                            fields: fields
                                .iter()
                                .map(|(k, v)| hoi4_clone::parser::Field {
                                    key: k.clone(),
                                    value: arg_to_value(v),
                                })
                                .collect(),
                        };
                        let inner_effs = lower_effects(&inner);
                        walk_effects(&inner_effs, stats, reg);
                    }
                }
            }
            Effect::If { cond, then, els } => {
                walk_trigger(cond, stats, reg);
                walk_effects(then, stats, reg);
                walk_effects(els, stats, reg);
            }
            Effect::ForEach { scope, filter, body } => {
                *stats.scope_keywords.entry(scope.clone()).or_default() += 1;
                if let Some(t) = filter {
                    walk_trigger(t, stats, reg);
                }
                walk_effects(body, stats, reg);
            }
            Effect::Random { .. } => {}
        }
    }
}

/// Arg → Value 反向转换,用于把 Block 参数重新喂回 lower 走查
fn arg_to_value(a: &hoi4_clone::ast::Arg) -> hoi4_clone::parser::Value {
    use hoi4_clone::ast::Arg;
    use hoi4_clone::parser::Value;
    match a {
        Arg::Num(n) => Value::Scalar(n.to_string()),
        Arg::Str(s) => Value::Scalar(s.clone()),
        Arg::Bool(b) => Value::Scalar(b.to_string()),
        Arg::Block(fields) => Value::Block(hoi4_clone::parser::Block {
            fields: fields
                .iter()
                .map(|(k, v)| hoi4_clone::parser::Field {
                    key: k.clone(),
                    value: arg_to_value(v),
                })
                .collect(),
        }),
    }
}

fn walk_trigger(t: &Trigger, stats: &mut Stats, reg: &Registry) {
    match t {
        Trigger::Check { name, .. } => {
            if reg.get_trigger(name).is_none() {
                *stats.unknown_triggers.entry(name.clone()).or_default() += 1;
            }
        }
        Trigger::And(v) | Trigger::Or(v) => {
            for x in v {
                walk_trigger(x, stats, reg)
            }
        }
        Trigger::Not(b) => walk_trigger(b, stats, reg),
        _ => {}
    }
}

fn process_file(path: &Path, stats: &mut Stats, interp: &Interpreter, reg: &Registry) {
    let Ok(mut src) = fs::read_to_string(path) else {
        return;
    };
    // 剥离 UTF-8 BOM(原版文件普遍带 BOM;这是脚本运行时应处理的,非作弊)
    if src.starts_with('\u{feff}') {
        src = src[3..].to_string();
    }
    stats.files += 1;

    let block = match parse(&src) {
        Ok(b) => {
            stats.parse_ok += 1;
            b
        }
        Err(e) => {
            stats.parse_fail += 1;
            if stats.parse_fail_samples.len() < 10 {
                stats.parse_fail_samples.push((path.display().to_string(), format!("{e}")));
            }
            // 容错模式: 剥离已知会触发 lexer 失败的语法结构后重试
            // (@常量定义、引号外 | 分隔的本地化键、行内 # 注释已在 lexer 处理)
            let cleaned = clean_for_probe(&src);
            match parse(&cleaned) {
                Ok(b) => {
                    stats.parse_ok_partial += 1;
                    b
                }
                Err(e2) => {
                    if stats.parse_fail_samples.len() < 10 {
                        stats.parse_fail_samples.push((
                            path.display().to_string(),
                            format!("(容错后仍失败) {e2}"),
                        ));
                    }
                    return;
                }
            }
        }
    };

    let effs = lower_effects(&block);
    walk_effects(&effs, stats, reg);

    // 试执行: 统计 error_log 增量
    let mut w = World::new();
    w.player_tag = "___probe___".into();
    let before = w.error_log.len();
    interp.run(&effs, &mut w);
    stats.exec_error_count += w.error_log.len() - before;
}

fn print_top(title: &str, map: HashMap<String, usize>, n: usize) {
    println!("\n--- {title} (TOP {n}) ---");
    let mut v: Vec<_> = map.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    if v.is_empty() {
        println!("  (无)");
    }
    for (name, c) in v.iter().take(n) {
        println!("  {c:>6}  {name}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("用法: coverage <文件或目录>...");
        std::process::exit(1);
    }

    let mut files = Vec::new();
    for a in &args {
        collect_txt(Path::new(a), &mut files);
    }
    println!("扫描到 {} 个 .txt 文件", files.len());

    let mut reg = Registry::new();
    register_all(&mut reg);
    let interp = Interpreter::new(reg_empty());
    let mut stats = Stats::default();
    for f in &files {
        process_file(f, &mut stats, &interp, &reg);
    }

    println!("\n========== 覆盖度报告 ==========");
    println!("文件总数        : {}", stats.files);
    println!(
        "parse 完全成功  : {} ({:.1}%)",
        stats.parse_ok,
        if stats.files > 0 { 100.0 * stats.parse_ok as f64 / stats.files as f64 } else { 0.0 }
    );
    println!(
        "parse 容错成功  : {} (剔除@/#行后)",
        stats.parse_ok_partial
    );
    println!("parse 失败      : {}", stats.parse_fail);
    println!("顶层 effect 总数: {}", stats.effects_total);
    println!("执行期错误次数 : {}", stats.exec_error_count);

    println!("\n--- 解析失败样例(前 10) ---");
    if stats.parse_fail_samples.is_empty() {
        println!("  (无)");
    } else {
        for (p, e) in &stats.parse_fail_samples {
            println!("  {p}: {e}");
        }
    }

    print_top("未实现的 effect", stats.unknown_effects, 30);
    print_top("未实现的 trigger", stats.unknown_triggers, 30);
    print_top("作用域关键字", stats.scope_keywords, 20);
}

/// 构造一个空 registry 给 Interpreter(执行只为触发 error_log,不需要真命令)
fn reg_empty() -> Registry {
    Registry::new()
}
