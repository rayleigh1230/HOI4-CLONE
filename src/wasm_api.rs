//! WASM 桥接层: 把引擎核心暴露给 JS(无 wasm-bindgen, 纯手写 FFI)
//!
//! 设计: 用全局 World + 字符串交换。
//! JS 调 run_setup(script) 建场景, tick(n) 推进, get_state() 取 JSON 快照。
//! 内存由 Rust 管, JS 只读字符串结果(避免跨边界传复杂结构)。

#![allow(clippy::missing_safety_doc)] // FFI 函数, JS 单线程调用

use crate::ast::lower::lower_effects;
use crate::commands::register_all;
use crate::parser::{parse, Value};
use crate::runtime::{Interpreter, Registry, World};
use std::cell::RefCell;

thread_local! {
    static ENGINE: RefCell<Engine> = RefCell::new(Engine::new());
}

struct Engine {
    interp: Interpreter,
    world: World,
}

impl Engine {
    fn new() -> Self {
        let mut reg = Registry::new();
        register_all(&mut reg);
        // demo stub trigger: 让作用域遍历能跑通
        reg.register_trigger("is_owned_and_controlled_by", |_, _| Ok(true));
        reg.register_trigger("is_core", |_, _| Ok(true));
        Self {
            interp: Interpreter::new(reg),
            world: World::new(),
        }
    }
}

/// 在 wasm 线性内存分配 n 字节, 返回起始指针。JS 用它写入字符串再传给其他 API。
#[no_mangle]
pub extern "C" fn engine_alloc(n: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(n);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf); // 泄漏, JS 持有直到本次调用结束
    ptr
}

/// 初始化世界(重置)
#[no_mangle]
pub extern "C" fn engine_reset() {
    ENGINE.with(|e| {
        *e.borrow_mut() = Engine::new();
    });
}

/// 设置玩家国家 tag(JS 传空终止字符串)
#[no_mangle]
pub unsafe extern "C" fn engine_set_player(tag_ptr: *const u8, tag_len: usize) {
    let tag = unsafe { ptr_to_str(tag_ptr, tag_len) };
    ENGINE.with(|e| {
        e.borrow_mut().world.player_tag = tag.to_string();
        e.borrow_mut()
            .world
            .countries
            .entry(tag.to_string())
            .or_default()
            .tag = tag.to_string();
    });
}

/// 设置省份控制权(前端点省份改归属 = 设定包围)
#[no_mangle]
pub unsafe extern "C" fn engine_set_province_controller(
    province_id: u32,
    tag_ptr: *const u8,
    tag_len: usize,
) {
    let tag = unsafe { ptr_to_str(tag_ptr, tag_len) };
    ENGINE.with(|e| {
        if let Some(p) = e.borrow_mut().world.provinces.get_mut(&province_id) {
            p.controller = tag.to_string();
            p.owner = tag.to_string();
        }
    });
}

/// 命令师移动到目标省(前端点选移动)
/// 注: division_id 用 u32 而非 u64, 避免 JS 调用时 BigInt 转换问题
#[no_mangle]
pub extern "C" fn engine_move_division(division_id: u32, target: u32) {
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Engine { interp, world } = &mut *e;
        let script = format!("move_division = {{ division = {division_id} target = {target} }}");
        if let Ok(b) = crate::parser::parse(&script) {
            let effs = crate::ast::lower::lower_effects(&b);
            interp.run(&effs, world);
        }
    });
}

/// 命令师支援攻击目标省(师不移动, 作为攻方远程参战)
/// 目标省须已有战斗, 否则命令无效(静默取消)
#[no_mangle]
pub extern "C" fn engine_support_attack(division_id: u32, target: u32) {
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Engine { interp, world } = &mut *e;
        let script = format!("support_attack = {{ division = {division_id} target = {target} }}");
        if let Ok(b) = crate::parser::parse(&script) {
            let effs = crate::ast::lower::lower_effects(&b);
            interp.run(&effs, world);
        }
    });
}

/// 追加航点到师的行军路径(前端航点规划用, 手机端友好无需 shift)
#[no_mangle]
pub extern "C" fn engine_queue_move(division_id: u32, target: u32) {
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Engine { interp, world } = &mut *e;
        let script = format!("queue_move = {{ division = {division_id} target = {target} }}");
        if let Ok(b) = crate::parser::parse(&script) {
            let effs = crate::ast::lower::lower_effects(&b);
            interp.run(&effs, world);
        }
    });
}

/// 停止师的主动行动(进军/移动/支援); 保留被动防守和撤退
#[no_mangle]
pub extern "C" fn engine_stop_order(division_id: u32) {
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Engine { interp, world } = &mut *e;
        let script = format!("stop_order = {{ division = {division_id} }}");
        if let Ok(b) = crate::parser::parse(&script) {
            let effs = crate::ast::lower::lower_effects(&b);
            interp.run(&effs, world);
        }
    });
}

/// 部署师到指定省(前端交互式部署)
#[no_mangle]
pub unsafe extern "C" fn engine_deploy_division(
    owner_ptr: *const u8, owner_len: usize,
    location: u32,
    equip_ptr: *const u8, equip_len: usize,
    battalions: u32,
) {
    let owner = unsafe { ptr_to_str(owner_ptr, owner_len) };
    let equip = unsafe { ptr_to_str(equip_ptr, equip_len) };
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Engine { interp, world } = &mut *e;
        let script = format!(
            "create_division = {{ owner = {owner} location = {location} equipment = {equip} battalions = {battalions} }}"
        );
        if let Ok(b) = crate::parser::parse(&script) {
            let effs = crate::ast::lower::lower_effects(&b);
            interp.run(&effs, world);
        }
    });
}

/// 补充装备到国家仓库
#[no_mangle]
pub unsafe extern "C" fn engine_add_equipment(
    owner_ptr: *const u8, owner_len: usize,
    equip_ptr: *const u8, equip_len: usize,
    amount: u32,
) {
    let owner = unsafe { ptr_to_str(owner_ptr, owner_len) };
    let equip = unsafe { ptr_to_str(equip_ptr, equip_len) };
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Engine { interp, world } = &mut *e;
        let script = format!("add_equipment = {{ owner = {owner} type = {equip} amount = {amount} }}");
        if let Ok(b) = crate::parser::parse(&script) {
            let effs = crate::ast::lower::lower_effects(&b);
            interp.run(&effs, world);
        }
    });
}

/// 补充人力到国家池
#[no_mangle]
pub unsafe extern "C" fn engine_add_manpower(
    owner_ptr: *const u8, owner_len: usize,
    amount: u32,
) {
    let owner = unsafe { ptr_to_str(owner_ptr, owner_len) };
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Engine { interp, world } = &mut *e;
        let script = format!("add_manpower = {{ owner = {owner} amount = {amount} }}");
        if let Ok(b) = crate::parser::parse(&script) {
            let effs = crate::ast::lower::lower_effects(&b);
            interp.run(&effs, world);
        }
    });
}

/// 自动部署: 一键给某方补满装备+人力(部署时用)
#[no_mangle]
pub unsafe extern "C" fn engine_supply(owner_ptr: *const u8, owner_len: usize) {
    let owner = unsafe { ptr_to_str(owner_ptr, owner_len) };
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Engine { interp, world } = &mut *e;
        // 自动补足装备(各5000)和人力(50000), 简化部署
        let script = format!(
            "add_equipment = {{ owner = {owner} type = infantry_equipment amount = 5000 }}
            add_equipment = {{ owner = {owner} type = medium_tank amount = 5000 }}
            add_manpower = {{ owner = {owner} amount = 500000 }}"
        );
        if let Ok(b) = crate::parser::parse(&script) {
            let effs = crate::ast::lower::lower_effects(&b);
            interp.run(&effs, world);
        }
    });
}

/// 运行 setup 脚本(建师/开战等)。返回 0 成功, 非 0 失败
#[no_mangle]
pub unsafe extern "C" fn engine_run_setup(script_ptr: *const u8, script_len: usize) -> u32 {
    let script = unsafe { ptr_to_str(script_ptr, script_len) };
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let block = match parse(script) {
            Ok(b) => b,
            Err(_) => return 1,
        };
        // 取 _setup 块(或顶层直接用)
        let target = block.fields.iter().find(|f| f.key == "_setup");
        let effs = match target {
            Some(f) => match &f.value {
                Value::Block(b) => lower_effects(b),
                _ => return 2,
            },
            None => lower_effects(&block),
        };
        let Engine { interp, world } = &mut *e;
        interp.run(&effs, world);
        0
    })
}

/// 推进 n 小时
#[no_mangle]
pub extern "C" fn engine_tick(hours: u32) {
    ENGINE.with(|e| {
        let mut e = e.borrow_mut();
        let Engine { interp, world } = &mut *e;
        // 用 GameClock::tick 保证主循环完整(战斗/行军/恢复/增援 全调用)
        // 之前内联版本漏了 advance_movement/recover_org, 导致浏览器撤退卡住
        crate::runtime::GameClock::advance(interp, world, hours as u64);
    })
}

/// 取当前世界状态(JSON, null 终止)。返回指针。
/// 内存由 wasm 线性内存持有; JS 应立即拷贝, 下次调用会覆盖。
#[no_mangle]
pub extern "C" fn engine_get_state() -> *const u8 {
    let json = ENGINE.with(|e| serialize_state(&e.borrow().world));
    // 存到静态缓冲, 追加 null 终止符
    STATE_BUF.with(|buf| {
        *buf.borrow_mut() = json.into_bytes();
        let mut b = buf.borrow_mut();
        b.push(0); // null 终止
        b.as_ptr()
    })
}

thread_local! {
    static STATE_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

/// 序列化世界状态为 JSON(手写, 无 serde)
fn serialize_state(world: &World) -> String {
    let mut s = String::from("{\"hour\":");
    s.push_str(&world.hour.to_string());
    s.push_str(",\"player\":\"");
    s.push_str(&world.player_tag);
    s.push_str("\",\"divisions\":[");
    let mut first = true;
    for d in world.divisions.values() {
        if !first {
            s.push(',');
        }
        first = false;
        // enum 拍平为原 JSON 键(JS 端零改动)
        use crate::runtime::entities::OrderState;
        let (dest, pending, progress, supporting, attacking, retreating) = match &d.order {
            OrderState::Idle => (0u32, 0u32, 0.0, 0u32, false, false),
            OrderState::Moving { dest, progress, hostile, .. } => (*dest, 0, *progress, 0, *hostile, false),
            OrderState::Retreating { dest, progress } => (*dest, 0, *progress, 0, false, true),
            OrderState::Pending { dest, .. } => (0, *dest, 0.0, 0, false, false),
            OrderState::Supporting { target } => (0, 0, 0.0, *target, false, false),
        };
        s.push_str(&format!(
            "{{\"id\":{},\"owner\":\"{}\",\"org\":{:.1},\"max_org\":{:.0},\"str\":{:.1},\"max_str\":{:.0},\"eq_ratio\":{:.2},\"mp_ratio\":{:.2},\"loc\":{},\"dest\":{},\"pending\":{},\"progress\":{:.3},\"supporting\":{},\"attacking\":{},\"retreating\":{},\"annihilated\":{}}}",
            d.id, d.owner_tag, d.org, d.max_org, d.strength, d.max_strength,
            d.equipment_ratio_only(), d.manpower_ratio(),
            d.location_province,
            dest, pending, progress, supporting,
            attacking, retreating, d.is_annihilated()
        ));
    }
    s.push_str("],\"battles\":[");
    let mut bfirst = true;
    for b in &world.battles {
        if !bfirst { s.push(','); }
        bfirst = false;
        let ids = |v: &[u64]| -> String {
            v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
        };
        s.push_str(&format!(
            "{{\"id\":{},\"prov\":{},\"atk\":[{}],\"def\":[{}],\"res_atk\":[{}],\"res_def\":[{}]}}",
            b.id, b.province,
            ids(&b.attackers), ids(&b.defenders),
            ids(&b.reserve_attackers), ids(&b.reserve_defenders)
        ));
    }
    s.push_str("]");
    // 省份(节点图用: id/controller/neighbors)
    s.push_str(",\"provinces\":[");
    let mut pfirst = true;
    for p in world.provinces.values() {
        if !pfirst { s.push(','); }
        pfirst = false;
        s.push_str(&format!(
            "{{\"id\":{},\"controller\":\"{}\",\"neighbors\":[",
            p.id, p.controller
        ));
        let mut nfirst = true;
        for n in &p.neighbors {
            if !nfirst { s.push(','); }
            nfirst = false;
            s.push_str(&n.to_string());
        }
        s.push_str("]}");
    }
    s.push_str("]}");
    s
}

/// 把 JS 传入的 (ptr, len) 转 &str。
/// 安全契约: JS 必须保证 ptr 指向的内存在本次调用期间有效且是合法 UTF-8。
unsafe fn ptr_to_str<'a>(ptr: *const u8, len: usize) -> &'a str {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(bytes).unwrap_or("")
}
