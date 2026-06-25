// 命令封装: 把 WASM FFI 包成语义化 JS 函数
import { getWasm, passStr } from './wasm.js';
const e = () => getWasm().exports;

// === 时间/重置 ===
export function tick(h) { e().engine_tick(h); }
export function reset() { e().engine_reset(); }
export function setPlayer(tag) {
  const t = passStr(tag); e().engine_set_player(t.ptr, t.len);
}

// === 场景/脚本 ===
export function runSetup(script) {
  const s = passStr(script); return e().engine_run_setup(s.ptr, s.len);
}
export function setProvinceController(provId, tag) {
  const t = passStr(tag); e().engine_set_province_controller(provId, t.ptr, t.len);
}
export function supply(owner) {
  const o = passStr(owner); e().engine_supply(o.ptr, o.len);
}

// === 建师/换模板 ===
export function deployTemplate(owner, loc, template) {
  const o = passStr(owner), t = passStr(template);
  e().engine_deploy_template(o.ptr, o.len, loc, t.ptr, t.len);
}
export function changeTemplate(divId, template) {
  const t = passStr(template);
  e().engine_change_template(divId, t.ptr, t.len);
}

// === 移动/下令 ===
export function moveDivision(divId, target) { e().engine_move_division(divId, target); }
export function supportAttack(divId, target) { e().engine_support_attack(divId, target); }
export function queueMove(divId, target) { e().engine_queue_move(divId, target); }
export function stopOrder(divId) { e().engine_stop_order(divId); }

// === 外交(宣战/阵营/和谈) ===
export function declareWar(attacker, defender) {
  const a = passStr(attacker), d = passStr(defender);
  e().engine_declare_war(a.ptr, a.len, d.ptr, d.len);
}
export function createFaction(tag, name) {
  const t = passStr(tag), n = passStr(name);
  e().engine_create_faction(t.ptr, t.len, n.ptr, n.len);
}
export function joinFaction(tag, name) {
  const t = passStr(tag), n = passStr(name);
  e().engine_join_faction(t.ptr, t.len, n.ptr, n.len);
}
export function whitePeace(a, b) {
  const ta = passStr(a), tb = passStr(b);
  e().engine_white_peace(ta.ptr, ta.len, tb.ptr, tb.len);
}
