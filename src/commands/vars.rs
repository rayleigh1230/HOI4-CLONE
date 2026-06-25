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
}
