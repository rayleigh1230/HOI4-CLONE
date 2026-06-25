// 顶栏: 日期 + 系统按钮组(按 router 注册表生成) + [切控制权]测试按钮。
// 对齐 spec §7.1: 时间控制移到底部命令栏(bottombar), 顶栏不再含时间按钮。
import { h, clear } from '../core/el.js';
import { subscribeKeys } from '../core/store.js';
import { open as openPanel, names as panelNames } from '../core/router.js';

let controlMode = false;
window._controlMode = false;

export function isControlMode() { return controlMode; }

export function render() {
  const el = document.getElementById('topbar');
  clear(el);

  // 日期显示(路径订阅 date: 仅日期变时才更新文本)
  const dateLabel = h('span', { class: 'topbar-date' }, '📅 --');
  subscribeKeys(['date'], (state) => {
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

  // [切控制权]测试按钮(上帝模式, spec §7.2: 移出正式手势, 独立按钮)
  el.append(h('button', {
    class: 'secondary',
    style: { background: controlMode ? '#e94560' : '#0f3460' },
    onclick() { controlMode = !controlMode; window._controlMode = controlMode; render(); },
    text: controlMode ? '切换中...' : '切控制权',
  }));
}
