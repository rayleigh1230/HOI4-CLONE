// 下令菜单(选师后点省弹: 进军/航点/支援, 底部弹出)
import { h, clear } from '../core/el.js';
import { moveDivision, queueMove, supportAttack } from '../engine/commands.js';
import { log } from '../main.js';

let pending = null;  // { divId, targetProv }

export function show(divId, targetProv) {
  pending = { divId, targetProv };
  const el = document.getElementById('order-menu');
  clear(el);
  el.append(
    h('button', { onclick() { moveDivision(divId, targetProv); log(`师#${divId} 进军→省${targetProv}`); hide(); } }, '⚔️ 进军攻击'),
    h('button', { onclick() { queueMove(divId, targetProv); log(`师#${divId} 追加航点→省${targetProv}`); hide(); } }, '➕ 追加航点'),
    h('button', { onclick() { supportAttack(divId, targetProv); log(`师#${divId} 支援→省${targetProv}`); hide(); } }, '🎯 支援攻击'),
    h('button', { class: 'secondary', onclick: hide }, '✖️ 取消'),
  );
  el.classList.add('open');
}

export function hide() {
  document.getElementById('order-menu').classList.remove('open');
  pending = null;
}
