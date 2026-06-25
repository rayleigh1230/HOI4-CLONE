// 顶栏: 日期 + 系统按钮组(按 router 注册表生成) + [切控制权]测试按钮 + 时间控制
import { h, clear } from '../core/el.js';
import { subscribe } from '../core/store.js';
import { open as openPanel, names as panelNames } from '../core/router.js';
import { doTick, toggleTime } from '../main.js';
import { setProvinceController } from '../engine/commands.js';

let controlMode = false;

export function isControlMode() { return controlMode; }

export function render() {
  const el = document.getElementById('topbar');
  clear(el);

  // 日期显示(绑定 date 数据)
  const dateLabel = h('span', { class: 'topbar-date' });
  subscribe((state) => {
    const d = state?.date;
    if (d) dateLabel.textContent = `📅 ${d.y}.${d.m}.${d.d}`;
  });
  el.append(dateLabel);

  // 系统按钮(按已注册面板生成)
  for (const name of panelNames()) {
    el.append(h('button', { class: 'secondary', onclick: () => openPanel(name) }, name));
  }

  // 分隔
  el.append(h('span', { style: { flex: 1 } }));

  // 时间控制按钮
  const tick1Btn = h('button', { class: 'secondary', onclick: () => doTick(1) }, '▶');
  const tick24Btn = h('button', { class: 'secondary', onclick: () => doTick(24) }, '⏩1日');
  const autoBtn = h('button', { class: 'secondary', onclick() {
    toggleTime(); autoBtn.textContent = toggleTime() ? '⏸' : '▶流逝';
  } }, '▶流逝');
  el.append(tick1Btn, tick24Btn, autoBtn);

  // [切控制权]测试按钮(上帝模式)
  el.append(h('button', {
    class: 'secondary',
    style: { background: controlMode ? '#e94560' : '#0f3460' },
    onclick() { controlMode = !controlMode; render(); },
    text: controlMode ? '切换中...' : '切控制权',
  }));
}
