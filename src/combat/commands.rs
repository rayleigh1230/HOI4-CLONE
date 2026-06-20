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

pub fn register(reg: &mut Registry) {
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
                        crate::combat::equipment_data::find_equipment("infantry_equipment").unwrap().clone()
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
        w.battles.push(Battle { id, province: prov, attackers: atks, defenders: defs });
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
