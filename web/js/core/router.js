// 面板路由: 注册 → 打开(左侧滑入, 带关闭按钮) → 关闭(滑出)。
// 对齐原版 country*view 统一滑入协议 + 用户反馈问题4(面板需可关闭)。
import { h, clear, prepend } from '../core/el.js';

const panels = new Map();  // name → { open(), close() }
let current = null;

const host = () => document.getElementById('panel-host');

export function register(name, panel) {
  panels.set(name, panel);
}

export function open(name) {
  if (current && panels.has(current)) {
    try { panels.get(current).close(); } catch (err) { console.error('[router] close error:', err); }
  }
  current = name;
  const p = panels.get(name);
  if (p) {
    host().classList.add('open');
    try { p.open(); } catch (err) { console.error('[router] open error:', err); }
    // 在面板内容顶部注入关闭按钮(面板 open() 已填充内容后, prepend 关闭条)
    prepend(host(), h('div', { class: 'panel-close-bar' }, [
      h('span', { class: 'panel-title', text: name }),
      h('button', { class: 'panel-close', onclick: close, text: '✖️' }),
    ]));
  }
}

export function close() {
  if (current && panels.has(current)) {
    try { panels.get(current).close(); } catch (err) { console.error('[router] close error:', err); }
  }
  current = null;
  host().classList.remove('open');
}

export function names() {
  return [...panels.keys()];
}
