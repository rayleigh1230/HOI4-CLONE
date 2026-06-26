//! 生产系统: 工厂每日产出装备入国家仓库(spec 2026-06-26-production-equipment-design)
//!
//! Country.production_lines 存结构, production_step 写逻辑(后者下个 phase 实现)。
//! 与 runtime/combat 平行模块。

pub mod production;  // 下个 phase 实现, 这里仅声明
#[cfg(test)]
mod tests;

/// 原版常量(取自 common/defines/00_defines.lua 行 598-623)
pub const FACTORY_SPEED_MIL: f64     = 4.5;   // BASE_FACTORY_SPEED_MIL
pub const EFFICIENCY_START: f64      = 0.10;  // BASE_FACTORY_START_EFFICIENCY_FACTOR
pub const EFFICIENCY_MAX: f64        = 0.50;  // BASE_FACTORY_MAX_EFFICIENCY_FACTOR
pub const EFFICIENCY_GAIN: f64       = 1.0;   // BASE_FACTORY_EFFICIENCY_GAIN
pub const EFFICIENCY_BALANCE: f64    = 0.1;   // BASE_FACTORY_EFFICIENCY_BALANCE_FACTOR
pub const VARIANT_RETENTION: f64     = 0.90;  // BASE_FACTORY_EFFICIENCY_VARIANT_CHANGE_FACTOR
pub const RESOURCE_LACK_PENALTY: f64 = 0.05;  // |PRODUCTION_RESOURCE_LACK_PENALTY|
pub const INACTIVE_SLOT_DECAY: f64   = 0.01;  // EFFICIENCY_LOSS_PER_UNUSED_DAY 简化
pub const SLOTS_PER_LINE: usize      = 15;    // 原版硬编码

/// 一个工厂槽位(per-slot 效率, 对齐原版 EFFICIENCY_LOSS_PER_UNUSED_DAY)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FactorySlot {
    pub efficiency: f64,    // 0..EFFICIENCY_MAX
    pub active: bool,
}

/// 一条生产线(对齐原版 production_line, 固定 15 槽位)
#[derive(Debug, Clone)]
pub struct ProductionLine {
    pub id: u32,
    pub chassis: String,        // 从 variant 派生
    pub variant: String,        // 完整 variant 名, 如 "infantry_equipment_1"
    pub slots: Vec<FactorySlot>,
    pub active_count: u32,
}

impl ProductionLine {
    pub fn new(id: u32, variant: String) -> Self {
        let chassis = variant_chassis(&variant).to_string();
        Self {
            id, chassis, variant,
            slots: (0..SLOTS_PER_LINE).map(|_| FactorySlot::default()).collect(),
            active_count: 0,
        }
    }

    /// 激活前 N 个槽位。新激活(之前 inactive 且 eff=0)的槽从 EFFICIENCY_START 起步。
    /// 已激活槽不变。被关闭的槽(之前 active 现 inactive)保留 efficiency 不重置。
    pub fn set_active(&mut self, n: u32) {
        let n = (n as usize).min(SLOTS_PER_LINE);
        for i in 0..SLOTS_PER_LINE {
            let was_active = self.slots[i].active;
            let now_active = i < n;
            self.slots[i].active = now_active;
            if !was_active && now_active && self.slots[i].efficiency == 0.0 {
                self.slots[i].efficiency = EFFICIENCY_START;
            }
        }
        self.active_count = n as u32;
    }
}

/// 从 variant 全名解析 chassis。
/// "infantry_equipment_1" → "infantry_equipment"
/// "light_tank_chassis_1" → "light_tank_chassis"
/// "infantry_equipment"(无 _数字 后缀) → 同名
/// 注意: chassis 名本身可能含下划线(light_tank_chassis), 只剥末尾 _数字
pub fn variant_chassis(variant: &str) -> &str {
    if let Some(pos) = variant.rfind('_') {
        let suffix = &variant[pos + 1..];
        if !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()) {
            return &variant[..pos];
        }
    }
    variant
}
