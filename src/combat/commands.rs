//! 战斗相关命令注册(M3-4)
use crate::ast::Arg;
use crate::data::template::DivisionStats;
use crate::runtime::entities::{Battle, Division, OrderState};
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

/// 国家视角权限校验(对齐原版单国家控制): 玩家只能下令 player_tag 国家的师。
/// 返回 true = 玩家控制该师(可下令); false = 非玩家师(拒绝下令)。
/// 玩家未设置(player_tag 空, 如 CLI/测试)时放行(向后兼容)。
fn player_controls(world: &crate::runtime::World, div_id: u64) -> bool {
    let player = &world.player_tag;
    if player.is_empty() {
        return true; // 无玩家设定(CLI/测试), 不限制
    }
    match world.divisions.get(&div_id) {
        Some(d) => d.owner_tag == *player,
        None => false,
    }
}

/// 从汇总属性构建 Division(新路径: 数据驱动)
/// template: Some(name) = 数据驱动建师(记模板引用); None = 旧路径(无模板)
fn build_division_from_stats(owner: &str, loc: u32, stats: DivisionStats, template: Option<&str>) -> Division {
    let mut eq_need = std::collections::HashMap::new();
    let mut eq_held = std::collections::HashMap::new();
    for (eq, qty) in &stats.equipment_need {
        eq_need.insert(eq.clone(), *qty);
        eq_held.insert(eq.clone(), *qty);  // 建师时满编
    }
    Division {
        id: 0,
        owner_tag: owner.into(),
        location_province: loc,
        soft_attack: stats.soft_attack,
        hard_attack: stats.hard_attack,
        defense: stats.defense,
        breakthrough: stats.breakthrough,
        armor: stats.armor,
        piercing: stats.piercing,
        hardness: stats.hardness,
        combat_width: stats.combat_width,
        max_speed: stats.max_speed,
        max_org: stats.max_org,
        org: stats.max_org,
        max_strength: stats.max_strength,
        strength: stats.max_strength,
        equipment_need: eq_need,
        equipment_held: eq_held,
        manpower_need: stats.manpower_need,
        manpower_held: stats.manpower_need,
        order: OrderState::Idle,
        modifiers: Default::default(),
        template_name: template.map(|s| s.to_string()),
    }
}

/// 把师作为攻方加入目标省的战斗(move_division 和 support_attack 共用)。
/// - 若目标省已有战斗: 按同 origin / 宽度判定进前线或预备队
/// - 若目标省无战斗: 用 enemies 新建战斗(守方按宽度分配前线/预备队)
///   (support_attack 调用时 enemies 为空, 因下单时已确保有战斗, 不会走新建分支)
fn join_as_attacker(world: &mut crate::runtime::World, div_id: u64, target: u32, enemies: &[u64]) {
    let from_prov = world.divisions.get(&div_id).map(|d| d.location_province).unwrap_or(0);
    let div_width = world.divisions.get(&div_id).map(|d| d.combat_width).unwrap_or(0.0);
    let empty_m = crate::combat::modifier::ModifierStack::empty_static();
    let existing_idx = world.battles.iter().position(|b| b.province == target);
    if let Some(bidx) = existing_idx {
        // 加入已有战斗: 判定进前线还是预备队
        // 规则: 同出发地(from_prov)已有师在攻该目标 → 后到的进预备队(时间线落后)
        //       不同出发地 → 直接前线(新方向); 再检查宽度: 超宽也进预备队
        // origin 取值: Moving 用其 origin 字段; 其它(支援/守方转攻)用 location_province
        let origin_of = |d: &Division| -> u32 {
            d.move_origin().unwrap_or(d.location_province)
        };
        let same_origin_exists = world.started && world.battles[bidx].attackers.iter()
            .chain(world.battles[bidx].reserve_attackers.iter())
            .any(|aid| world.divisions.get(aid)
                .map(|d| origin_of(d) == from_prov)
                .unwrap_or(false));
        let over_width = !crate::combat::width::can_join_frontline(world, &world.battles[bidx].attackers, div_width, empty_m, target);
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
            if crate::combat::width::can_join_frontline(world, &frontline_d, w_div, empty_m, target) {
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
    // 建州(归属/建筑/人力的权威源)
    reg.register("create_state", |w, p| {
        let id = num_of(np(p, "create_state", "id")?)? as u32;
        let name = ParamGet::get(p, "name").and_then(Arg::as_str).unwrap_or("").to_string();
        let owner = np(p, "create_state", "owner")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?;
        let controller = ParamGet::get(p, "controller").and_then(Arg::as_str).unwrap_or(owner).to_string();
        let manpower = ParamGet::get(p, "manpower").and_then(Arg::as_num).unwrap_or(0.0);
        let category = ParamGet::get(p, "state_category").and_then(Arg::as_str).unwrap_or("wasteland").to_string();
        // cores = { GER FRA } 裸值列表
        let mut cores = Vec::new();
        if let Some(Arg::Block(fields)) = ParamGet::get(p, "cores") {
            for (_, v) in fields {
                if let Some(s) = v.as_str() { cores.push(s.to_string()); }
            }
        }
        // buildings = { infrastructure = 5 ... } 命名块
        let mut buildings = std::collections::HashMap::new();
        if let Some(Arg::Block(fields)) = ParamGet::get(p, "buildings") {
            for (k, v) in fields {
                if let Some(n) = v.as_num() { buildings.insert(k.clone(), n); }
            }
        }
        w.states.insert(id, crate::runtime::State {
            id, name, owner: owner.into(), controller, manpower,
            state_category: category, cores, buildings,
            resources: Default::default(),
            provinces: vec![],
        });
        Ok(())
    });

    // 创建省份(行军基础设施: state_id/terrain/neighbors)
    reg.register("create_province", |w, p| {
        let id = num_of(np(p, "create_province", "id")?)? as u32;
        let state_id = num_of(np(p, "create_province", "state")?)? as u32;
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
            id, state_id, terrain: terrain.into(), neighbors, ..Default::default()
        });
        // 反向注册: 省 id 加入所属 State 的 provinces 列表
        if let Some(state) = w.states.get_mut(&state_id) {
            state.provinces.push(id);
        }
        Ok(())
    });

    // 创建师(M3: 硬编码属性; M4 接装备+营汇总)
    reg.register("create_division", |w, p| {
        let owner = np(p, "create_division", "owner")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?;
        let loc = num_of(np(p, "create_division", "location")?)? as u32;
        let opt_num = |k: &str| ParamGet::get(p, k).and_then(Arg::as_num);
        // 支持三种建师方式:
        // 0) 按模板: template="xxx" → 查 GameData 汇总(数据驱动, 新路径)
        // 1) 按营数: battalions=7 + equipment=infantry_equipment → 自动算真实数值(1936, 旧路径)
        // 2) 手填: 显式给 soft_attack/defense/... (兼容旧脚本)
        if let Some(tmpl_name) = ParamGet::get(p, "template").and_then(Arg::as_str) {
            // 新路径: 数据驱动汇总(返回统计 + 未知营告警)
            let (stats, warnings) = match w.data.templates.get(tmpl_name) {
                Some(t) => t.to_division_stats(&w.data),
                None => return Err(CmdError::RuntimeError(format!("未知模板: {tmpl_name}"))),
            };
            // 告警透传到 stderr(不阻断建师, 对齐 Paradox 容错)
            for warn in &warnings {
                eprintln!("[create_division] ⚠️ {warn}");
            }
            let d = build_division_from_stats(owner, loc, stats, Some(tmpl_name));
            w.add_division(d);
            return Ok(());
        }
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
            max_speed: 4.0,  // 旧路径无模板, 默认步兵速度
            max_org,
            org: max_org,
            max_strength: max_str,
            strength: max_str,
            equipment_need: eq_need,
            equipment_held: eq_held,
            manpower_need: mp_total,
            manpower_held: mp_total,
            order: OrderState::Idle,
            modifiers: Default::default(),
            template_name: None,  // 旧路径(battalions/手填)无模板引用
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
        // 若两国未处于战争状态, 自动宣战(隐含语义: start_battle = 开战)
        if !w.are_at_war(attacker, defender) {
            w.declare_war(attacker, defender);
        }
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
        let empty_m = crate::combat::modifier::ModifierStack::empty_static();
        for did in &atks {
            let w_div = w.divisions.get(did).map(|d| d.combat_width).unwrap_or(0.0);
            if crate::combat::width::can_join_frontline(w, &frontline_a, w_div, empty_m, prov) {
                frontline_a.push(*did);
            } else {
                reserve_a.push(*did);
            }
        }
        let mut frontline_d = Vec::new();
        let mut reserve_d = Vec::new();
        for did in &defs {
            let w_div = w.divisions.get(did).map(|d| d.combat_width).unwrap_or(0.0);
            if crate::combat::width::can_join_frontline(w, &frontline_d, w_div, empty_m, prov) {
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

    // 主动行军: 师移动到目标省。下令即判定:
    // - 师在战斗地块 + 目标己方省 → 防守主动撤退(retreating=true, 退出战斗, 不分攻守)
    // - 目标有敌军 → 进攻移动(红箭头, 立刻开战)
    // - 否则普通移动(绿)
    reg.register("move_division", |w, p| {
        let div_id = num_of(np(p, "move_division", "division")?)? as u64;
        let target = num_of(np(p, "move_division", "target")?)? as u32;
        // 先取 owner + location(释放借用), 再做判定
        let (owner, cur_loc) = match w.divisions.get(&div_id) {
            Some(d) => (d.owner_tag.clone(), d.location_province),
            None => return Err(CmdError::RuntimeError(format!("move_division: 师 {div_id} 不存在"))),
        };
        // 国家视角校验: 只能下令玩家自己的师(对齐原版单国家控制)
        if !player_controls(w, div_id) {
            return Ok(()); // 非玩家师, 静默拒绝
        }
        // 防守主动撤退判定(规则4: 撤退只去邻近省份):
        // 师当前在战斗地块 + 目标是相邻的己方控制省 → 撤退
        let on_battle_province = w.battles.iter().any(|b| b.province == cur_loc);
        let target_is_friendly = w.province_controller(target).map(|c| c == owner).unwrap_or(false);
        let target_is_adjacent = w.provinces.get(&cur_loc)
            .map(|p| p.neighbors.contains(&target))
            .unwrap_or(false);
        if on_battle_province && target_is_friendly && target_is_adjacent {
            // 进入撤退状态: 从所有战斗角色移除, 转入 Retreating 行军
            // location 保持当前省(撤退路上不可见, 到达才改)
            if let Some(d) = w.divisions.get_mut(&div_id) {
                d.order = OrderState::Retreating { dest: target, progress: 0.0 };
            }
            for b in w.battles.iter_mut() {
                b.attackers.retain(|&id| id != div_id);
                b.defenders.retain(|&id| id != div_id);
                b.reserve_attackers.retain(|&id| id != div_id);
                b.reserve_defenders.retain(|&id| id != div_id);
            }
            return Ok(());
        }
        // 【边界B】师在 Pending/Retreating/Supporting → 忽略移动命令(不能中断战斗/撤退/支援)
        let blocked_state = match w.divisions.get(&div_id) {
            Some(d) => d.is_pending() || d.is_withdrawing() || d.is_supporting(),
            None => false,
        };
        if blocked_state {
            return Ok(());
        }
        // 【边界C】目标 == 当前省 → 忽略(无意义命令)
        if target == cur_loc {
            return Ok(());
        }
        // 【寻路】find_path 返回 [下一站, ..., 最终目标]; None 则师不动
        let path = match crate::combat::pathfinding::find_path(w, cur_loc, target) {
            Some(p) => p,
            None => return Ok(()), // 不连通或目标不在地图, 静默忽略
        };
        // path 非空: 拆成 dest(第一站) + remaining(后续站, 不含 dest)
        let first = path[0];
        let remaining: Vec<u32> = path[1..].to_vec();
        // 进军判定: 第一站非己方控制 → 进军红箭头
        let first_controller = w.province_controller(first).unwrap_or("");
        let is_hostile = first_controller != owner;
        // 设移动状态: dest=第一站, remaining=后续站
        if let Some(d) = w.divisions.get_mut(&div_id) {
            d.order = OrderState::Moving {
                dest: first, progress: 0.0,
                hostile: is_hostile, origin: cur_loc,
                remaining,
            };
        }
        // 第一站有敌军防守 → 开战: 加入或新建战斗(复用 join_as_attacker)
        // 先快照敌人 tag(避借用冲突: are_at_war 借 w, divisions.values() 借 divisions)
        let enemy_tags: Vec<String> = w.enemies_of(&owner);
        let first_enemies: Vec<u64> = w.divisions.values()
            .filter(|d| d.location_province == first && enemy_tags.contains(&d.owner_tag) && !d.is_withdrawing())
            .map(|d| d.id)
            .collect();
        if !first_enemies.is_empty() {
            join_as_attacker(w, div_id, first, &first_enemies);
        }
        Ok(())
    });

    // 支援攻击: 师不移动, 作为攻方远程参与目标省战斗。
    // 规则: 下令时目标省须已有战斗 且 与师 location 相邻, 否则指令无效(静默取消)。
    // 其他判定(加入战斗/伤害/宽度)与移动攻击一致。
    reg.register("support_attack", |w, p| {
        let div_id = num_of(np(p, "support_attack", "division")?)? as u64;
        let target = num_of(np(p, "support_attack", "target")?)? as u32;
        // 国家视角校验
        if !player_controls(w, div_id) {
            return Ok(());
        }
        // 【决策13】邻接检查: 目标省须与师 location 相邻, 否则静默无效
        let cur_loc = match w.divisions.get(&div_id) {
            Some(d) => d.location_province,
            None => return Ok(()),
        };
        let adjacent = w.provinces.get(&cur_loc)
            .map(|p| p.neighbors.contains(&target))
            .unwrap_or(false);
        if !adjacent {
            return Ok(()); // 不相邻, 静默无效
        }
        // 检查目标省是否已有战斗(下单时判定; 无战斗 → 指令无效)
        let has_battle = w.battles.iter().any(|b| b.province == target);
        if !has_battle {
            // 静默取消: 不报错, 不设 supporting(蓝色箭头不出现)
            return Ok(());
        }
        // 取该战斗的守方(已在前线的敌军), 用于 join_as_attacker 的 enemies 参数
        // (join_as_attacker 在已有战斗时只走"加入"分支, enemies 仅用于新建分支, 传空即可)
        let enemies: Vec<u64> = Vec::new();
        // 设支援状态(师不移动: 不改 location/order 内的移动字段)
        if let Some(d) = w.divisions.get_mut(&div_id) {
            d.order = OrderState::Supporting { target };
        }
        // 加入已有战斗(同 origin / 宽度判定, 复用 move_division 逻辑)
        join_as_attacker(w, div_id, target, &enemies);
        Ok(())
    });

    // 停止命令: 取消师当前主动发起的行动(进军/移动/支援), 保留被动防守和撤退。
    // 规则:
    // - Retreating → 完全忽略(撤退不能停, 哪怕有 destination)
    // - Moving/Supporting → 可停止: 转回 Idle, 从主动参与的战斗 attackers/reserve_attackers 移除
    // - 不动 defenders/reserve_defenders(被动防守继续)
    // - Idle/Pending(纯防守/撤退变攻方)→ 忽略(无主动指令可停)
    reg.register("stop_order", |w, p| {
        let div_id = num_of(np(p, "stop_order", "division")?)? as u64;
        // 国家视角校验
        if !player_controls(w, div_id) {
            return Ok(());
        }
        // 读取判断(单独作用域, 释放借用后再 get_mut)
        let should_stop = {
            let Some(d) = w.divisions.get(&div_id) else { return Ok(()); };
            matches!(d.order, OrderState::Moving { .. } | OrderState::Supporting { .. })
        };
        if !should_stop { return Ok(()); }
        // 清主动行动状态 → Idle
        if let Some(d) = w.divisions.get_mut(&div_id) {
            d.order = OrderState::Idle;
        }
        // 从所有战斗的"攻方"角色移除(保留守方角色 = 被动防守)
        for b in w.battles.iter_mut() {
            b.attackers.retain(|&id| id != div_id);
            b.reserve_attackers.retain(|&id| id != div_id);
            // 不动 defenders / reserve_defenders(被动防守)
        }
        Ok(())
    });

    // 航点追加: 把目标追加到当前行军路径末尾(多段长程规划, 手机端友好无需 shift)。
    // - 当前 Moving: 从路径末尾寻路到 target, 拼接到 remaining
    // - 当前 Idle: 等同 move_division(从头寻路)
    // - Pending/Retreating/Supporting: 忽略(决策11/4.4)
    reg.register("queue_move", |w, p| {
        let div_id = num_of(np(p, "queue_move", "division")?)? as u64;
        let target = num_of(np(p, "queue_move", "target")?)? as u32;
        // 国家视角校验(先于读取, 避免无效借用)
        if !player_controls(w, div_id) {
            return Ok(());
        }
        // 读当前 location + owner(释放借用)
        let (cur_loc, owner) = match w.divisions.get(&div_id) {
            Some(d) => (d.location_province, d.owner_tag.clone()),
            None => return Ok(()),
        };
        // 边界C: 同省忽略(无意义追加)
        if target == cur_loc {
            return Ok(());
        }
        // 读当前 order 决定追加(Moving)还是新建(Idle)
        let order_snapshot = w.divisions.get(&div_id).map(|d| d.order.clone());
        match order_snapshot {
            Some(OrderState::Moving { dest, ref remaining, .. }) => {
                // 路径末尾 = remaining 最后一个, 或 dest(remaining 空时)
                let end_prov = remaining.last().copied().unwrap_or(dest);
                if end_prov == target {
                    return Ok(()); // 追加的就是当前末尾, 无意义
                }
                // 从末尾寻路到 target
                let seg = match crate::combat::pathfinding::find_path(w, end_prov, target) {
                    Some(s) => s,
                    None => return Ok(()), // 不连通, 忽略
                };
                // 拼接到 remaining
                if let Some(d) = w.divisions.get_mut(&div_id) {
                    if let OrderState::Moving { ref mut remaining, .. } = d.order {
                        remaining.extend(seg);
                    }
                }
                Ok(())
            }
            Some(OrderState::Idle) => {
                // 等同 move_division: 从 cur_loc 寻路到 target
                let path = match crate::combat::pathfinding::find_path(w, cur_loc, target) {
                    Some(p) => p,
                    None => return Ok(()),
                };
                let first = path[0];
                let remaining: Vec<u32> = path[1..].to_vec();
                let hostile = w.province_controller(first).map(|c| c != owner).unwrap_or(false);
                if let Some(d) = w.divisions.get_mut(&div_id) {
                    d.order = OrderState::Moving {
                        dest: first, progress: 0.0, hostile, origin: cur_loc, remaining,
                    };
                }
                // 第一站有敌军 → 开战
                let enemy_tags2: Vec<String> = w.enemies_of(&owner);
                let first_enemies: Vec<u64> = w.divisions.values()
                    .filter(|d| d.location_province == first && enemy_tags2.contains(&d.owner_tag) && !d.is_withdrawing())
                    .map(|d| d.id).collect();
                if !first_enemies.is_empty() {
                    join_as_attacker(w, div_id, first, &first_enemies);
                }
                Ok(())
            }
            // Pending/Retreating/Supporting → 忽略
            _ => Ok(()),
        }
    });

    // 加国家级 modifier(科技/国策/精神触发)
    // stat 用原版属性名(带或不带 _factor), op 由后缀推导(spec §3)
    reg.register("add_country_modifier", |w, p| {
        let tag = np(p, "add_country_modifier", "tag")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("tag 应为字符串".into()))?;
        let token = np(p, "add_country_modifier", "stat")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("stat 应为字符串".into()))?;
        let value = num_of(np(p, "add_country_modifier", "value")?)?;
        let (stat, op) = crate::combat::modifier::parse_modifier_token(token)
            .ok_or_else(|| CmdError::RuntimeError(format!("未知属性: {token}")))?;
        let country = w.countries.entry(tag.into()).or_default();
        country.modifiers.push(crate::combat::modifier::Modifier { stat, value, op });
        Ok(())
    });

    // 加师级 modifier(堑壕/计划/经验)
    reg.register("add_division_modifier", |w, p| {
        let div_id = num_of(np(p, "add_division_modifier", "division")?)? as u64;
        let token = np(p, "add_division_modifier", "stat")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("stat 应为字符串".into()))?;
        let value = num_of(np(p, "add_division_modifier", "value")?)?;
        let (stat, op) = crate::combat::modifier::parse_modifier_token(token)
            .ok_or_else(|| CmdError::RuntimeError(format!("未知属性: {token}")))?;
        let Some(d) = w.divisions.get_mut(&div_id) else {
            return Err(CmdError::RuntimeError(format!("师 {div_id} 不存在")));
        };
        d.modifiers.push(crate::combat::modifier::Modifier { stat, value, op });
        Ok(())
    });

    // 宣战(建立战争, 阵营自动拉入)
    reg.register("declare_war", |w, p| {
        let attacker = np(p, "declare_war", "attacker")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("attacker 应为字符串".into()))?;
        let defender = np(p, "declare_war", "defender")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("defender 应为字符串".into()))?;
        w.declare_war(attacker, defender);
        Ok(())
    });

    // 白和(无条件停火, 结束两国间所有战争)
    reg.register("white_peace", |w, p| {
        let a = np(p, "white_peace", "a")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("a 应为字符串".into()))?;
        let b = np(p, "white_peace", "b")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("b 应为字符串".into()))?;
        w.wars.retain(|war| {
            !(war.attackers.contains(a) && war.defenders.contains(b)
                || war.defenders.contains(a) && war.attackers.contains(b))
        });
        Ok(())
    });

    // 创建阵营
    reg.register("create_faction", |w, p| {
        let leader = np(p, "create_faction", "leader")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("leader 应为字符串".into()))?;
        let name = np(p, "create_faction", "name")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("name 应为字符串".into()))?;
        let country = w.countries.entry(leader.into()).or_default();
        country.tag = leader.into();
        country.faction = Some(name.into());
        Ok(())
    });

    // 加入阵营
    reg.register("join_faction", |w, p| {
        let tag = np(p, "join_faction", "tag")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("tag 应为字符串".into()))?;
        let name = np(p, "join_faction", "name")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("name 应为字符串".into()))?;
        let country = w.countries.entry(tag.into()).or_default();
        country.tag = tag.into();
        country.faction = Some(name.into());
        Ok(())
    });

    // 换师的模板(重新汇总数值, 保留运行态 location/org/strength)
    // 对齐原版 add_units_to_division_template 的"师↔模板引用"语义
    reg.register("change_template", |w, p| {
        let div_id = num_of(np(p, "change_template", "division")?)? as u64;
        let tmpl_name = np(p, "change_template", "template")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("template 应为字符串".into()))?;
        let (stats, warnings) = match w.data.templates.get(tmpl_name) {
            Some(t) => t.to_division_stats(&w.data),
            None => return Err(CmdError::RuntimeError(format!("未知模板: {tmpl_name}"))),
        };
        for warn in &warnings {
            eprintln!("[change_template] ⚠️ {warn}");
        }
        let d = w.divisions.get_mut(&div_id)
            .ok_or_else(|| CmdError::RuntimeError(format!("师 #{div_id} 不存在")))?;
        // 覆盖战斗属性(模板汇总)
        d.soft_attack = stats.soft_attack;
        d.hard_attack = stats.hard_attack;
        d.defense = stats.defense;
        d.breakthrough = stats.breakthrough;
        d.armor = stats.armor;
        d.piercing = stats.piercing;
        d.hardness = stats.hardness;
        d.combat_width = stats.combat_width;
        d.max_speed = stats.max_speed;
        d.max_org = stats.max_org;
        d.max_strength = stats.max_strength;
        d.manpower_need = stats.manpower_need;
        // 装备需求更新(held 保持当前持有, 不强制满编——换模板可能缺装备)
        d.equipment_need = stats.equipment_need.clone();
        d.template_name = Some(tmpl_name.to_string());
        // 运行态保留: location_province / org / strength / order / modifiers 不动
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

    // 创建生产线
    reg.register("create_production_line", |w, p| {
        let owner = np(p, "create_production_line", "owner")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?
            .to_string();
        let variant = np(p, "create_production_line", "variant")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("variant 应为字符串".into()))?
            .to_string();
        let factories = num_of(np(p, "create_production_line", "factories")?)? as u32;

        if !w.data.equipment.contains_key(&variant) {
            return Err(CmdError::RuntimeError(format!("variant {} 未在 GameData", variant)));
        }

        let id = w.countries.get(&owner)
            .map(|c| c.production_lines.iter().map(|l| l.id).max().unwrap_or(0) + 1)
            .unwrap_or(1);

        let mut line = crate::economy::ProductionLine::new(id, variant);
        line.set_active(factories);

        let country = w.countries.entry(owner).or_default();
        country.production_lines.push(line);
        Ok(())
    });

    // 调整生产线工厂数
    reg.register("set_line_factories", |w, p| {
        let owner = np(p, "set_line_factories", "owner")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?
            .to_string();
        let line_id = num_of(np(p, "set_line_factories", "line_id")?)? as u32;
        let factories = num_of(np(p, "set_line_factories", "factories")?)? as u32;

        let country = w.countries.entry(owner).or_default();
        let line = country.production_lines.iter_mut()
            .find(|l| l.id == line_id)
            .ok_or_else(|| CmdError::RuntimeError(format!("line_id {} 不存在", line_id)))?;
        line.set_active(factories);
        Ok(())
    });

    // 切换生产线型号
    reg.register("change_line_variant", |w, p| {
        let owner = np(p, "change_line_variant", "owner")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?
            .to_string();
        let line_id = num_of(np(p, "change_line_variant", "line_id")?)? as u32;
        let variant = np(p, "change_line_variant", "variant")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("variant 应为字符串".into()))?
            .to_string();

        if !w.data.equipment.contains_key(&variant) {
            return Err(CmdError::RuntimeError(format!("variant {} 未在 GameData", variant)));
        }

        let country = w.countries.entry(owner).or_default();
        let line = country.production_lines.iter_mut()
            .find(|l| l.id == line_id)
            .ok_or_else(|| CmdError::RuntimeError(format!("line_id {} 不存在", line_id)))?;
        crate::economy::production::change_line_variant(line, &variant);
        Ok(())
    });

    // 删除生产线
    reg.register("remove_production_line", |w, p| {
        let owner = np(p, "remove_production_line", "owner")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("owner 应为字符串".into()))?
            .to_string();
        let line_id = num_of(np(p, "remove_production_line", "line_id")?)? as u32;

        let country = w.countries.entry(owner).or_default();
        let before = country.production_lines.len();
        country.production_lines.retain(|l| l.id != line_id);
        if country.production_lines.len() == before {
            return Err(CmdError::RuntimeError(format!("line_id {} 不存在", line_id)));
        }
        Ok(())
    });

    // State 资源调试命令(demo setup 用)
    reg.register("add_state_resource", |w, p| {
        let sid = num_of(np(p, "add_state_resource", "state")?)? as u32;
        let resource = np(p, "add_state_resource", "resource")?.as_str()
            .ok_or_else(|| CmdError::RuntimeError("resource 应为字符串".into()))?
            .to_string();
        let amount = num_of(np(p, "add_state_resource", "amount")?)?;

        let state = w.states.get_mut(&sid)
            .ok_or_else(|| CmdError::RuntimeError(format!("state {} 不存在", sid)))?;
        *state.resources.entry(resource).or_insert(0.0) += amount;
        Ok(())
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::registry::Registry;

    #[test]
    fn t_production_commands_registered() {
        let mut reg = Registry::new();
        register(&mut reg);
        // 5 new commands should be registered
        for name in ["create_production_line", "set_line_factories",
                     "change_line_variant", "remove_production_line",
                     "add_state_resource"] {
            assert!(reg.get_effect(name).is_some(), "{name} 应已注册");
        }
    }
}
