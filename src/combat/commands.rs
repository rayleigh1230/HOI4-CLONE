//! 战斗相关命令注册(M3-4)
use crate::ast::Arg;
use crate::runtime::entities::{Battle, Division};
use crate::runtime::error::CmdError;
use crate::runtime::registry::ParamGet;
use crate::runtime::Registry;

/// 取命名参数, 缺失返回 BadParam
fn np<'a>(p: &'a [(String, Arg)], cmd: &str, key: &str) -> Result<&'a Arg, CmdError> {
    ParamGet::get(p, key).ok_or_else(|| {
        CmdError::BadParam { cmd: cmd.into(), key: key.into(), reason: "缺少参数".into() }
    })
}
fn num_of(a: &Arg) -> Result<f64, CmdError> {
    a.as_num().ok_or_else(|| CmdError::RuntimeError(format!("期望数字, 得 {:?}", a)))
}

/// 把师作为攻方加入目标省的战斗(move_division 和 support_attack 共用)。
/// - 若目标省已有战斗: 按同 origin / 宽度判定进前线或预备队
/// - 若目标省无战斗: 用 enemies 新建战斗(守方按宽度分配前线/预备队)
///   (support_attack 调用时 enemies 为空, 因下单时已确保有战斗, 不会走新建分支)
fn join_as_attacker(world: &mut crate::runtime::World, div_id: u64, target: u32, enemies: &[u64]) {
    let from_prov = world.divisions.get(&div_id).map(|d| d.location_province).unwrap_or(0);
    let div_width = world.divisions.get(&div_id).map(|d| d.combat_width).unwrap_or(0.0);
    let existing_idx = world.battles.iter().position(|b| b.province == target);
    if let Some(bidx) = existing_idx {
        // 加入已有战斗: 判定进前线还是预备队
        // 规则: 同出发地(from_prov)已有师在攻该目标 → 后到的进预备队(时间线落后)
        //       不同出发地 → 直接前线(新方向); 再检查宽度: 超宽也进预备队
        let same_origin_exists = world.battles[bidx].attackers.iter()
            .chain(world.battles[bidx].reserve_attackers.iter())
            .any(|aid| world.divisions.get(aid)
                .map(|d| d.origin_province == from_prov)
                .unwrap_or(false));
        let over_width = !crate::combat::width::can_join_frontline(world, &world.battles[bidx].attackers, div_width);
        if same_origin_exists || over_width {
            world.battles[bidx].reserve_attackers.push(div_id);
        } else {
            world.battles[bidx].attackers.push(div_id);
        }
    } else {
        // 新建战斗: 守方按宽度分配(守方无出发地概念, 用宽度)
        let mut frontline_d = Vec::new();
        let mut reserve_d = Vec::new();
        for eid in enemies {
            let w_div = world.divisions.get(eid).map(|d| d.combat_width).unwrap_or(0.0);
            if crate::combat::width::can_join_frontline(world, &frontline_d, w_div) {
                frontline_d.push(*eid);
            } else {
                reserve_d.push(*eid);
            }
        }
        let battle_id = world.next_battle_id;
        world.next_battle_id += 1;
        world.battles.push(Battle {
            id: battle_id, province: target,
            attackers: vec![div_id], defenders: frontline_d,
            reserve_attackers: vec![], reserve_defenders: reserve_d,
        });
    }
}

pub fn register(reg: &mut Registry) {
    // 创建省份(行军基础设施: owner/controller/neighbors)
    reg.register("create_province", |w, p| {
        let id = num_of(np(p, "create_province", "id")?)? as u32;
        let owner = np(p, "create_province", "owner")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?;
        let terrain = ParamGet::get(p, "terrain").and_then(Arg::as_str).unwrap_or("plains");
        // neighbors: 嵌套块参数 neighbors = { 10 20 } 或单值
        let mut neighbors = Vec::new();
        if let Some(Arg::Block(fields)) = ParamGet::get(p, "neighbors") {
            for (_, v) in fields {
                if let Some(n) = v.as_num() {
                    neighbors.push(n as u32);
                }
            }
        }
        w.provinces.insert(id, crate::runtime::Province {
            id,
            owner: owner.into(),
            controller: owner.into(),
            terrain: terrain.into(),
            neighbors,
        });
        Ok(())
    });

    // 创建师(M3: 硬编码属性; M4 接装备+营汇总)
    reg.register("create_division", |w, p| {
        let owner = np(p, "create_division", "owner")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?;
        let loc = num_of(np(p, "create_division", "location")?)? as u32;
        let opt_num = |k: &str| ParamGet::get(p, k).and_then(Arg::as_num);
        // 支持两种建师方式:
        // 1) 按营数: battalions=7 + equipment=infantry_equipment → 自动算真实数值(1936)
        // 2) 手填: 显式给 soft_attack/defense/... (兼容旧脚本)
        let (sa, ha, df, br, ar, pr, hd, cw, max_org, max_str, mp_total, eq_amt) =
            if let Some(bn) = opt_num("battalions") {
                // 按营数 + 装备查表
                let eq_name = ParamGet::get(p, "equipment").and_then(Arg::as_str).unwrap_or("infantry_equipment");
                let e = crate::combat::equipment_data::find_equipment(eq_name)
                    .copied()
                    .unwrap_or_else(|| {
                        // 未知装备退回步兵
                        *crate::combat::equipment_data::find_equipment("infantry_equipment").unwrap()
                    });
                let n = bn;
                (
                    n * e.soft_attack,
                    n * e.hard_attack,
                    n * e.defense,
                    n * e.breakthrough,
                    e.armor,  // 装甲取最高(简化), 不×营数
                    e.piercing,
                    e.hardness,
                    n * crate::combat::equipment_data::BATTALION_WIDTH,
                    crate::combat::equipment_data::BATTALION_ORG, // org 加权平均(同类营)
                    n * crate::combat::equipment_data::BATTALION_HP,
                    n * crate::combat::equipment_data::BATTALION_MANPOWER,
                    n * crate::combat::equipment_data::BATTALION_EQUIPMENT_NEED,
                )
            } else {
                // 手填(兼容)
                (
                    opt_num("soft_attack").unwrap_or(21.0),
                    opt_num("hard_attack").unwrap_or(3.5),
                    opt_num("defense").unwrap_or(140.0),
                    opt_num("breakthrough").unwrap_or(14.0),
                    opt_num("armor").unwrap_or(0.0),
                    opt_num("piercing").unwrap_or(7.0),
                    opt_num("hardness").unwrap_or(0.0),
                    opt_num("combat_width").unwrap_or(14.0),
                    opt_num("max_org").unwrap_or(60.0),
                    opt_num("max_strength").unwrap_or(175.0),
                    opt_num("manpower").unwrap_or(7000.0),
                    100.0,
                )
            };
        // 装备需求/持有(按算出的 eq_amt 满编)
        let eq_name = ParamGet::get(p, "equipment").and_then(Arg::as_str).unwrap_or("infantry_equipment");
        let mut eq_need = std::collections::HashMap::new();
        let mut eq_held = std::collections::HashMap::new();
        eq_need.insert(eq_name.to_string(), eq_amt);
        eq_held.insert(eq_name.to_string(), eq_amt); // 建师时满编
        let d = Division {
            id: 0,
            owner_tag: owner.into(),
            location_province: loc,
            soft_attack: sa,
            hard_attack: ha,
            defense: df,
            breakthrough: br,
            armor: ar,
            piercing: pr,
            hardness: hd,
            combat_width: cw,
            max_org,
            org: max_org,
            max_strength: max_str,
            strength: max_str,
            equipment_need: eq_need,
            equipment_held: eq_held,
            manpower_need: mp_total,
            manpower_held: mp_total,
            retreating: false,
            destination: None,
            move_progress: 0.0,
            attacking: false,
            origin_province: loc,
            pending_arrival: None,
            supporting: None,
        };
        w.add_division(d);
        Ok(())
    });

    // 往国家仓库加装备(M4a 手动补充; M4b 由生产系统自动产)
    reg.register("add_equipment", |w, p| {
        let owner = np(p, "add_equipment", "owner")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?;
        let eq = np(p, "add_equipment", "type")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("type 应为字符串".into()))?;
        let amt = num_of(np(p, "add_equipment", "amount")?)?;
        let country = w.countries.entry(owner.into()).or_default();
        *country.equipment_stockpile.entry(eq.into()).or_insert(0.0) += amt;
        Ok(())
    });

    // 往国家人力池加兵员(陆战循环)
    reg.register("add_manpower", |w, p| {
        let owner = np(p, "add_manpower", "owner")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?;
        let amt = num_of(np(p, "add_manpower", "amount")?)?;
        let country = w.countries.entry(owner.into()).or_default();
        country.manpower_pool += amt;
        Ok(())
    });

    // 开始战斗: 把两个 tag 的师设为攻守
    reg.register("start_battle", |w, p| {
        let attacker = np(p, "start_battle", "attacker")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("attacker 应为字符串".into()))?;
        let defender = np(p, "start_battle", "defender")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("defender 应为字符串".into()))?;
        let prov = num_of(np(p, "start_battle", "province")?)? as u32;
        let atks = w.divisions_of(attacker);
        let defs = w.divisions_of(defender);
        if atks.is_empty() || defs.is_empty() {
            return Err(CmdError::RuntimeError(
                format!("start_battle: 攻方 {attacker} 或守方 {defender} 无师"),
            ));
        }
        let id = w.next_battle_id;
        w.next_battle_id += 1;
        // 宽度分配: 逐个加入前线, 超宽(>70)的进预备队
        let mut frontline_a = Vec::new();
        let mut reserve_a = Vec::new();
        for did in &atks {
            let w_div = w.divisions.get(did).map(|d| d.combat_width).unwrap_or(0.0);
            if crate::combat::width::can_join_frontline(w, &frontline_a, w_div) {
                frontline_a.push(*did);
            } else {
                reserve_a.push(*did);
            }
        }
        let mut frontline_d = Vec::new();
        let mut reserve_d = Vec::new();
        for did in &defs {
            let w_div = w.divisions.get(did).map(|d| d.combat_width).unwrap_or(0.0);
            if crate::combat::width::can_join_frontline(w, &frontline_d, w_div) {
                frontline_d.push(*did);
            } else {
                reserve_d.push(*did);
            }
        }
        w.battles.push(Battle {
            id, province: prov,
            attackers: frontline_a, defenders: frontline_d,
            reserve_attackers: reserve_a, reserve_defenders: reserve_d,
        });
        Ok(())
    });

    // 主动行军: 师移动到目标省。下令即判定: 目标有敌军→进攻移动(红箭头, 立刻开战); 否则普通移动(绿)
    reg.register("move_division", |w, p| {
        let div_id = num_of(np(p, "move_division", "division")?)? as u64;
        let target = num_of(np(p, "move_division", "target")?)? as u32;
        // 先取 owner(释放借用), 再查敌军, 最后改师
        let owner = w.divisions.get(&div_id)
            .ok_or_else(|| CmdError::RuntimeError(format!("move_division: 师 {div_id} 不存在")))?
            .owner_tag.clone();
        // 查目标省有无敌军(非己方的师; 排除撤退师 — 撤退师不当守方被重新拉入)
        let enemies: Vec<u64> = w.divisions.values()
            .filter(|d| d.location_province == target && d.owner_tag != owner && !d.retreating)
            .map(|d| d.id)
            .collect();
        // 进军判定: 目标省非己方控制 → 进军红箭头(无论有无敌军)
        let target_controller = w.provinces.get(&target).map(|p| p.controller.as_str()).unwrap_or("");
        let is_hostile = target_controller != owner;
        // 设移动状态
        if let Some(d) = w.divisions.get_mut(&div_id) {
            d.origin_province = d.location_province; // 记录出发地
            d.destination = Some(target);
            d.move_progress = 0.0;
            d.attacking = is_hostile; // 进军(敌方地块)=红
            if is_hostile {
                d.retreating = false; // 进军取消恢复; 己方行军保留恢复
            }
        }
        // 有敌军防守 → 开战: 加入或新建战斗(复用 join_as_attacker)
        if !enemies.is_empty() {
            join_as_attacker(w, div_id, target, &enemies);
        }
        Ok(())
    });

    // 支援攻击: 师不移动, 作为攻方远程参与目标省战斗。
    // 规则: 下令时目标省须已有战斗, 否则指令无效(静默取消, 不设 supporting)。
    // 其他判定(加入战斗/伤害/宽度)与移动攻击一致。
    reg.register("support_attack", |w, p| {
        let div_id = num_of(np(p, "support_attack", "division")?)? as u64;
        let target = num_of(np(p, "support_attack", "target")?)? as u32;
        // 检查目标省是否已有战斗(下单时判定; 无战斗 → 指令无效)
        let has_battle = w.battles.iter().any(|b| b.province == target);
        if !has_battle {
            // 静默取消: 不报错, 不设 supporting(蓝色箭头不出现)
            return Ok(());
        }
        // 取该战斗的守方(已在前线的敌军), 用于 join_as_attacker 的 enemies 参数
        // (join_as_attacker 在已有战斗时只走"加入"分支, enemies 仅用于新建分支, 传空即可)
        let enemies: Vec<u64> = Vec::new();
        // 设支援状态(师不移动: 不改 location/destination/move_progress/attacking)
        if let Some(d) = w.divisions.get_mut(&div_id) {
            d.supporting = Some(target);
        }
        // 加入已有战斗(同 origin / 宽度判定, 复用 move_division 逻辑)
        join_as_attacker(w, div_id, target, &enemies);
        Ok(())
    });

    // trigger: 当前作用域师是否破阵
    reg.register_trigger("is_broken", |w, _p| {
        if let Some(did) = w.current_scope().division_id() {
            Ok(w.divisions.get(&did).map(|d| d.is_broken()).unwrap_or(false))
        } else {
            Ok(false)
        }
    });
}
