//! 变量类命令注册(M2: Result + params 签名)
use crate::ast::Arg;
use crate::runtime::error::CmdError;
use crate::runtime::registry::ParamGet;
use crate::runtime::Registry;

pub fn register(reg: &mut Registry) {
    reg.register("set_stability", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("set_stability"))?;
        w.set_var("stability", n);
        Ok(())
    });
    reg.register("add_stability", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("add_stability"))?;
        w.add_var("stability", n);
        Ok(())
    });
    reg.register("add_political_power", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("add_political_power"))?;
        w.add_var("political_power", n);
        Ok(())
    });
    reg.register("set_political_power", |w, p| {
        let n = p.pos(0).and_then(Arg::as_num).ok_or_else(|| bad_param("set_political_power"))?;
        w.set_var("political_power", n);
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
        let f = reg.get_effect("add_stability").unwrap();
        f(&mut w, &[("".into(), Arg::Num(0.05))]).unwrap();
        assert!((w.get_var("stability") - 0.05).abs() < 1e-9);
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
}
