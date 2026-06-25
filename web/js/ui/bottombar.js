// 底部命令栏: 时间控制(对齐 spec §7.1 — 时间大按钮放底部, 手指可达)。
// 单击推进 / 自动流逝 / 停止。状态在按钮上反馈。
import { h, clear } from '../core/el.js';
import { doTick, toggleTime } from '../main.js';

let autoBtn = null;
let running = false;

export function render() {
  const el = document.getElementById('bottombar');
  clear(el);
  el.append(
    h('button', { class: 'secondary', title: '推进 1 小时', onclick: () => doTick(1) }, '⏱ +1时'),
    h('button', { class: 'secondary', title: '推进 1 天', onclick: () => doTick(24) }, '⏩ 1日'),
    (autoBtn = h('button', {
      title: '自动流逝',
      onclick() { running = toggleTime(); autoBtn.textContent = running ? '⏸ 暂停' : '▶ 流逝'; },
    }, '▶ 流逝')),
  );
}

// 外部可调用以同步自动流逝按钮状态(如重置后)
export function syncRunning(state) {
  running = state;
  if (autoBtn) autoBtn.textContent = running ? '⏸ 暂停' : '▶ 流逝';
}
