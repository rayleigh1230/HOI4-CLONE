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
        // 从 &mut Engine 拆字段: interp 只读, world 可变
        let Engine { interp, world } = &mut *e;
        for _ in 0..hours {
            world.hour += 1;
            world.fire_event(interp, "on_hourly");
            crate::combat::resolve::resolve_all_battles(world);
        }
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
        s.push_str(&format!(
            "{{\"id\":{},\"owner\":\"{}\",\"org\":{:.2},\"max_org\":{:.2},\"str\":{:.2},\"max_str\":{:.2},\"eq_ratio\":{:.3},\"broken\":{}}}",
            d.id, d.owner_tag, d.org, d.max_org, d.strength, d.max_strength, d.equipment_ratio(), d.is_broken()
        ));
    }
    s.push_str("],\"battles\":");
    s.push_str(&world.battles.len().to_string());
    s.push('}');
    s
}

/// 把 JS 传入的 (ptr, len) 转 &str。
/// 安全契约: JS 必须保证 ptr 指向的内存在本次调用期间有效且是合法 UTF-8。
unsafe fn ptr_to_str<'a>(ptr: *const u8, len: usize) -> &'a str {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(bytes).unwrap_or("")
}
