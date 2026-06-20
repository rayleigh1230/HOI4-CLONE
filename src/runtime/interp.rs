//! Interpreter: 解释执行 Effect AST (Task 7 完整实现)
use crate::ast::{Arg, CompareOp, Effect, Trigger};
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
            self.run_one(e, world);
        }
    }

    fn run_one(&self, e: &Effect, world: &mut World) {
        match e {
            Effect::Command { name, args } => {
                if let Some(f) = self.reg.get(name) {
                    f(world, args);
                } else {
                    eprintln!("[warn] 未注册的 effect: {name}");
                }
            }
            Effect::If { cond, then, els } => {
                if self.eval(cond, world) {
                    self.run(then, world);
                } else {
                    self.run(els, world);
                }
            }
            Effect::ForEach { scope, filter, body } => {
                // M1: 作用域遍历简化为"执行一次"(不实际枚举省份/国家)
                if filter.as_ref().is_none_or(|t| self.eval(t, world)) {
                    eprintln!("[info] {scope}: 执行作用域体(M1 简化为单次)");
                    self.run(body, world);
                }
            }
            Effect::Random { table } => {
                if let Some((_, crate::ast::RandomPick::EventId(id))) = table.first() {
                    eprintln!("[info] random_events 选中: {id} (M1 不触发事件)");
                }
            }
        }
    }

    pub fn eval(&self, t: &Trigger, world: &World) -> bool {
        match t {
            Trigger::Always(b) => *b,
            Trigger::And(parts) => parts.iter().all(|p| self.eval(p, world)),
            Trigger::Or(parts) => parts.iter().any(|p| self.eval(p, world)),
            Trigger::Not(inner) => !self.eval(inner, world),
            Trigger::Compare { lhs, op, rhs } => {
                let l = world.get_var(lhs);
                let r = match rhs {
                    Arg::Num(n) => *n,
                    _ => return false,
                };
                match op {
                    CompareOp::Lt => l < r,
                    CompareOp::Gt => l > r,
                    CompareOp::Le => l <= r,
                    CompareOp::Ge => l >= r,
                    CompareOp::Eq => (l - r).abs() < 1e-9,
                    CompareOp::Ne => (l - r).abs() >= 1e-9,
                }
            }
            Trigger::Check { name: _, args: _ } => {
                // M1: 触发器命令(如 has_dlc/is_major)无法真实验证,默认 true 让脚本能跑通
                true
            }
        }
    }
}
