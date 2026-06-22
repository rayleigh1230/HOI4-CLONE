//! 装备增援: 师从国家仓库领装备补满需求(M4a)
//!
//! 每日调用(M4a-3)。简化: 按师 id 顺序补, 不做优先级队列。
//! 借用策略: 先快照各国仓库余量 + 各师缺口, 计算转移量, 再写回。
use crate::runtime::World;

/// 对所有师执行增援(从其所属国家仓库补装备)
pub fn reinforce_all(world: &mut World) {
    // 阶段1: 计算每个师的装备转移需求 (div_id, [(eq_type, transfer_amount)])
    // 同时跟踪各国仓库被消耗的累计量
    use std::collections::HashMap;
    let mut transfers: Vec<(u64, Vec<(String, f64)>)> = Vec::new();
    // 国家仓库余量快照(随转移递减, 模拟"先到的师先领")
    let mut stock_remaining: HashMap<String, f64> = HashMap::new();
    for (tag, country) in &world.countries {
        for (eq, amt) in &country.equipment_stockpile {
            *stock_remaining.entry(format!("{tag}::{eq}")).or_insert(0.0) += amt;
        }
    }

    // 按 div id 顺序处理(确定性)
    let mut div_ids: Vec<u64> = world.divisions.keys().copied().collect();
    div_ids.sort_unstable();
    for did in div_ids {
        let div = match world.divisions.get(&did) {
            Some(d) => d,
            None => continue,
        };
        // 移动中(Moving/Retreating, 含撤退行军)的师不增援(行军中无法补员, 到达后才行)
        if div.is_moving() || div.is_withdrawing() {
            continue;
        }
        let tag = div.owner_tag.clone();
        let mut div_transfer: Vec<(String, f64)> = Vec::new();
        for (eq, need) in &div.equipment_need {
            let held = div.equipment_held.get(eq).copied().unwrap_or(0.0);
            let shortage = (need - held).max(0.0);
            if shortage <= 0.0 {
                continue;
            }
            let key = format!("{tag}::{eq}");
            let available = *stock_remaining.get(&key).unwrap_or(&0.0);
            let transfer = shortage.min(available);
            if transfer > 0.0 {
                *stock_remaining.get_mut(&key).unwrap() -= transfer;
                div_transfer.push((eq.clone(), transfer));
            }
        }
        if !div_transfer.is_empty() {
            transfers.push((did, div_transfer));
        }
    }

    // 阶段2: 写回 — 师加装备, 国家仓库扣
    for (did, div_transfer) in transfers {
        if let Some(div) = world.divisions.get_mut(&did) {
            for (eq, amt) in &div_transfer {
                *div.equipment_held.entry(eq.clone()).or_insert(0.0) += amt;
            }
        }
        let tag = world
            .divisions
            .get(&did)
            .map(|d| d.owner_tag.clone())
            .unwrap_or_default();
        if let Some(country) = world.countries.get_mut(&tag) {
            for (eq, amt) in &div_transfer {
                let cur = country.equipment_stockpile.get(eq).copied().unwrap_or(0.0);
                country.equipment_stockpile.insert(eq.clone(), (cur - amt).max(0.0));
            }
        }
    }

    // 人力增援: 各师从所属国家 manpower_pool 补人力(按 div id 顺序)
    let mut mp_remaining: HashMap<String, f64> = world
        .countries
        .iter()
        .map(|(t, c)| (t.clone(), c.manpower_pool))
        .collect();
    let mut div_ids2: Vec<u64> = world.divisions.keys().copied().collect();
    div_ids2.sort_unstable();
    // 收集 (div_id, transfer_amount)
    let mut mp_transfers: Vec<(u64, f64)> = Vec::new();
    for did in div_ids2 {
        let div = match world.divisions.get(&did) {
            Some(d) => d,
            None => continue,
        };
        if div.is_moving() || div.is_withdrawing() {
            continue; // 移动中不增援
        }
        let shortage = (div.manpower_need - div.manpower_held).max(0.0);
        if shortage <= 0.0 {
            continue;
        }
        let available = *mp_remaining.get(&div.owner_tag).unwrap_or(&0.0);
        let transfer = shortage.min(available);
        if transfer > 0.0 {
            *mp_remaining.get_mut(&div.owner_tag).unwrap() -= transfer;
            mp_transfers.push((did, transfer));
        }
    }
    // 写回人力 + 同步 HP(HP 随人力补员恢复: strength = max_strength × 人力比)
    for (did, amt) in mp_transfers {
        if let Some(div) = world.divisions.get_mut(&did) {
            div.manpower_held += amt;
            div.strength = div.max_strength * div.manpower_ratio();
        }
        let tag = world.divisions.get(&did).map(|d| d.owner_tag.clone()).unwrap_or_default();
        if let Some(country) = world.countries.get_mut(&tag) {
            country.manpower_pool = (country.manpower_pool - amt).max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::entities::{Country, Division};

    fn div_with_eq(tag: &str, held: f64, need: f64) -> Division {
        let mut d = Division {
            id: 0,
            owner_tag: tag.into(),
            ..Default::default()
        };
        d.equipment_need.insert("infantry_equipment".into(), need);
        d.equipment_held.insert("infantry_equipment".into(), held);
        d
    }

    #[test]
    fn t_reinforce_fills_shortage_from_stockpile() {
        let mut w = World::new();
        let mut ger = Country {
            tag: "GER".into(),
            ..Default::default()
        };
        ger.equipment_stockpile.insert("infantry_equipment".into(), 50.0);
        w.countries.insert("GER".into(), ger);
        let did = w.add_division(div_with_eq("GER", 80.0, 100.0)); // 缺 20
        reinforce_all(&mut w);
        let held = w
            .divisions
            .get(&did)
            .unwrap()
            .equipment_held
            .get("infantry_equipment")
            .copied()
            .unwrap_or(0.0);
        assert!((held - 100.0).abs() < 1e-9, "应补满到 100, 实际 {held}");
        // 仓库扣 20
        let stock = w.countries.get("GER").unwrap().equipment_stockpile.get("infantry_equipment").copied().unwrap_or(0.0);
        assert!((stock - 30.0).abs() < 1e-9, "仓库应剩 30, 实际 {stock}");
    }

    #[test]
    fn t_reinforce_partial_when_stockpile_low() {
        let mut w = World::new();
        let mut ger = Country {
            tag: "GER".into(),
            ..Default::default()
        };
        ger.equipment_stockpile.insert("infantry_equipment".into(), 5.0); // 只够补 5
        w.countries.insert("GER".into(), ger);
        let did = w.add_division(div_with_eq("GER", 80.0, 100.0)); // 缺 20
        reinforce_all(&mut w);
        let held = w.divisions.get(&did).unwrap().equipment_held.get("infantry_equipment").copied().unwrap_or(0.0);
        assert!((held - 85.0).abs() < 1e-9, "仓库不足应只补到 85, 实际 {held}");
    }

    #[test]
    fn t_no_transfer_when_full() {
        let mut w = World::new();
        let mut ger = Country {
            tag: "GER".into(),
            ..Default::default()
        };
        ger.equipment_stockpile.insert("infantry_equipment".into(), 50.0);
        w.countries.insert("GER".into(), ger);
        let did = w.add_division(div_with_eq("GER", 100.0, 100.0)); // 已满
        reinforce_all(&mut w);
        let held = w.divisions.get(&did).unwrap().equipment_held.get("infantry_equipment").copied().unwrap_or(0.0);
        assert!((held - 100.0).abs() < 1e-9, "已满不应变");
        let stock = w.countries.get("GER").unwrap().equipment_stockpile.get("infantry_equipment").copied().unwrap_or(0.0);
        assert!((stock - 50.0).abs() < 1e-9, "仓库不应被消耗");
    }
}
