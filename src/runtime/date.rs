//! 日期系统: GameDate 精确公历 + 双向换算
//!
//! 对齐原版 START_DATE = "1936.1.1.12"。保留 World.hour: u64 不动,
//! GameDate 作为派生查询层(现有 clock/测试零破坏)。
//! 设计见 docs/superpowers/specs/2026-06-25-date-system-design.md

/// 游戏日期(年.月.日.时), 精确公历
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameDate {
    pub year: u32,  // 1936..
    pub month: u32, // 1..12
    pub day: u32,   // 1..31(按月)
    pub hour: u32,  // 0..23
}

impl GameDate {
    /// 开局日期(原版 START_DATE, 1936年1月1日12时)
    pub const START: GameDate = GameDate { year: 1936, month: 1, day: 1, hour: 12 };

    /// 从开局起经过 total_hours 小时后的日期
    pub fn from_hours(total_hours: u64) -> GameDate {
        let mut d = Self::START;
        let new_hour = d.hour as u64 + total_hours;
        let mut remaining_days = new_hour / 24;
        d.hour = (new_hour % 24) as u32;
        while remaining_days > 0 {
            let dim = days_in_month(d.year, d.month) as u64;
            if d.day as u64 - 1 + remaining_days < dim {
                d.day += remaining_days as u32;
                remaining_days = 0;
            } else {
                remaining_days -= dim - (d.day as u64 - 1);
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
        // 算 self 与 START 之间相差多少完整天 + 当天小时差
        // 用"序数日"法: 把日期转成从某基准起的绝对天数, 再算差
        let self_abs_days = date_to_abs_days(self.year, self.month, self.day);
        let start_abs_days = date_to_abs_days(Self::START.year, Self::START.month, Self::START.day);
        let day_diff = (self_abs_days - start_abs_days) as u64;
        // 小时差: self.hour 相对 START.hour 的偏移(可能为负, 用天补偿)
        let hour_diff = self.hour as i64 - Self::START.hour as i64;
        (day_diff as i64 * 24 + hour_diff) as u64
    }

    /// 推进 n 小时
    pub fn advance_hours(&self, hours: u64) -> GameDate {
        Self::from_hours(self.to_hours() + hours)
    }

    /// 该日期是一年中的第几天(1..365/366, 用于昼夜 darkness 计算)
    pub fn day_of_year(&self) -> u32 {
        let months_cumulative: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
        let mut doy = months_cumulative[(self.month - 1) as usize] + self.day;
        if is_leap_year(self.year) && self.month > 2 {
            doy += 1;
        }
        doy
    }
}

/// 某年某月的天数
pub fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if is_leap_year(year) { 29 } else { 28 },
        _ => 30,
    }
}

/// 闰年判定: (能被4整除且不能被100整除) 或 能被400整除
pub fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// 把日期转成绝对天数序数(从公元1年1月1日起)
/// 用于计算两个日期之间的天数差(公历标准算法)
fn date_to_abs_days(year: u32, month: u32, day: u32) -> i64 {
    // 用 Howard Hinnant 算法: days_from_civil
    let y = if month <= 2 { year as i64 - 1 } else { year as i64 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;  // [0, 399]
    let m = month as u64;
    let d = day as u64;
    let doy = (153 * if m > 2 { m - 3 } else { m + 9 } + 2) / 5 + d - 1;  // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;  // [0, 146096]
    era * 146097 + doe as i64 - 719468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_from_hours_zero_is_start() {
        let d = GameDate::from_hours(0);
        assert_eq!(d, GameDate::START);
        assert_eq!((d.year, d.month, d.day, d.hour), (1936, 1, 1, 12));
    }

    #[test]
    fn t_from_hours_12h_crosses_to_next_day_midnight() {
        // START 12:00 + 12h = 次日 0:00
        let d = GameDate::from_hours(12);
        assert_eq!((d.year, d.month, d.day, d.hour), (1936, 1, 2, 0));
    }

    #[test]
    fn t_month_rollover_31_days() {
        // 1月1日 + 30天完整天 = 1月31日; +31天 = 2月1日
        let d = GameDate::from_hours(31 * 24); // 30天完整 + 1天到2月1日(从12h起算, 31*24=744h)
        // START是1月1日12时, +744h: 744/24=31天完整 → 2月1日, 余0h → 12时
        assert_eq!((d.month, d.day), (2, 1), "1月有31天, +31天到2月1日");
    }

    #[test]
    fn t_leap_year_1936() {
        // 1936是闰年: 2月有29天
        // 从2月28日推进1天应到2月29日(不是3月1日)
        let feb28 = GameDate { year: 1936, month: 2, day: 28, hour: 12 };
        let next = feb28.advance_hours(24);
        assert_eq!((next.month, next.day), (2, 29), "1936闰年2月29日应存在");
    }

    #[test]
    fn t_non_leap_year_1937() {
        // 1937不是闰年: 2月28日 + 1天 = 3月1日
        let feb28 = GameDate { year: 1937, month: 2, day: 28, hour: 12 };
        let next = feb28.advance_hours(24);
        assert_eq!((next.month, next.day), (3, 1), "1937平年2月只有28天");
    }

    #[test]
    fn t_round_trip() {
        for n in [0u64, 1, 12, 24, 100, 365 * 24, 366 * 24, 1000, 4 * 365 * 24] {
            let d = GameDate::from_hours(n);
            let back = d.to_hours();
            assert_eq!(back, n, "round-trip 失败: from_hours({n}) → {:?} → to_hours()={back}", d);
        }
    }

    #[test]
    fn t_day_of_year() {
        let jan1 = GameDate { year: 1936, month: 1, day: 1, hour: 0 };
        assert_eq!(jan1.day_of_year(), 1);
        let dec31 = GameDate { year: 1936, month: 12, day: 31, hour: 0 };
        assert_eq!(dec31.day_of_year(), 366, "1936闰年12月31日是第366天");
        let dec31_1937 = GameDate { year: 1937, month: 12, day: 31, hour: 0 };
        assert_eq!(dec31_1937.day_of_year(), 365, "1937平年12月31日是第365天");
    }

    #[test]
    fn t_advance_one_year_leap() {
        // 1936是闰年(366天), 从1月1日推进365天 → 12月31日
        let d = GameDate { year: 1936, month: 1, day: 1, hour: 12 };
        let next = d.advance_hours(365 * 24);
        assert_eq!((next.year, next.month, next.day), (1936, 12, 31), "闰年推进365天到年末");
    }

    #[test]
    fn t_is_leap_year() {
        assert!(is_leap_year(1936));  // 能被4整除, 不能被100
        assert!(!is_leap_year(1937));
        assert!(!is_leap_year(1900)); // 能被100整除但不能被400
        assert!(is_leap_year(2000));  // 能被400整除
    }

    #[test]
    fn t_days_in_month() {
        assert_eq!(days_in_month(1936, 2), 29);
        assert_eq!(days_in_month(1937, 2), 28);
        assert_eq!(days_in_month(1936, 1), 31);
        assert_eq!(days_in_month(1936, 4), 30);
    }
}
