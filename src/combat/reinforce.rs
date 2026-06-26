//! 装备增援: 师从国家仓库领装备补满需求(M4a)
//!
//! 每日调用(M4a-3)。简化: 按师 id 顺序补, 不做优先级队列。
//! 借用策略: 先快照各国仓库余量 + 各师缺口, 计算转移量, 再写回。
//!
//! 原版语义: 需求按 chassis(如 "infantry_equipment"), 持有/库存按 variant 全名
//! (如 "infantry_equipment_1")。增援时按 chassis 在 variant 池里查, 优先取最新
//! variant(字母倒序: variant_2 先于 variant_1)。
use crate::economy::variant_chassis;
use crate::runtime::World;

/// 对所有师执行增援(从其所属国家仓库补装备)
pub fn reinforce_all(world: &mut World) {
    // 阶段1: 计算每个师的装备转移需求 (div_id, [(variant_key, transfer_amount)])
    // 同时跟踪各国仓库被消耗的累计量(快照, 随转移递减, 模拟"先到的师先领")
    use std::collections::HashMap;
    let mut transfers: Vec<(u64, Vec<(String, f64)>)> = Vec::new();
    let mut stock_remaining: HashMap<String, f64> = HashMap::new();
    for (tag, country) in &world.countries {
        for (variant, amt) in &country.equipment_stockpile {
            *stock_remaining.entry(format!("{tag}::{variant}")).or_insert(0.0) += amt;
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
        let prefix = format!("{tag}::");
        // need 按 chassis, held 按 variant; 缺口时按 chassis 在 stockpile 找 variant 池
        for (chassis, need) in &div.equipment_need {
            // 该 chassis 当前持有总量(所有 variant 之和)
            let held_total: f64 = div
                .equipment_held
                .iter()
                .filter(|(k, _)| variant_chassis(k) == chassis.as_str())
                .map(|(_, v)| v)
                .sum();
            let shortage = (need - held_total).max(0.0);
            if shortage <= 0.0 {
                continue;
            }
            // 在快照里找该国家、该 chassis 的所有 variant(按字母倒序优先取最新)
            let mut candidates: Vec<String> = stock_remaining
                .keys()
                .filter(|k| {
                    // key 形如 "TAG::variant"
                    k.starts_with(&prefix)
                        && k.split("::").nth(1)
                            .map(|v| variant_chassis(v) == chassis.as_str())
                            .unwrap_or(false)
                })
                .cloned()
                .collect();
            candidates.sort_by(|a, b| b.cmp(a));
            let mut remaining = shortage;
            for key in candidates {
                if remaining <= 0.0 {
                    break;
                }
                let avail = *stock_remaining.get(&key).unwrap_or(&0.0);
                if avail <= 0.0 {
                    continue;
                }
                let take = remaining.min(avail);
                *stock_remaining.get_mut(&key).unwrap() -= take;
                // 取 variant 名(去掉 "TAG::" 前缀)写回师持有
                let variant_key = key.split("::").nth(1).unwrap_or(&key).to_string();
                div_transfer.push((variant_key, take));
                remaining -= take;
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
        // need 用 chassis 名
        d.equipment_need.insert("infantry_equipment".into(), need);
        // held 用 variant 全名(原版语义: 持有按变体)
        d.equipment_held.insert("infantry_equipment_1".into(), held);
        d
    }

    #[test]
    fn t_reinforce_fills_shortage_from_stockpile() {
        let mut w = World::new();
        let mut ger = Country {
            tag: "GER".into(),
            ..Default::default()
        };
        ger.equipment_stockpile.insert("infantry_equipment_1".into(), 50.0);
        w.countries.insert("GER".into(), ger);
        let did = w.add_division(div_with_eq("GER", 80.0, 100.0)); // 缺 20
        reinforce_all(&mut w);
        let held = w
            .divisions
            .get(&did)
            .unwrap()
            .equipment_held
            .get("infantry_equipment_1")
            .copied()
            .unwrap_or(0.0);
        assert!((held - 100.0).abs() < 1e-9, "应补满到 100, 实际 {held}");
        // 仓库扣 20
        let stock = w.countries.get("GER").unwrap().equipment_stockpile.get("infantry_equipment_1").copied().unwrap_or(0.0);
        assert!((stock - 30.0).abs() < 1e-9, "仓库应剩 30, 实际 {stock}");
    }

    #[test]
    fn t_reinforce_partial_when_stockpile_low() {
        let mut w = World::new();
        let mut ger = Country {
            tag: "GER".into(),
            ..Default::default()
        };
        ger.equipment_stockpile.insert("infantry_equipment_1".into(), 5.0); // 只够补 5
        w.countries.insert("GER".into(), ger);
        let did = w.add_division(div_with_eq("GER", 80.0, 100.0)); // 缺 20
        reinforce_all(&mut w);
        let held = w.divisions.get(&did).unwrap().equipment_held.get("infantry_equipment_1").copied().unwrap_or(0.0);
        assert!((held - 85.0).abs() < 1e-9, "仓库不足应只补到 85, 实际 {held}");
    }

    #[test]
    fn t_no_transfer_when_full() {
        let mut w = World::new();
        let mut ger = Country {
            tag: "GER".into(),
            ..Default::default()
        };
        ger.equipment_stockpile.insert("infantry_equipment_1".into(), 50.0);
        w.countries.insert("GER".into(), ger);
        let did = w.add_division(div_with_eq("GER", 100.0, 100.0)); // 已满
        reinforce_all(&mut w);
        let held = w.divisions.get(&did).unwrap().equipment_held.get("infantry_equipment_1").copied().unwrap_or(0.0);
        assert!((held - 100.0).abs() < 1e-9, "已满不应变");
        let stock = w.countries.get("GER").unwrap().equipment_stockpile.get("infantry_equipment_1").copied().unwrap_or(0.0);
        assert!((stock - 50.0).abs() < 1e-9, "仓库不应被消耗");
    }

    #[test]
    fn t_reinforce_prefers_newer_variant() {
        let mut w = World::new();
        let mut ger = Country { tag: "GER".into(), ..Default::default() };
        ger.equipment_stockpile.insert("infantry_equipment_1".into(), 30.0);
        ger.equipment_stockpile.insert("infantry_equipment_2".into(), 30.0);
        w.countries.insert("GER".into(), ger);
        let did = w.add_division(div_with_eq("GER", 80.0, 100.0)); // 缺 20

        reinforce_all(&mut w);

        let v2 = w.divisions.get(&did).unwrap().equipment_held.get("infantry_equipment_2").copied().unwrap_or(0.0);
        let v1 = w.divisions.get(&did).unwrap().equipment_held.get("infantry_equipment_1").copied().unwrap_or(0.0);
        // 缺 20, 应优先从 v2 取(v2 stockpile 减 20, v1 不动); v1 held 保持种子值 80
        assert!((v2 - 20.0).abs() < 1e-9, "应优先补 v2, 实际 v2={}", v2);
        assert!((v1 - 80.0).abs() < 1e-9, "v1 held 不应有新增(保持种子 80), 实际 v1={}", v1);
        let v1_stock = w.countries.get("GER").unwrap().equipment_stockpile.get("infantry_equipment_1").copied().unwrap_or(0.0);
        assert!((v1_stock - 30.0).abs() < 1e-9, "v1 不应被动, stock={}", v1_stock);
    }

    #[test]
    fn t_reinforce_mixed_variants_fill_chassis_need() {
        let mut w = World::new();
        let mut ger = Country { tag: "GER".into(), ..Default::default() };
        ger.equipment_stockpile.insert("infantry_equipment_1".into(), 5.0);   // v1 只够补 5
        ger.equipment_stockpile.insert("infantry_equipment_2".into(), 30.0);  // v2 充足
        w.countries.insert("GER".into(), ger);
        let did = w.add_division(div_with_eq("GER", 80.0, 100.0)); // 缺 20

        reinforce_all(&mut w);

        // 倒序优先取 v2, 取完 20(库存够); v1 held 保持种子值 80(未被取)
        let v2 = w.divisions.get(&did).unwrap().equipment_held.get("infantry_equipment_2").copied().unwrap_or(0.0);
        assert!((v2 - 20.0).abs() < 1e-9, "v2 应补 20, 实际 {}", v2);
        let v1 = w.divisions.get(&did).unwrap().equipment_held.get("infantry_equipment_1").copied().unwrap_or(0.0);
        assert!((v1 - 80.0).abs() < 1e-9, "v1 不应被取(因 v2 够, 保持种子 80), 实际 {}", v1);
    }
}
