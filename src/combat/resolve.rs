//! 陆战结算(公式见 docs/formulas/land-combat.md)
//!
//! 安全借用策略: 攻方属性拷贝成只读 AtkStats, 守方可变借用写伤害。避免 unsafe。
use crate::runtime::entities::Division;
use crate::runtime::World;

/// 全局系数(对应 NMilitary defines, 见 docs/formulas/land-combat.md)
const ORG_DICE_SIZE: f64 = 4.0;
const STR_DICE_SIZE: f64 = 2.0;
const ORG_DMG_MOD: f64 = 0.053;
const STR_DMG_MOD: f64 = 0.060;
const HIT_CHANCE_DEF_LEFT: f64 = 0.10; // 防御池未空
const HIT_CHANCE_NO_DEF: f64 = 0.40; // 防御池耗尽
const ARMOR_ORG_BONUS_DICE: f64 = 6.0; // 装甲碾压额外组织度骰
const ARMOR_STR_BONUS_DICE: f64 = 2.0;
const DAMAGE_SPLIT_FIRST: f64 = 0.35; // 首要目标分摊
const EQUIPMENT_LOSS_FACTOR: f64 = 0.70; // HP损→装备损(原版 defines)

/// 攻方只读快照(避免与守方可变借用冲突)
#[derive(Clone, Copy)]
struct AtkStats {
    soft_attack: f64,
    hard_attack: f64,
    armor: f64,
    piercing: f64,
}

impl AtkStats {
    fn from(d: &Division) -> Self {
        // M4a: 攻击属性按装备充足度缩放(缺装备→攻击下降)
        Self {
            soft_attack: d.effective_soft_attack(),
            hard_attack: d.effective_hard_attack(),
            armor: d.armor,
            piercing: d.piercing,
        }
    }
}

/// 对一组攻击者 vs 一组防御者结算 1 小时(仅正向; 反击由 resolve_all_battles 对称处理)
/// 守方为可变引用切片, 兼容 HashMap::get_mut 收集的 Vec<&mut Division>
pub fn resolve_hour(attackers: &[Division], defenders: &mut [&mut Division]) {
    if attackers.is_empty() || defenders.is_empty() {
        return;
    }
    // 正向: 攻方 → 守方(守方用 defense 池; P1-5 所有攻击者共享消耗)
    let atk_stats: Vec<AtkStats> = attackers.iter().map(AtkStats::from).collect();
    apply_all_attackers(&atk_stats, defenders, CombatPool::Defense);
}

/// 哪一方的防御池: 守方用 defense, 攻方(被反击时)用 breakthrough
#[derive(Clone, Copy)]
enum CombatPool {
    Defense,
    Breakthrough,
}

impl CombatPool {
    fn pool_value(self, d: &Division) -> f64 {
        // M4a: 防御池也按装备充足度缩放
        match self {
            CombatPool::Defense => d.effective_defense(),
            CombatPool::Breakthrough => d.effective_breakthrough(),
        }
    }
}

/// 所有攻击方对一组目标输出伤害(P1-5: 防御池对所有攻击者共享消耗)
/// 每个目标的 defense/breakthrough 池被所有攻击者的攻击累加消耗。
fn apply_all_attackers(attackers: &[AtkStats], targets: &mut [&mut Division], pool: CombatPool) {
    let n = targets.len();
    if n == 0 || attackers.is_empty() {
        return;
    }
    let target_hardness = targets[0].hardness;

    for (i, tgt) in targets.iter_mut().enumerate() {
        // P0-1: 每个攻击者对目标的分摊(首要35% + 均分65%)
        let base = (1.0 - DAMAGE_SPLIT_FIRST) / n as f64;
        let share = if i == 0 { DAMAGE_SPLIT_FIRST + base } else { base };

        // 聚合所有攻击者对本目标的攻击点
        let mut total_attacks = 0.0f64;
        // 每攻击者的贡献(用于按比例分摊命中, 保留各自装甲判定)
        let mut per_atk: Vec<(f64, bool, bool)> = Vec::new(); // (attacks, armor_outclass, def_outclass)
        for atk in attackers {
            let atk_pts = atk.soft_attack * (1.0 - target_hardness) + atk.hard_attack * target_hardness;
            let on_this = atk_pts * share;
            if on_this <= 0.0 {
                continue;
            }
            total_attacks += on_this;
            per_atk.push((on_this, atk.armor > tgt.piercing, tgt.armor > atk.piercing));
        }
        if total_attacks <= 0.0 {
            continue;
        }

        // P1-5: 用目标防御池一次判定总命中(所有攻击共享消耗)
        let total_hits = compute_hits(total_attacks, pool.pool_value(tgt));

        // 按攻击点比例把命中分给各攻击者, 各自算伤害(含装甲碾压骰子)
        for (atk_pts, armor_outclass, def_outclass) in per_atk {
            let hits = total_hits * (atk_pts / total_attacks);
            let mut org_dice = ORG_DICE_SIZE;
            let mut str_dice = STR_DICE_SIZE;
            if armor_outclass {
                org_dice += ARMOR_ORG_BONUS_DICE;
                str_dice += ARMOR_STR_BONUS_DICE;
            }
            let armor_deflect = if def_outclass { 0.5 } else { 1.0 };

            let org_dmg = hits * ((org_dice + 1.0) / 2.0) * ORG_DMG_MOD * armor_deflect;
            let str_dmg = hits * ((str_dice + 1.0) / 2.0) * STR_DMG_MOD * armor_deflect;

            tgt.org = (tgt.org - org_dmg).max(0.0);
            let hp_before = tgt.strength;
            tgt.strength = (tgt.strength - str_dmg).max(0.0);
            let hp_loss = hp_before - tgt.strength;
            if hp_loss > 0.0 {
                consume_losses(tgt, hp_loss);
            }
        }
    }
}

/// 计算命中数(防御池机制)
fn compute_hits(attacks: f64, def_pool: f64) -> f64 {
    let defended = attacks.min(def_pool);
    let undefended = (attacks - def_pool).max(0.0);
    defended * HIT_CHANCE_DEF_LEFT + undefended * HIT_CHANCE_NO_DEF
}

/// 按 HP 损失量扣装备和人力(四量模型)
/// 装备: hp_loss × EQUIPMENT_LOSS_FACTOR(0.70), 各类型按持有比例分摊
/// 人力: hp_loss × (manpower_need / max_strength), 即 1 HP 对应的兵员数
fn consume_losses(div: &mut Division, hp_loss: f64) {
    if hp_loss <= 0.0 {
        return;
    }
    // 装备消耗
    let eq_loss = hp_loss * EQUIPMENT_LOSS_FACTOR;
    let total_held: f64 = div.equipment_held.values().sum();
    if total_held > 0.0 {
        let types: Vec<String> = div.equipment_held.keys().cloned().collect();
        for eq_type in types {
            let held = *div.equipment_held.get(&eq_type).unwrap_or(&0.0);
            let share = held / total_held;
            let loss = (eq_loss * share).min(held);
            *div.equipment_held.get_mut(&eq_type).unwrap() = held - loss;
        }
    }
    // 人力消耗: 1 HP = (manpower_need / max_strength) 人
    let mp_per_hp = if div.max_strength > 0.0 {
        div.manpower_need / div.max_strength
    } else {
        0.0
    };
    let mp_loss = (hp_loss * mp_per_hp).min(div.manpower_held);
    div.manpower_held -= mp_loss;
}

/// World 级战斗结算: 遍历所有 battle, 每小时调用
/// 三阶段: ① 快照攻守; ② 对称结算(攻→守 defense 池 + 守→攻 breakthrough 池);
///         ③ 按 id 累积伤害 delta 写回(同一师参与多场战斗, 伤害累加而非覆盖 → 修 P1-6)。
/// 安全借用: 每场战斗本地克隆师副本, 算 (before-after) delta, 最后一次性写回 world。无 unsafe。
pub fn resolve_all_battles(world: &mut World) {
    let battle_specs: Vec<(Vec<u64>, Vec<u64>)> = world
        .battles
        .iter()
        .map(|b| (b.attackers.clone(), b.defenders.clone()))
        .collect();

    // P1-6 修复: 累积每个师的伤害 delta(同一师可能在多场战斗, 伤害应相加而非覆盖)
    // before 快照来自 world.divisions(每场战斗重新取, 反映"这场战斗开始时"的状态)
    use std::collections::HashMap;
    #[derive(Default)]
    struct DamageDelta {
        org_loss: f64,
        str_loss: f64,
        mp_loss: f64,
        // 装备消耗: eq_type → 累积消耗量
        eq_loss: HashMap<String, f64>,
    }
    let mut deltas: HashMap<u64, DamageDelta> = HashMap::new();

    for (atk_ids, def_ids) in &battle_specs {
        // before 快照(算本场 delta 用)
        let atk_before: HashMap<u64, Division> =
            atk_ids.iter().filter_map(|id| world.divisions.get(id).map(|d| (*id, d.clone()))).collect();
        let def_before: HashMap<u64, Division> =
            def_ids.iter().filter_map(|id| world.divisions.get(id).map(|d| (*id, d.clone()))).collect();

        let mut atks: Vec<Division> =
            atk_ids.iter().filter_map(|id| world.divisions.get(id).cloned()).collect();
        let mut defs: Vec<Division> =
            def_ids.iter().filter_map(|id| world.divisions.get(id).cloned()).collect();
        if atks.is_empty() || defs.is_empty() {
            continue;
        }

        // 正向: 攻 → 守(守用 defense 池; P1-5 所有攻击者共享消耗)
        {
            let atk_stats: Vec<AtkStats> = atks.iter().map(AtkStats::from).collect();
            let mut def_refs: Vec<&mut Division> = defs.iter_mut().collect();
            apply_all_attackers(&atk_stats, &mut def_refs, CombatPool::Defense);
        }
        // 反向(反击): 守 → 攻(攻用 breakthrough 池)
        {
            let def_stats: Vec<AtkStats> = defs.iter().map(AtkStats::from).collect();
            let mut atk_refs: Vec<&mut Division> = atks.iter_mut().collect();
            apply_all_attackers(&def_stats, &mut atk_refs, CombatPool::Breakthrough);
        }

        // 累积本场 delta 到 deltas(而非覆盖 final_state)
        for (i, id) in atk_ids.iter().enumerate() {
            let Some(before) = atk_before.get(id) else { continue };
            let Some(after) = atks.get(i) else { continue };
            let d = deltas.entry(*id).or_default();
            d.org_loss += before.org - after.org;
            d.str_loss += before.strength - after.strength;
            d.mp_loss += before.manpower_held - after.manpower_held;
            accumulate_eq_loss(&mut d.eq_loss, &before.equipment_held, &after.equipment_held);
        }
        for (i, id) in def_ids.iter().enumerate() {
            let Some(before) = def_before.get(id) else { continue };
            let Some(after) = defs.get(i) else { continue };
            let d = deltas.entry(*id).or_default();
            d.org_loss += before.org - after.org;
            d.str_loss += before.strength - after.strength;
            d.mp_loss += before.manpower_held - after.manpower_held;
            accumulate_eq_loss(&mut d.eq_loss, &before.equipment_held, &after.equipment_held);
        }
    }

    // 写回: 从原始 world 值减去累积 delta(避免任何顺序依赖)
    for (id, dlt) in deltas {
        if let Some(d) = world.divisions.get_mut(&id) {
            d.org = (d.org - dlt.org_loss).max(0.0);
            d.strength = (d.strength - dlt.str_loss).max(0.0);
            d.manpower_held = (d.manpower_held - dlt.mp_loss).max(0.0);
            for (eq_type, loss) in dlt.eq_loss {
                if let Some(held) = d.equipment_held.get_mut(&eq_type) {
                    *held = (*held - loss).max(0.0);
                }
            }
        }
    }

    // P2-14: 战斗生命周期 — 移除破阵师 + 结束战斗
    cleanup_battles(world);
}

/// 累积装备消耗: 对每个装备类型, before - after(若为正即消耗)
fn accumulate_eq_loss(
    total: &mut std::collections::HashMap<String, f64>,
    before: &std::collections::HashMap<String, f64>,
    after: &std::collections::HashMap<String, f64>,
) {
    let all_keys: std::collections::HashSet<&String> = before.keys().chain(after.keys()).collect();
    for key in all_keys {
        let b = *before.get(key).unwrap_or(&0.0);
        let a = *after.get(key).unwrap_or(&0.0);
        let loss = b - a;
        if loss > 0.0 {
            *total.entry(key.clone()).or_insert(0.0) += loss;
        }
    }
}

/// 战斗生命周期: 区分撤退(org0+HP有)和歼灭(HP0)
/// - 歼灭: 从 world.divisions 删除师(番号撤销)
/// - 撤退: 标 retreating, 移出当前战斗(师保留, 待撤邻省)
///
/// 一方全退(歼灭或撤退) → 战斗结束
fn cleanup_battles(world: &mut World) {
    /// 战斗快照: (idx, province, atk前线, def前线, atk预备, def预备)
    type BattleSpec = (usize, u32, Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>);
    let battle_specs: Vec<BattleSpec> = world
        .battles
        .iter()
        .enumerate()
        .map(|(i, b)| (i, b.province, b.attackers.clone(), b.defenders.clone(),
                       b.reserve_attackers.clone(), b.reserve_defenders.clone()))
        .collect();

    let mut battles_to_remove: Vec<usize> = Vec::new();
    let mut battle_updates: Vec<(usize, Vec<u64>, Vec<u64>)> = Vec::new();
    // 占地记录: (province_id, winner_tag) — 守方全退→攻方占省
    let mut province_captures: Vec<(u32, String)> = Vec::new();
    let mut to_annihilate: Vec<u64> = Vec::new();
    // 撤退: (id, is_attacker) — 角色决定撤退方式(P3)
    let mut to_mark_retreat: Vec<(u64, bool)> = Vec::new();
    let mut routing_reserves: Vec<(u64, bool)> = Vec::new(); // 带溃: (id, is_attacker)

    for (idx, province, atk_ids, def_ids, res_atk, res_def) in &battle_specs {
        // 分类每方: 退出(歼灭/撤退)的移出, 存活的保留
        let classify = |ids: &[u64]| -> (Vec<u64>, Vec<u64>, Vec<u64>) {
            let mut alive = Vec::new();
            let mut annihilated = Vec::new();
            let mut retreating = Vec::new();
            for id in ids {
                match world.divisions.get(id) {
                    Some(d) if d.is_annihilated() => annihilated.push(*id),
                    Some(d) if d.is_retreating() => retreating.push(*id),
                    Some(_) => alive.push(*id),
                    None => annihilated.push(*id), // 已不存在的当歼灭
                }
            }
            (alive, annihilated, retreating)
        };
        let (atk_alive, atk_ann, atk_ret) = classify(atk_ids);
        let (def_alive, def_ann, def_ret) = classify(def_ids);
        to_annihilate.extend(atk_ann);
        to_annihilate.extend(def_ann);
        // 按角色标记撤退: 攻方 true / 守方 false
        for id in &atk_ret { to_mark_retreat.push((*id, true)); }
        for id in &def_ret { to_mark_retreat.push((*id, false)); }

        // 带溃机制: 前线守方全退/消灭 → 预备队强制撤退(被溃兵冲散) + 攻方占地
        // (前线崩了, 预备队还没展开就被带溃, 只能跟着撤)
        let def_frontline_routed = def_alive.is_empty();
        let atk_frontline_routed = atk_alive.is_empty();
        if def_frontline_routed || atk_frontline_routed {
            battles_to_remove.push(*idx);
            // 守方前线崩 → 守方预备队带溃撤退
            // 占地条件: 有攻方师已在该省(location==province, 非移动中) → 立即占地;
            //           否则等攻方行军到达(由 advance_movement 处理)
            if def_frontline_routed {
                for rid in res_def {
                    routing_reserves.push((*rid, false)); // 守方预备队
                }
                // 检查是否有攻方师已在该省(已到达)
                let attacker_present = (atk_alive.iter().chain(res_atk.iter()))
                    .any(|aid| world.divisions.get(aid)
                        .map(|d| d.location_province == *province && d.destination.is_none())
                        .unwrap_or(false));
                if attacker_present {
                    let winner_src = atk_alive.first().or(res_atk.first());
                    if let Some(wid) = winner_src {
                        if let Some(winner) = world.divisions.get(wid) {
                            province_captures.push((*province, winner.owner_tag.clone()));
                        }
                    }
                }
            }
            // 攻方前线崩 → 攻方预备队带溃撤退(守方守住, 不占地)
            if atk_frontline_routed {
                for rid in res_atk {
                    routing_reserves.push((*rid, true)); // 攻方预备队
                }
            }
        } else if atk_alive.len() < atk_ids.len() || def_alive.len() < def_ids.len() {
            battle_updates.push((*idx, atk_alive, def_alive));
        }
    }

    // 带溃预备队加入撤退处理(带角色)
    to_mark_retreat.extend(routing_reserves);
    // 撤退处理(P3):
    // - 攻方(is_attacker=true)→ 瞬间回 origin_province(无需行军, 取消进攻动作)
    // - 守方(is_attacker=false)→ 撤到邻接己方省(行军, retreating=true); 无邻省→包围→歼灭
    let mut surrounded: Vec<u64> = Vec::new();
    for (id, is_attacker) in to_mark_retreat {
        if is_attacker {
            // 进攻方撤退: 瞬间回出发地(取消进攻动作, 不行军)
            if let Some(d) = world.divisions.get_mut(&id) {
                d.location_province = d.origin_province;
                d.destination = None;
                d.move_progress = 0.0;
                d.attacking = false;
                d.retreating = false; // 已回到 origin, 不需行军撤退
                d.pending_arrival = None;
            }
            continue;
        }
        // 守方撤退: 撤向邻接己方省(原逻辑)
        let (loc, owner) = match world.divisions.get(&id) {
            Some(d) => (d.location_province, d.owner_tag.clone()),
            None => continue,
        };
        match world.friendly_neighbor(loc, &owner) {
            Some(dest) => {
                if let Some(d) = world.divisions.get_mut(&id) {
                    d.retreating = true;
                    d.destination = Some(dest);
                    d.move_progress = 0.0;
                }
            }
            None => {
                // 无邻接己方省 → 被包围 → 歼灭
                surrounded.push(id);
            }
        }
    }
    // 歼灭: 删除师(战斗歼灭 + 包围歼灭)
    to_annihilate.extend(surrounded);
    for id in to_annihilate {
        world.divisions.remove(&id);
    }
    // 占地: 攻方胜 → 占领战斗省份
    for (province, winner) in province_captures {
        if let Some(p) = world.provinces.get_mut(&province) {
            p.controller = winner.clone();
            p.owner = winner;
        }
    }
    // 应用战斗更新
    for (idx, atk, def) in battle_updates {
        world.battles[idx].attackers = atk;
        world.battles[idx].defenders = def;
    }
    for idx in battles_to_remove.into_iter().rev() {
        world.battles.remove(idx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inf(owner: &str) -> Division {
        Division {
            id: 0,
            owner_tag: owner.into(),
            location_province: 1,
            soft_attack: 30.0,
            hard_attack: 2.0,
            defense: 40.0,
            breakthrough: 8.0,
            armor: 0.0,
            piercing: 5.0,
            hardness: 0.0,
            combat_width: 10.0,
            max_org: 60.0,
            org: 60.0,
            max_strength: 20.0,
            strength: 20.0,
            ..Default::default()
        }
    }

    #[test]
    fn t_inf_vs_inf_reduces_org() {
        let atks = [inf("ATK")];
        let mut d = inf("DEF");
        let org_before = d.org;
        let mut defs = [&mut d];
        resolve_hour(&atks, &mut defs);
        assert!(d.org < org_before, "守方组织度应下降");
        assert!(d.org >= 0.0);
    }

    #[test]
    fn t_armor_outclass_deals_damage() {
        // 装甲师 vs 步兵(穿甲不足): 装甲碾压。调高软攻击让伤害可见
        let mut armor = inf("ATK");
        armor.armor = 50.0;
        armor.piercing = 50.0;
        armor.soft_attack = 200.0;
        let mut d = inf("DEF"); // piercing=5 < armor=50
        let org_before = d.org;
        let mut defs = [&mut d];
        resolve_hour(&[armor], &mut defs);
        assert!(d.org < org_before, "装甲碾压应造成伤害");
        assert!(org_before - d.org > 1.0, "装甲碾压伤害应显著, 实际 {}", org_before - d.org);
    }

    #[test]
    fn t_high_defense_reduces_damage() {
        // 低防御方 defense=5(攻击点超过池, 命中率 40%); 高防御方 defense=200(池未空, 10%)
        let atks = [inf("ATK")];
        let atks2 = [inf("ATK")];
        let mut low = inf("DEF");
        let mut high = inf("DEF");
        low.defense = 5.0; // 攻击点 10.5 > 5, 部分进入 40% 命中
        high.defense = 200.0; // 攻击点 10.5 < 200, 全 10% 命中
        let low_before = low.org;
        let high_before = high.org;
        let mut low_defs = [&mut low];
        let mut high_defs = [&mut high];
        resolve_hour(&atks, &mut low_defs);
        resolve_hour(&atks2, &mut high_defs);
        let low_drop = low_before - low.org;
        let high_drop = high_before - high.org;
        assert!(
            high_drop < low_drop,
            "高防御池应减少伤害: high_drop={high_drop} 应 < low_drop={low_drop}"
        );
    }

    #[test]
    fn t_p1_5_defense_pool_shared_among_attackers() {
        // P1-5: 守方 defense 池对所有攻击者共享消耗, 不再无限
        // 守方 defense=50。两个攻击者各 100 软攻击。
        // 旧 bug: 每攻击者独立 100.min(50)=50 defended, 命中同单攻击者(defense 像无限)
        // 修复: 总攻击 200, 池 50 一次性 → 50 defended(10%) + 150 undefended(40%)
        //       命中 = 50×0.10 + 150×0.40 = 5 + 60 = 65
        // 对比单攻击者 100 攻击 vs defense 50: 50×0.10 + 50×0.40 = 5+20 = 25
        // 双攻击者命中(65) 应 > 单攻击者两倍(50)? 实际 65 > 50 ✓ 溢出更多
        let mut def_double = inf("DEF");
        def_double.defense = 50.0;
        let org_before_double = def_double.org;

        let atk1 = inf("ATK");
        let atk2 = inf("ATK");
        let mut defs_d = [&mut def_double];
        // 两个攻击者同时打(聚合)
        apply_all_attackers(&[AtkStats::from(&atk1), AtkStats::from(&atk2)], &mut defs_d, CombatPool::Defense);
        let drop_double = org_before_double - def_double.org;

        // 单攻击者对照
        let mut def_single = inf("DEF");
        def_single.defense = 50.0;
        let org_before_single = def_single.org;
        let mut defs_s = [&mut def_single];
        apply_all_attackers(&[AtkStats::from(&atk1)], &mut defs_s, CombatPool::Defense);
        let drop_single = org_before_single - def_single.org;

        // 双攻击者伤害应显著大于单攻击者(因为防御池被打穿, 更多 40% 命中)
        assert!(
            drop_double > drop_single * 1.5,
            "P1-5: 双攻击者应因防御池共享而造成更多伤害: double={drop_double} single={drop_single}"
        );
    }

    // ===== World 级测试辅助 =====
    use crate::runtime::World;
    use crate::runtime::entities::Battle;

    /// 建一个 World, 三个师: A(共享师) 同时是省1战斗的攻方 + 省2战斗的守方;
    /// B 是省1守方, C 是省2攻方。两场都打 A, 验证 A 的 org 反映两场伤害之和。
    fn world_with_shared_division() -> (World, u64, u64, u64) {
        let mut w = World::new();
        let a = w.add_division(inf("ATK"));      // 共享师, 同时是省1攻方 + 省2守方
        let b = w.add_division(inf("DEF"));      // 省1守方
        let c = w.add_division(inf("DEF2"));     // 省2攻方
        // 战斗1: 省X, A 攻 B 守
        w.battles.push(Battle {
            id: 0, province: 10,
            attackers: vec![a], defenders: vec![b],
            ..Default::default()
        });
        // 战斗2: 省Y, C 攻 A 守(A 同时在这场)
        w.battles.push(Battle {
            id: 1, province: 20,
            attackers: vec![c], defenders: vec![a],
            ..Default::default()
        });
        (w, a, b, c)
    }

    #[test]
    fn t_p1_6_shared_division_takes_damage_from_both_battles() {
        // P1-6 修覆盖 bug: 同一师参与多场战斗, 两场伤害都应生效
        let (mut w, a, _b, _c) = world_with_shared_division();
        let org_before = w.divisions.get(&a).unwrap().org;

        resolve_all_battles(&mut w);

        let org_after = w.divisions.get(&a).unwrap().org;
        let total_loss = org_before - org_after;
        // A 既被 B 反击(突破池) 又被 C 打(防御池), 总伤害应明显大于只挨一场
        assert!(
            total_loss > 0.0,
            "A 应有伤害: before={org_before} after={org_after}"
        );

        // 对照: 只有一场战斗时 A 的伤害(应明显小于两场之和)
        let (mut w1, a1, _b1, _c1) = world_with_shared_division();
        w1.battles.remove(1); // 只留省1战斗
        resolve_all_battles(&mut w1);
        let loss1 = org_before - w1.divisions.get(&a1).unwrap().org;

        let (mut w2, _a2, _b2, _c2) = world_with_shared_division();
        w2.battles.remove(0); // 只留省2战斗
        resolve_all_battles(&mut w2);
        let loss2 = org_before - w2.divisions.get(&a).unwrap().org;

        // 核心断言: 总损失应严格大于任一单场(两场伤害都生效了, 非覆盖)
        assert!(
            total_loss > loss1 && total_loss > loss2,
            "P1-6: 两场伤害应累积。单场 loss1={loss1} loss2={loss2}, 总 {total_loss} 应 > 两者"
        );
        // 累积应接近两场之和(允许小误差, 因伤害公式非线性)
        let expected = loss1 + loss2;
        assert!(
            (total_loss - expected).abs() < 0.5,
            "累积值 {total_loss} 应接近两场之和 {expected}"
        );
    }

    #[test]
    fn t_p1_6_equipment_consumption_accumulates() {
        // 装备损耗也应累积而非覆盖
        let (mut w, a, _b, _c) = world_with_shared_division();
        // A 默认无装备; 手动加装备库存让 hp_loss 触发消耗可见
        if let Some(d) = w.divisions.get_mut(&a) {
            d.equipment_need.insert("infantry_equipment".into(), 100.0);
            d.equipment_held.insert("infantry_equipment".into(), 100.0);
        }
        let held_before = w.divisions.get(&a).unwrap().equipment_held
            .values().sum::<f64>();
        resolve_all_battles(&mut w);
        let held_after = w.divisions.get(&a).unwrap().equipment_held
            .values().sum::<f64>();
        // 两场都造成 str 损失 → 装备消耗应累积(明显小于单场只扣一点)
        assert!(
            held_before - held_after > 0.0,
            "装备应被消耗: before={held_before} after={held_after}"
        );
    }

    // ===== P3: 进攻失败瞬间回 origin_province(不行军) =====

    /// 进攻方 A(origin=1, location=1, 进攻省2)被打退(org=0, HP有余)
    /// → 应瞬间回 origin_province(1), destination 清空, 无需行军。
    #[test]
    fn t_p3_attacker_retreats_to_origin_instantly() {
        let mut w = World::new();
        // A: GER, 归属省1, 正进攻省2(location 仍是1, origin=1)
        let mut a = inf("ATK");
        a.owner_tag = "GER".into();
        a.location_province = 1;
        a.origin_province = 1;
        a.destination = Some(2);
        a.attacking = true;
        // 模拟被打退: org=0, strength>0
        a.org = 0.0;
        a.strength = 10.0;
        let a_id = w.add_division(a);
        // 战斗: 省2, A 攻 B 守
        w.battles.push(Battle {
            id: 0, province: 2,
            attackers: vec![a_id], defenders: vec![999], // 守方占位(实际不会分类影响)
            ..Default::default()
        });
        // 省份地图: 省1=GER(出发地), 省2=FRA
        w.provinces.insert(1, crate::runtime::Province {
            id: 1, owner: "GER".into(), controller: "GER".into(),
            terrain: "plains".into(), neighbors: vec![2],
        });
        w.provinces.insert(2, crate::runtime::Province {
            id: 2, owner: "FRA".into(), controller: "FRA".into(),
            terrain: "plains".into(), neighbors: vec![1],
        });

        resolve_all_battles(&mut w);

        let d = w.divisions.get(&a_id).expect("A 应存活(撤退非歼灭)");
        // 核心: A 瞬间回 origin_province(1), 不再行军
        assert_eq!(d.location_province, 1, "A 应回到 origin_province(1)");
        assert!(d.destination.is_none(), "进攻失败回origin后destination应清空");
        assert!(!d.attacking, "attacking 应清除");
        assert!(!d.retreating, "瞬间回origin不应再标 retreating(无需行军撤退)");
        assert!(d.pending_arrival.is_none(), "pending_arrival 应清空");
    }

    /// 守方在自己省被打退 → 仍按原逻辑撤到邻接己方省(需行军, retreating=true)。
    /// (守方 location==origin, 不触发"回出发地"分支)
    #[test]
    fn t_p3_defender_retreats_to_neighbor_keeps_marching() {
        let mut w = World::new();
        // D: FRA 守省2, origin=2(没移动过), 被打退
        let mut d = inf("DEF");
        d.owner_tag = "FRA".into();
        d.location_province = 2;
        d.origin_province = 2; // 守方: origin==location
        d.org = 0.0;
        d.strength = 10.0;
        let d_id = w.add_division(d);
        // 攻方 A: GER, soft_attack=0(不打D, 让 D 保持 org=0 触发撤退分类)
        let mut a = inf("ATK");
        a.owner_tag = "GER".into();
        a.location_province = 1;
        a.origin_province = 1;
        a.soft_attack = 0.0;
        a.hard_attack = 0.0;
        let _a_id = w.add_division(a);
        // 战斗: 省2, A 攻 D 守
        w.battles.push(Battle {
            id: 0, province: 2,
            attackers: vec![_a_id], defenders: vec![d_id],
            ..Default::default()
        });
        // 省份: 省2=FRA, 省3=FRA(邻接, 撤退目标)
        w.provinces.insert(2, crate::runtime::Province {
            id: 2, owner: "FRA".into(), controller: "FRA".into(),
            terrain: "plains".into(), neighbors: vec![3],
        });
        w.provinces.insert(3, crate::runtime::Province {
            id: 3, owner: "FRA".into(), controller: "FRA".into(),
            terrain: "plains".into(), neighbors: vec![2],
        });

        resolve_all_battles(&mut w);

        let div = w.divisions.get(&d_id).expect("D 应存活");
        // 守方撤退: 仍 location=2, destination=3(撤向邻省), retreating=true
        assert_eq!(div.location_province, 2, "守方撤退不改 location(行军中)");
        assert_eq!(div.destination, Some(3), "守方撤向邻省3");
        assert!(div.retreating, "守方撤退标 retreating");
    }
}
