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
///         ③ 按 id 写回所有受影响师(攻+守)。避免多可变借用冲突, 无 unsafe。
pub fn resolve_all_battles(world: &mut World) {
    let battle_specs: Vec<(Vec<u64>, Vec<u64>)> = world
        .battles
        .iter()
        .map(|b| (b.attackers.clone(), b.defenders.clone()))
        .collect();

    // 用 HashMap 聚合每个师的最终值(同一师可能在多场战斗, 需合并而非覆盖 → 修 P1-6)
    // 存完整快照: org/strength/equipment_held(M4a 装备消耗需写回)
    use std::collections::HashMap;
    let mut final_state: HashMap<u64, Division> = HashMap::new();

    for (atk_ids, def_ids) in &battle_specs {
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

        // 合并本场战斗结果到 final_state(存完整 Division 快照)
        for (i, id) in atk_ids.iter().enumerate() {
            if let Some(d) = atks.get(i) {
                final_state.insert(*id, d.clone());
            }
        }
        for (i, id) in def_ids.iter().enumerate() {
            if let Some(d) = defs.get(i) {
                final_state.insert(*id, d.clone());
            }
        }
    }

    // 写回: org/strength/equipment_held(M4a 装备消耗)
    for (id, snap) in final_state {
        if let Some(d) = world.divisions.get_mut(&id) {
            d.org = snap.org;
            d.strength = snap.strength;
            d.equipment_held = snap.equipment_held;
            d.manpower_held = snap.manpower_held;
        }
    }

    // P2-14: 战斗生命周期 — 移除破阵师 + 结束战斗
    cleanup_battles(world);
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
    let mut to_mark_retreat: Vec<u64> = Vec::new();
    let mut routing_reserves: Vec<u64> = Vec::new(); // 带溃: 前线崩了, 预备队强制撤退

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
        to_mark_retreat.extend(atk_ret);
        to_mark_retreat.extend(def_ret);

        // 带溃机制: 前线守方全退/消灭 → 预备队强制撤退(被溃兵冲散) + 攻方占地
        // (前线崩了, 预备队还没展开就被带溃, 只能跟着撤)
        let def_frontline_routed = def_alive.is_empty();
        let atk_frontline_routed = atk_alive.is_empty();
        if def_frontline_routed || atk_frontline_routed {
            battles_to_remove.push(*idx);
            // 守方前线崩 → 守方预备队带溃撤退 + 攻方占地(攻方有存活时)
            if def_frontline_routed {
                // 预备队强制撤退
                for rid in res_def {
                    routing_reserves.push(*rid);
                }
                // 占地
                if !atk_alive.is_empty() || !res_atk.is_empty() {
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
                    routing_reserves.push(*rid);
                }
            }
        } else if atk_alive.len() < atk_ids.len() || def_alive.len() < def_ids.len() {
            battle_updates.push((*idx, atk_alive, def_alive));
        }
    }

    // 带溃预备队加入撤退处理
    to_mark_retreat.extend(routing_reserves);
    // 撤退处理: 分配撤退目标(邻接己方省); 无邻省→被包围→歼灭
    let mut surrounded: Vec<u64> = Vec::new();
    for id in to_mark_retreat {
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
}
