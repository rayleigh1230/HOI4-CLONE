//! Interpreter: 解释执行 Effect AST(M2: Result + Check 查表 + 错误收集)
use crate::ast::{Arg, CompareOp, Effect, Trigger};
use crate::runtime::error::CmdError;
use crate::runtime::{Registry, World};

pub struct Interpreter {
    pub reg: Registry,
}

impl Interpreter {
    pub fn new(reg: Registry) -> Self {
        Self { reg }
    }

    pub fn run(&self, effs: &[Effect], world: &mut World) {
        for e in effs {
            if let Err(err) = self.run_one(e, world) {
                world.error_log.push(err);
            }
        }
    }

    fn run_one(&self, e: &Effect, world: &mut World) -> Result<(), CmdError> {
        match e {
            Effect::Command { name, params } => match self.reg.get_effect(name) {
                Some(f) => f(world, params),
                None => {
                    eprintln!("[warn] 未注册的 effect: {name}");
                    Err(CmdError::UnknownCommand(name.clone()))
                }
            },
            Effect::If { cond, then, els } => {
                if self.eval(cond, world)? {
                    self.run(then, world);
                } else {
                    self.run(els, world);
                }
                Ok(())
            }
            Effect::ForEach { scope, filter, body } => {
                self.run_for_each(scope, filter.as_ref(), body, world)?;
                Ok(())
            }
            Effect::Random { table } => {
                if let Some((_, crate::ast::RandomPick::EventId(id))) = table.first() {
                    eprintln!("[info] random_events 选中: {id} (M2 不触发事件)");
                }
                Ok(())
            }
        }
    }

    /// 作用域遍历: 根据 scope 名枚举实体, 每个压栈执行 body(M3 真实枚举)
    fn run_for_each(
        &self,
        scope_name: &str,
        filter: Option<&Trigger>,
        body: &[Effect],
        world: &mut World,
    ) -> Result<(), CmdError> {
        // 先收集要遍历的 Scope 列表(避免遍历时借用 world)
        let targets: Vec<crate::runtime::Scope> = match scope_name {
            "every_country" | "all_country" => world
                .countries
                .keys()
                .map(|t| crate::runtime::Scope::Country(t.clone()))
                .collect(),
            "random_country" => {
                let tags: Vec<String> = world.countries.keys().cloned().collect();
                if tags.is_empty() {
                    return Ok(());
                }
                // M3 确定性取首个(不引入 rand; 真正随机 M5)
                vec![crate::runtime::Scope::Country(tags.into_iter().next().unwrap())]
            }
            "every_owned_state" | "all_owned_state" => {
                let tag = match world.current_country() {
                    Some(t) => t.to_string(),
                    None => return Ok(()),
                };
                world
                    .countries
                    .get(&tag)
                    .map(|c| {
                        c.owned_states
                            .iter()
                            .map(|p| crate::runtime::Scope::Province(*p))
                            .collect()
                    })
                    .unwrap_or_default()
            }
            "all_army" | "every_army" => {
                let tag = match world.current_country() {
                    Some(t) => t.to_string(),
                    None => return Ok(()),
                };
                world
                    .divisions_of(&tag)
                    .into_iter()
                    .map(crate::runtime::Scope::Division)
                    .collect()
            }
            _ => {
                eprintln!("[warn] 未知作用域: {scope_name}, 跳过");
                return Ok(());
            }
        };

        for target in targets {
            world.scope_stack.push(target);
            let pass = match filter {
                Some(t) => self.eval(t, world)?,
                None => true,
            };
            if pass {
                self.run(body, world);
            }
            world.scope_stack.pop();
        }
        Ok(())
    }

    pub fn eval(&self, t: &Trigger, world: &World) -> Result<bool, CmdError> {
        match t {
            Trigger::Always(b) => Ok(*b),
            Trigger::And(parts) => {
                for p in parts {
                    if !self.eval(p, world)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Trigger::Or(parts) => {
                for p in parts {
                    if self.eval(p, world)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Trigger::Not(inner) => Ok(!self.eval(inner, world)?),
            Trigger::Compare { lhs, op, rhs } => {
                let l = world.get_var(lhs);
                let r = match rhs {
                    Arg::Num(n) => *n,
                    _ => return Ok(false),
                };
                Ok(match op {
                    CompareOp::Lt => l < r,
                    CompareOp::Gt => l > r,
                    CompareOp::Le => l <= r,
                    CompareOp::Ge => l >= r,
                    CompareOp::Eq => (l - r).abs() < 1e-9,
                    CompareOp::Ne => (l - r).abs() >= 1e-9,
                })
            }
            Trigger::Check { name, args } => match self.reg.get_trigger(name) {
                Some(f) => f(world, args),
                None => {
                    eprintln!("[debug] 未注册的 trigger: {name}, 默认 false");
                    Ok(false)
                }
            },
        }
    }
}
