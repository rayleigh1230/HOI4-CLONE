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
                let pass = match filter {
                    Some(t) => self.eval(t, world)?,
                    None => true,
                };
                if pass {
                    eprintln!("[info] {scope}: 执行作用域体(M2 简化为单次)");
                    self.run(body, world);
                }
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
