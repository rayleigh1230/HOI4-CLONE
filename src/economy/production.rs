//! 每日生产循环: 工厂每日按生产线产出装备入国家仓库
//!
//! 触发: clock.rs 每日(on_daily 后, reinforce_all 前)。
//! 三阶段(快照→计算→写回)避借用冲突, 沿用 reinforce.rs 风格。

use crate::data::equipment::EquipmentDef as GameEquipmentDef;
use crate::runtime::World;
use std::collections::HashMap;

/// 每日生产: 所有国家的所有生产线产出装备 + 更新 slot 效率
pub fn production_step(world: &mut World) {
    // 阶段 1: 快照各国可用资源(Σ owned_states 的 State.resources)
    let mut country_resources: HashMap<String, HashMap<String, f64>> = HashMap::new();
    for (tag, country) in &world.countries {
        let mut total: HashMap<String, f64> = HashMap::new();
        for sid in &country.owned_states {
            if let Some(state) = world.states.get(sid) {
                for (r, v) in &state.resources {
                    *total.entry(r.clone()).or_insert(0.0) += v;
                }
            }
        }
        country_resources.insert(tag.clone(), total);
    }

    // 阶段 2: 每条 line 计算产出 + 收集 slot efficiency 更新
    let mut outputs: Vec<(String, String, f64)> = Vec::new(); // (tag, variant, amount)
    let mut slot_updates: Vec<(String, u32, Vec<(usize, f64)>)> = Vec::new();

    for (tag, country) in &world.countries {
        let res_avail = country_resources.get(tag).cloned().unwrap_or_default();
        for line in &country.production_lines {
            // 找装备定义(数据驱动层)
            let equipment = match world.data.equipment.get(&line.variant) {
                Some(e) => e,
                None => continue,
            };
            let res_mult = resource_penalty(line, equipment, &res_avail);

            let mut total_output = 0.0;
            let mut new_effs: Vec<(usize, f64)> = Vec::new();
            for (i, slot) in line.slots.iter().enumerate() {
                if !slot.active {
                    // inactive 槽衰减: -INACTIVE_SLOT_DECAY/日
                    if slot.efficiency > 0.0 {
                        let new_e = (slot.efficiency - super::INACTIVE_SLOT_DECAY).max(0.0);
                        new_effs.push((i, new_e));
                    }
                    continue;
                }
                // 单 slot 日产出 = FACTORY_SPEED_MIL × efficiency × res_mult / build_cost_ic
                let bc = equipment.stats.build_cost_ic.max(0.0001); // 防 0 除
                let out = super::FACTORY_SPEED_MIL * slot.efficiency * res_mult / bc;
                total_output += out;
                // 效率增长: eff += (MAX - eff) × GAIN × BALANCE
                let new_e = slot.efficiency
                    + (super::EFFICIENCY_MAX - slot.efficiency) * super::EFFICIENCY_GAIN * super::EFFICIENCY_BALANCE;
                new_effs.push((i, new_e));
            }
            if total_output > 0.0 {
                outputs.push((tag.clone(), line.variant.clone(), total_output));
            }
            if !new_effs.is_empty() {
                slot_updates.push((tag.clone(), line.id, new_effs));
            }
        }
    }

    // 阶段 3: 写回 stockpile + slot efficiency
    for (tag, variant, amt) in outputs {
        if let Some(country) = world.countries.get_mut(&tag) {
            *country
                .equipment_stockpile
                .entry(variant)
                .or_insert(0.0) += amt;
        }
    }
    for (tag, line_id, updates) in slot_updates {
        if let Some(country) = world.countries.get_mut(&tag) {
            if let Some(line) = country
                .production_lines
                .iter_mut()
                .find(|l| l.id == line_id)
            {
                for (i, e) in updates {
                    line.slots[i].efficiency = e;
                }
            }
        }
    }
}

/// 资源惩罚(严格 -5%/工厂/单位, 原版 PRODUCTION_RESOURCE_LACK_PENALTY)
/// 每缺 1 单位资源 → 该 line 产出 -5%, 多资源类型累加
/// 返回值: 产出系数 [0, 1]
pub fn resource_penalty(
    line: &super::ProductionLine,
    equipment: &GameEquipmentDef,
    country_res: &HashMap<String, f64>,
) -> f64 {
    let mut penalty: f64 = 0.0;
    for (resource, need_per_factory) in &equipment.resources {
        let total_need = line.active_count as f64 * need_per_factory;
        let available = country_res.get(resource).copied().unwrap_or(0.0);
        let shortage = (total_need - available).max(0.0);
        penalty += shortage * super::RESOURCE_LACK_PENALTY;
    }
    (1.0 - penalty).max(0.0)
}

/// 切换生产线型号(严格原版保留率)
/// - 不同 chassis: 全槽重置到 EFFICIENCY_START(active)/ 0(inactive)
/// - 同 chassis 不同 variant: 每 slot efficiency × VARIANT_RETENTION(0.9)
/// - 同 variant: 无操作
pub fn change_line_variant(line: &mut super::ProductionLine, new_variant: &str) {
    let new_chassis = super::variant_chassis(new_variant);
    let same_chassis = new_chassis == line.chassis;
    let same_variant = new_variant == line.variant;
    if same_variant {
        return; // 无变化
    }
    let retention = if same_chassis {
        super::VARIANT_RETENTION // 0.9
    } else {
        0.0 // 重置
    };
    for slot in &mut line.slots {
        if retention == 0.0 {
            slot.efficiency = if slot.active {
                super::EFFICIENCY_START
            } else {
                0.0
            };
        } else {
            slot.efficiency *= retention;
        }
    }
    line.chassis = new_chassis.to_string();
    line.variant = new_variant.to_string();
}
