// 顶栏: 日期 + 系统按钮组 + [切视角]按钮(切换控制的国家)。
// 对齐 spec §7.1 + 国家视角: 时间控制在 bottombar; 切视角切 player_tag(原版控制台 tag)。
import { h, clear } from '../core/el.js';
import { subscribeKeys } from '../core/store.js';
import { open as openPanel, names as panelNames } from '../core/router.js';
import { setPlayer } from '../engine/commands.js';
import { refresh } from '../main.js';

const NATIONS = ['GER', 'FRA'];

export function render() {
  const el = document.getElementById('topbar');
  clear(el);

  // 日期 + 当前视角(路径订阅)
  const dateLabel = h('span', { class: 'topbar-date' }, '📅 --');
  let currentNation = '';
  subscribeKeys(['date', 'player'], (state) => {
    const d = state?.date;
    if (d) dateLabel.textContent = `📅 ${d.y}.${d.m}.${d.d}`;
    currentNation = state?.player || '';
    // 视角标签更新
    if (viewLabel) viewLabel.textContent = `👁 ${currentNation}`;
  });
  el.append(dateLabel);

  // 当前视角标签
  let viewLabel = h('span', { class: 'topbar-view' }, `👁 ${currentNation}`);
  el.append(viewLabel);

  // 系统按钮(按已注册面板生成)
  for (const name of panelNames()) {
    el.append(h('button', { class: 'secondary', onclick: () => openPanel(name) }, name));
  }

  // 分隔
  el.append(h('span', { style: { flex: 1 } }));

  // [切视角]按钮: 点击弹出 GER/FRA 选择, 切换 player_tag(模拟原版控制台 tag 切换)。
  el.append(h('button', {
    class: 'secondary',
    onclick() { showNationSwitcher(currentNation); },
    text: '🔀 切视角',
  }));
}

// 弹出国家选择(切换控制视角)
function showNationSwitcher(current) {
  const el = document.getElementById('order-menu');
  el.innerHTML = '';
  el.append(h('div', { class: 'order-title', text: '切换到哪个国家?' }));
  for (const tag of NATIONS) {
    el.append(h('button', {
      class: tag === current ? '' : 'secondary',
      onclick() {
        setPlayer(tag);
        el.classList.remove('open');
        refresh();
      },
    }, `${tag === current ? '✓ ' : ''}${tag}`));
  }
  el.append(h('button', { class: 'secondary', onclick: () => el.classList.remove('open') }, '取消'));
  el.classList.add('open');
}
