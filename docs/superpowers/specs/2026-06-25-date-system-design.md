# 日期系统(GameDate 精确日历) 设计文档

> 日期: 2026-06-25
> 状态: 已批准(头脑风暴),待实现
> 关联: `docs/design-principles.md`(原则1: 原版设计是首要参考)
> 关联: `docs/HANDOFF.md`(clock.rs 当前只有 hour: u64)

---

## 0. 背景与目标

### 现状问题

当前时间只有 `World.hour: u64`(单调递增的小时计数, 从 0 开始)。clock.rs 用 `hour % 24` / `hour % (24*30)` 判定日/周/月切换——但月份长度不固定(2月28天、7月31天), 用 `% (24*30)` 算月是错的。

后续 6 个系统依赖精确日期:
- **国策**: 完成 = 开工日期 + 70 天
- **科技**: 研究 N 天
- **训练**: 师训练 90 天
- **建筑**: 建造 N 天
- **停战**: 宣战后 180 天
- **昼夜**: darkness 按日期(季节)算

若不建日期系统, 这些系统各自把 hour 换算成天数, 接口五花八门。昼夜的 darkness 更完全无法算(需要季节)。

### 目标

引入 `GameDate`(精确公历) + `World.date()` 派生查询。保留 `hour: u64` 不动(现有 clock/测试零破坏), 加日期作为查询层。

### 范围(本次做)

- **GameDate 结构**: year/month/day/hour, 精确公历(真实月份天数 + 闰年)
- **双向换算**: `from_hours` / `to_hours` / `advance_hours`
- **World.date() 派生**: 从 hour 算当前日期
- **clock.rs 月切换修正**: 改用"月份变化"判定, 不再 `% 30`
- **day_of_year**: 一年中第几天(昼夜 darkness 计算)

### 非目标(本次不做)

- 昼夜 darkness 计算(需要纬度 + 日期, 昼夜系统独立做)
- 国策/科技/训练/建筑系统本身(它们用日期接口, 但系统是独立的)
- 剧本切换的 set_date(预留命令, 但剧本系统后续做)
- 历法事件(节气等, 原版不用)

---

## 1. 核心设计决策

| # | 决策 | 选择 |
|---|---|---|
| 1 | 日历精度 | 精确公历(真实月份天数 + 闰年), 对齐原版 START_DATE = "1936.1.1.12" |
| 2 | hour 保留 | 不替换 hour: u64; GameDate 作为派生查询层(现有 clock/测试零破坏) |
| 3 | 换算基准 | 从 GameDate::START(1936.1.1.12) 起的偏移小时数 |
| 4 | 月切换判定 | 比对 tick 前后 date().month 是否变化(不用 % 30) |
| 5 | 闰年规则 | (year%4==0 && year%100!=0) \|\| year%400==0 |

---

## 2. GameDate 结构

```rust
/// 游戏日期(年.月.日.时), 精确公历, 对齐原版 START_DATE = "1936.1.1.12"
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameDate {
    pub year: u32,    // 1936..
    pub month: u32,   // 1..12
    pub day: u32,     // 1..31(按月)
    pub hour: u32,    // 0..23
}

impl GameDate {
    /// 开局日期(原版 START_DATE, 1936年1月1日12时)
    pub const START: GameDate = GameDate { year: 1936, month: 1, day: 1, hour: 12 };

    /// 从开局起经过 total_hours 小时后的日期
    /// 逐小时累加, 进位 day→month→year, 按真实月份天数(含闰年)
    pub fn from_hours(total_hours: u64) -> GameDate {
        let mut d = Self::START;
        let mut remaining = total_hours;
        // 先加到 hour 进位
        let new_hour = d.hour as u64 + remaining;
        remaining = new_hour / 24;
        d.hour = (new_hour % 24) as u32;
        // 逐天进位(检查月份天数)
        while remaining > 0 {
            let dim = days_in_month(d.year, d.month);
            if d.day as u64 + remaining <= dim as u64 {
                d.day += remaining as u32;
                remaining = 0;
            } else {
                remaining -= (dim as u64 - d.day as u64 + 1);
                d.day = 1;
                d.month += 1;
                if d.month > 12 {
                    d.month = 1;
                    d.year += 1;
                }
            }
        }
        d
    }

    /// 从该日期到开局的偏移小时数(反向换算)
    pub fn to_hours(&self) -> u64 {
        let mut total: u64 = self.hour as u64 - Self::START.hour as u64;
        let mut d = Self::START;
        while d.year < self.year || (d.year == self.year && d.month < self.month)
            || (d.year == self.year && d.month == self.month && d.day < self.day) {
            total += 24;
            // 推进一天
            let dim = days_in_month(d.year, d.month);
            d.day += 1;
            if d.day > dim {
                d.day = 1;
                d.month += 1;
                if d.month > 12 {
                    d.month = 1;
                    d.year += 1;
                }
            }
        }
        total
    }

    /// 推进 n 小时
    pub fn advance_hours(&self, hours: u64) -> GameDate {
        Self::from_hours(self.to_hours() + hours)
    }

    /// 该日期是一年中的第几天(1..365/366, 用于昼夜 darkness 计算)
    pub fn day_of_year(&self) -> u32 {
        let months_cumulative = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
        let mut doy = months_cumulative[(self.month - 1) as usize] + self.day;
        // 闰年且过了2月: +1
        if is_leap_year(self.year) && self.month > 2 {
            doy += 1;
        }
        doy
    }
}

/// 某年某月的天数
fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap_year(year) { 29 } else { 28 },
        _ => 30, // 兜底(不该出现)
    }
}

/// 闰年判定
fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
```

---

## 3. World.date() 派生

```rust
impl World {
    /// 当前游戏日期(从 hour 派生)
    pub fn date(&self) -> GameDate {
        GameDate::from_hours(self.hour)
    }

    /// 从开局起经过的总天数(用于"N 天后"判定)
    pub fn total_days(&self) -> u64 {
        self.hour / 24
    }
}
```

**保留 hour 不动**: clock.rs 的 `hour += 1` 和 `% 24` 逻辑不改, 现有测试零破坏。GameDate 是纯派生查询, 不存状态。

---

## 4. clock.rs 月切换修正

当前用 `hour % (24*30)`, 在不同月份天数下会错位(2月只28天, 7月31天)。改为比对 tick 前后月份:

```rust
pub fn tick(interp: &Interpreter, world: &mut World) {
    let prev_month = world.date().month;  // tick 前月份
    world.hour += 1;
    world.started = true;
    world.fire_event(interp, "on_hourly");
    // ... 战斗结算 ...
    if world.hour % 24 == 0 {
        world.fire_event(interp, "on_daily");
        world.fire_event(interp, &format!("on_daily_{}", world.player_tag));
        crate::combat::reinforce::reinforce_all(world);
    }
    if world.hour % (24 * 7) == 0 {
        world.fire_event(interp, "on_weekly");
    }
    // 月切换: 比对月份变化(不用 % 30, 因为月份天数不固定)
    if world.date().month != prev_month {
        world.fire_event(interp, "on_monthly");
    }
}
```

**注意**: `prev_month` 在 `hour += 1` 之前取。跨年时 month 从 12→1, 也会触发(正确)。

---

## 5. 后续系统如何使用(预留, 本次不实现)

```rust
// 国策: 完成 = 开工日期 + 70 天
let complete_hour = world.hour + 70 * 24;

// 停战: 180 天后过期
let truce_end_hour = world.hour + 180 * 24;

// 昼夜 darkness: 按日期算季节
let doy = world.date().day_of_year();  // 1..366
let season_progress = (doy as f64) / 366.0;  // 0=年初, 0.5=年中

// 训练: 90 天
if world.hour >= training_start_hour + 90 * 24 { /* 完成 */ }

// 脚本里表达绝对日期
set_date = 1939.1.1   // 剧本切换(后续)
```

---

## 6. 文件组织

```
src/runtime/
├── date.rs       ← 新增: GameDate + from_hours/to_hours/advance_hours/day_of_year + days_in_month + is_leap_year
├── mod.rs        ← 改: 声明 date 模块 + re-export GameDate
├── world.rs      ← 改: 加 date() / total_days() 派生方法
└── clock.rs      ← 改: 月切换改用月份比对
```

### 改动清单

| 文件 | 改动 | 性质 |
|---|---|---|
| `src/runtime/date.rs` | 全新: GameDate + 换算 | 新增 |
| `src/runtime/mod.rs` | 声明 date + re-export | 小改 |
| `src/runtime/world.rs` | 加 date() / total_days() | 小改 |
| `src/runtime/clock.rs` | 月切换改月份比对 | 小改 |

---

## 7. 测试策略

| 测试 | 验证 |
|---|---|
| from_hours 基础 | 0h → 1936.1.1.12; 12h → 1936.1.2.0 |
| 月份进位 | 1月31天后 → 2月1日 |
| 闰年 | 1936.2.28+24h → 1936.2.29(闰); 1937.2.28+24h → 1937.3.1(平) |
| to_hours 反向 | from_hours(N).to_hours() == N (round-trip) |
| day_of_year | 1936.1.1 → 1; 1936.12.31 → 366(闰年) |
| advance_hours | 1936.1.1 + 365*24 → 1936.12.31(闰年366天, 加365天到年末) |
| clock 月切换修正 | 跨真实月份边界时 on_monthly 触发 |
| World.date() 派生 | tick 24次后日期+1天 |

---

## 8. 验收标准

1. `cargo test` 全绿(现有 168 + 新增日期测试)
2. `World::date()` 从 hour 正确派生日期
3. `GameDate::from_hours(0)` = 1936.1.1.12(开局)
4. 闰年正确(1936.2.29 存在, 1937.2.29 不存在)
5. `from_hours` / `to_hours` round-trip 一致
6. clock 月切换按真实月份边界(不再 % 30)
7. **后续系统调 world.date() 而非自己算天数**
