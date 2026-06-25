//! 变量类命令注册(M2: Result + params 签名)
use crate::ast::Arg;
use crate::runtime::error::CmdError;
use crate::runtime::registry::ParamGet;
use crate::runtime::Registry;

/// 取当前作用域国家的可变引用(栈优先回退 player_tag)。
/// 无国家时返回 RuntimeError(决策5: 不静默吞)。
fn scope_country_mut(w: &mut crate::runtime::World) -> Result<&mut crate::runtime::Country, crate::runtime::error::CmdError> {
    let tag = w.current_country_tag()
        .ok_or_else(|| crate::runtime::error::CmdError::RuntimeError(
            "资源命令需要国家作用域(player_tag 空或无 Country scope)".into()
        ))?;
    w.countries.entry(tag.clone()).or_default();
    Ok(w.countries.get_mut(&tag)
        .expect("刚 or_default 插入, 必然存在"))
}

pub fn register(reg: &mut Registry) {
    reg.register("set_stability", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("set_stability"))?;
        scope_country_mut(w)?.stability = n;
        Ok(())
    });
    reg.register("add_stability", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("add_stability"))?;
        scope_country_mut(w)?.stability += n;
        Ok(())
    });
    reg.register("add_political_power", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("add_political_power"))?;
        scope_country_mut(w)?.political_power += n;
        Ok(())
    });
    reg.register("set_political_power", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("set_political_power"))?;
        scope_country_mut(w)?.political_power = n;
        Ok(())
    });
    reg.register("create_country", |w, p| {
        let tag = ParamGet::get(p, "tag").and_then(Arg::as_str)
            .ok_or_else(|| crate::runtime::error::CmdError::BadParam {
                cmd: "create_country".into(), key: "tag".into(), reason: "缺少 tag".into()
            })?;
        let pp = ParamGet::get(p, "political_power").and_then(Arg::as_num).unwrap_or(0.0);
        let stab = ParamGet::get(p, "stability").and_then(Arg::as_num).unwrap_or(0.5);
        let ws = ParamGet::get(p, "war_support").and_then(Arg::as_num).unwrap_or(0.5);
        let cap = ParamGet::get(p, "capital_state").and_then(Arg::as_num).unwrap_or(0.0) as u32;
        // 已存在则覆盖资源字段(以最后一次为准, 对齐原版 history 加载语义)
        let c = w.countries.entry(tag.into()).or_default();
        c.tag = tag.into();
        c.political_power = pp;
        c.stability = stab;
        c.war_support = ws;
        c.capital_state = cap;
        Ok(())
    });
    reg.register("add_war_support", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("add_war_support"))?;
        scope_country_mut(w)?.war_support += n;
        Ok(())
    });
    reg.register("set_war_support", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("set_war_support"))?;
        scope_country_mut(w)?.war_support = n;
        Ok(())
    });
    reg.register("add_to_variable", |w, p| apply_var_block(w, p, false));
    reg.register("set_variable", |w, p| apply_var_block(w, p, true));
    reg.register("set_flag", |w, p| {
        let s = p.pos(0).and_then(Arg::as_str).ok_or_else(|| bad_param("set_flag"))?;
        w.set_flag(s);
        Ok(())
    });
}

/// 处理 add_to_variable/set_variable 的参数(命名字段或嵌套块)
fn apply_var_block(
    w: &mut crate::runtime::World,
    p: &[(String, Arg)],
    is_set: bool,
) -> Result<(), CmdError> {
    for (k, v) in p {
        match v {
            Arg::Block(fields) => {
                for (vk, vv) in fields {
                    if let Some(n) = vv.as_num() {
                        if is_set { w.set_var(vk, n); } else { w.add_var(vk, n); }
                    }
                }
            }
            _ if !k.is_empty() => {
                if let Some(n) = v.as_num() {
                    if is_set { w.set_var(k, n); } else { w.add_var(k, n); }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn bad_param(cmd: &str) -> CmdError {
    CmdError::BadParam { cmd: cmd.to_string(), key: "pos[0]".into(), reason: "缺少或类型错误".into() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::World;

    #[test]
    fn t_add_stability_cmd() {
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        w.player_tag = "X".into();          // ★ 设国家作用域
        w.countries.insert("X".into(), Default::default());
        let f = reg.get_effect("add_stability").unwrap();
        f(&mut w, &[("".into(), Arg::Num(0.05))]).unwrap();
        // ★ 读 Country 字段, 非全局 var(默认 0.5 + 0.05 = 0.55)
        let stab = w.countries.get("X").unwrap().stability;
        assert!((stab - 0.55).abs() < 1e-9, "默认0.5+0.05=0.55, 实际 {}", stab);
    }

    #[test]
    fn t_add_to_variable_named_field() {
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        let f = reg.get_effect("add_to_variable").unwrap();
        f(&mut w, &[("AFG_x".into(), Arg::Num(0.05))]).unwrap();
        assert!((w.get_var("AFG_x") - 0.05).abs() < 1e-9);
    }

    #[test]
    fn t_command_returns_error_on_bad_param() {
        // P0-3: 坏参数返回 Err
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        let f = reg.get_effect("add_stability").unwrap();
        let result = f(&mut w, &[]); // 空参数
        assert!(result.is_err());
    }

    #[test]
    fn t_add_political_power_targets_scope_country() {
        // add_political_power 改当前作用域国家(player_tag 回退), 不是全局
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        w.player_tag = "GER".into();
        w.countries.insert("GER".into(), Default::default());
        let f = reg.get_effect("add_political_power").unwrap();
        f(&mut w, &[("".into(), Arg::Num(50.0))]).unwrap();
        let pp = w.countries.get("GER").unwrap().political_power;
        assert!((pp - 50.0).abs() < 1e-9, "PP 应加到 GER 国家");
    }

    #[test]
    fn t_add_stability_targets_scope_country() {
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        w.player_tag = "GER".into();
        w.countries.insert("GER".into(), Default::default());
        let f = reg.get_effect("add_stability").unwrap();
        f(&mut w, &[("".into(), Arg::Num(0.1))]).unwrap();
        let stab = w.countries.get("GER").unwrap().stability;
        // 默认 0.5 + 0.1 = 0.6
        assert!((stab - 0.6).abs() < 1e-9, "稳定度应 0.5+0.1=0.6, 实际 {}", stab);
    }

    #[test]
    fn t_resource_command_errors_without_country() {
        // 无国家作用域(player_tag 空) → 报错(决策5: 不静默吞)
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        w.player_tag = String::new(); // 无国家
        let f = reg.get_effect("add_political_power").unwrap();
        let result = f(&mut w, &[("".into(), Arg::Num(50.0))]);
        assert!(result.is_err(), "无国家时 add_political_power 应返回 Err");
    }

    #[test]
    fn t_create_country_sets_resources() {
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        let f = reg.get_effect("create_country").unwrap();
        f(&mut w, &[
            ("tag".into(), Arg::Str("GER".into())),
            ("political_power".into(), Arg::Num(50.0)),
            ("stability".into(), Arg::Num(0.7)),
            ("war_support".into(), Arg::Num(0.3)),
            ("capital_state".into(), Arg::Num(1.0)),
        ]).unwrap();
        let c = w.countries.get("GER").unwrap();
        assert!((c.political_power - 50.0).abs() < 1e-9);
        assert!((c.stability - 0.7).abs() < 1e-9);
        assert!((c.war_support - 0.3).abs() < 1e-9);
        assert_eq!(c.capital_state, 1);
        assert_eq!(c.tag, "GER");
    }

    #[test]
    fn t_create_country_optional_fields_default() {
        // 缺省字段用 Default(PP=0, stability/war_support=0.5)
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        let f = reg.get_effect("create_country").unwrap();
        f(&mut w, &[("tag".into(), Arg::Str("X".into()))]).unwrap();
        let c = w.countries.get("X").unwrap();
        assert!((c.political_power).abs() < 1e-9);
        assert!((c.stability - 0.5).abs() < 1e-9);
        assert!((c.war_support - 0.5).abs() < 1e-9);
    }

    #[test]
    fn t_add_war_support_targets_country() {
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        w.player_tag = "GER".into();
        w.countries.insert("GER".into(), Default::default());
        let f = reg.get_effect("add_war_support").unwrap();
        f(&mut w, &[("".into(), Arg::Num(0.1))]).unwrap();
        let ws = w.countries.get("GER").unwrap().war_support;
        assert!((ws - 0.6).abs() < 1e-9, "默认0.5+0.1=0.6");
    }
}
