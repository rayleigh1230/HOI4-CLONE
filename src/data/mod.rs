//! 数据驱动层: 原版数据文件 → 只读 GameData 定义表
//!
//! 与 runtime 平行(parser 的第二个消费者)。
//! loader 把 Block 当"数据定义"读, runtime::interp 把 Block 当"命令"执行。
//! GameData 启动加载一次, 运行时只读不改。

pub mod equipment;
pub mod loader;
pub mod subunit;
pub mod template;

use crate::data::equipment::{ChassisDef, EquipmentDef, ModuleDef};
use crate::data::subunit::SubUnitDef;
use crate::data::template::DivisionTemplate;
use std::collections::HashMap;

/// 装备属性集合(战斗相关字段, 从 add_stats/multiply_stats 提取)
/// 贯穿装备/营/师三层: 装备算出 → 营汇总 → 师汇总
#[derive(Debug, Clone, Default)]
pub struct EquipStats {
    pub soft_attack: f64,
    pub hard_attack: f64,
    pub defense: f64,
    pub breakthrough: f64,
    pub armor: f64,        // 原版 armor_value
    pub piercing: f64,     // 原版 ap_attack
    pub hardness: f64,
    pub build_cost_ic: f64,
    pub maximum_speed: f64,
    pub reliability: f64,
}

impl EquipStats {
    /// 加法合并(把 other 的各字段加到 self) — 用于 Σ add_stats
    pub fn add(&mut self, other: &EquipStats) {
        self.soft_attack += other.soft_attack;
        self.hard_attack += other.hard_attack;
        self.defense += other.defense;
        self.breakthrough += other.breakthrough;
        self.armor += other.armor;
        self.piercing += other.piercing;
        self.hardness += other.hardness;
        self.build_cost_ic += other.build_cost_ic;
        self.maximum_speed += other.maximum_speed;
        self.reliability += other.reliability;
    }

    /// 乘法修正 — 用于 Π (1 + multiply_stats)
    /// 对每个字段: self[field] *= 1.0 + other[field]
    pub fn multiply(&mut self, other: &EquipStats) {
        self.soft_attack *= 1.0 + other.soft_attack;
        self.hard_attack *= 1.0 + other.hard_attack;
        self.defense *= 1.0 + other.defense;
        self.breakthrough *= 1.0 + other.breakthrough;
        self.armor *= 1.0 + other.armor;
        self.piercing *= 1.0 + other.piercing;
        self.hardness *= 1.0 + other.hardness;
        self.build_cost_ic *= 1.0 + other.build_cost_ic;
        self.maximum_speed *= 1.0 + other.maximum_speed;
        self.reliability *= 1.0 + other.reliability;
    }
}

/// 只读静态定义数据库(启动加载, 运行时不改)
#[derive(Debug, Clone, Default)]
pub struct GameData {
    pub modules: HashMap<String, ModuleDef>,
    pub chassis: HashMap<String, ChassisDef>,
    pub equipment: HashMap<String, EquipmentDef>,   // 可生产装备
    pub sub_units: HashMap<String, SubUnitDef>,     // 营定义
    pub templates: HashMap<String, DivisionTemplate>, // 师模板
    pub start_year: u32,
}

use std::sync::OnceLock;

/// 进程级 GameData 缓存(只加载一次, 所有 World 共享)
/// OnceLock 是 std 稳定 API(1.70+), 零外部依赖
static GAME_DATA: OnceLock<GameData> = OnceLock::new();

/// 取共享只读 GameData(Arc 包裹)
pub fn cached_game_data() -> std::sync::Arc<GameData> {
    std::sync::Arc::new(GAME_DATA.get_or_init(|| crate::data::loader::load_all()).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_equipstats_add() {
        let mut a = EquipStats { soft_attack: 10.0, defense: 20.0, ..Default::default() };
        let b = EquipStats { soft_attack: 5.0, defense: 30.0, ..Default::default() };
        a.add(&b);
        assert!((a.soft_attack - 15.0).abs() < 1e-9);
        assert!((a.defense - 50.0).abs() < 1e-9);
    }

    #[test]
    fn t_equipstats_multiply() {
        // soft 10, multiply +0.3 → 10 × 1.3 = 13
        let mut a = EquipStats { soft_attack: 10.0, ..Default::default() };
        let m = EquipStats { soft_attack: 0.3, ..Default::default() };
        a.multiply(&m);
        assert!((a.soft_attack - 13.0).abs() < 1e-9);
    }

    #[test]
    fn t_equipstats_add_then_multiply_matches_formula() {
        // 验证 spec §3.3 公式: raw = base + Σ add; final = raw × Π(1+mult)
        // base soft=10, add +5 → raw=15; mult +0.2 → 15×1.2=18
        let mut a = EquipStats { soft_attack: 10.0, ..Default::default() };
        a.add(&EquipStats { soft_attack: 5.0, ..Default::default() });
        a.multiply(&EquipStats { soft_attack: 0.2, ..Default::default() });
        assert!((a.soft_attack - 18.0).abs() < 1e-9);
    }

    #[test]
    fn t_gamedata_default_empty() {
        let d = GameData::default();
        assert!(d.equipment.is_empty());
        assert_eq!(d.start_year, 0);
    }
}
