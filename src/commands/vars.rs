//! 变量类命令注册
use crate::ast::Arg;
use crate::runtime::Registry;
#[allow(unused_imports)]
use crate::runtime::World;

pub fn register(reg: &mut Registry) {
    reg.register("set_stability", |w, a| {
        if let Some(Arg::Num(n)) = a.first() {
            w.set_var("stability", *n)
        }
    });
    reg.register("add_stability", |w, a| {
        if let Some(Arg::Num(n)) = a.first() {
            w.add_var("stability", *n)
        }
    });
    reg.register("add_political_power", |w, a| {
        if let Some(Arg::Num(n)) = a.first() {
            w.add_var("political_power", *n)
        }
    });
    reg.register("set_political_power", |w, a| {
        if let Some(Arg::Num(n)) = a.first() {
            w.set_var("political_power", *n)
        }
    });
    reg.register("add_to_variable", |w, a| {
        // args[0] = "varname=value"
        if let Some(Arg::Str(s)) = a.first() {
            if let Some((k, v)) = s.split_once('=') {
                if let Ok(n) = v.trim().parse::<f64>() {
                    w.add_var(k.trim(), n);
                }
            }
        }
    });
    reg.register("set_variable", |w, a| {
        if let Some(Arg::Str(s)) = a.first() {
            if let Some((k, v)) = s.split_once('=') {
                if let Ok(n) = v.trim().parse::<f64>() {
                    w.set_var(k.trim(), n);
                }
            }
        }
    });
    reg.register("set_flag", |w, a| {
        if let Some(Arg::Str(s)) = a.first() {
            w.set_flag(s);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_add_stability_cmd() {
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        let f = reg.get("add_stability").unwrap();
        f(&mut w, &[Arg::Num(0.05)]);
        assert!((w.get_var("stability") - 0.05).abs() < 1e-9);
    }

    #[test]
    fn t_add_to_variable_block_arg() {
        // 模拟 add_to_variable = { x = 0.05 } 经 lower 后 args[0] = "x=0.05"
        let mut reg = Registry::new();
        register(&mut reg);
        let mut w = World::new();
        let f = reg.get("add_to_variable").unwrap();
        f(&mut w, &[Arg::Str("AFG_state_development_production_speed=0.05".into())]);
        assert!((w.get_var("AFG_state_development_production_speed") - 0.05).abs() < 1e-9);
    }
}
