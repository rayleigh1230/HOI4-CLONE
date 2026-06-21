//! 游戏主循环: hourly tick + on_actions 钩子分发(spec §4.2.1)
use crate::runtime::{Interpreter, World};

pub struct GameClock;

impl GameClock {
    /// 推进游戏 1 小时, 触发相应钩子
    /// 注: 用 % 而非 is_multiple_of, 保持对 Rust <1.87 的兼容(Cargo.toml 未钉版本)
    #[allow(clippy::manual_is_multiple_of)]
    pub fn tick(interp: &Interpreter, world: &mut World) {
        world.hour += 1;
        world.fire_event(interp, "on_hourly");
        crate::combat::movement::check_engagements(world); // 检查移动中师是否遇敌→开战
        crate::combat::resolve::resolve_all_battles(world); // 战斗结算(含撤退/包围判定)
        crate::combat::width::reinforce_reserves(world); // 预备队补位
        crate::combat::movement::advance_movement(&mut *world); // 行军推进
        crate::combat::recovery::recover_org(world); // 组织度恢复(非战斗师)
        if world.hour % 24 == 0 {
            world.fire_event(interp, "on_daily");
            world.fire_event(interp, &format!("on_daily_{}", world.player_tag));
            crate::combat::reinforce::reinforce_all(world); // M4a: 每日增援补装备
        }
        if world.hour % (24 * 7) == 0 {
            world.fire_event(interp, "on_weekly");
        }
        if world.hour % (24 * 30) == 0 {
            world.fire_event(interp, "on_monthly");
        }
    }

    /// 推进 n 小时
    pub fn advance(interp: &Interpreter, world: &mut World, hours: u64) {
        for _ in 0..hours {
            Self::tick(interp, world);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Arg, Effect};
    use crate::commands::register_all;
    use crate::runtime::{Interpreter, Registry, World};

    #[test]
    fn t_daily_hook_fires_after_24_ticks() {
        let mut reg = Registry::new();
        register_all(&mut reg);
        let interp = Interpreter::new(reg);
        let mut world = World::new();
        world.on(
            "on_daily",
            vec![Effect::Command {
                name: "add_political_power".into(),
                params: vec![("".into(), Arg::Num(1.0))],
            }],
        );
        GameClock::advance(&interp, &mut world, 23);
        assert!(world.get_var("political_power").abs() < 1e-9);
        GameClock::tick(&interp, &mut world); // 第 24 次
        assert!(
            (world.get_var("political_power") - 1.0).abs() < 1e-9,
            "24h 后 on_daily 应触发"
        );
    }

    #[test]
    fn t_hourly_fires_every_tick() {
        let mut reg = Registry::new();
        register_all(&mut reg);
        let interp = Interpreter::new(reg);
        let mut world = World::new();
        world.on(
            "on_hourly",
            vec![Effect::Command {
                name: "add_political_power".into(),
                params: vec![("".into(), Arg::Num(0.5))],
            }],
        );
        GameClock::advance(&interp, &mut world, 10);
        assert!(
            (world.get_var("political_power") - 5.0).abs() < 1e-9,
            "10 tick 应加 5.0"
        );
    }
}
