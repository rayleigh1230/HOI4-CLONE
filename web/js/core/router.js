// 面板路由: 注册 → 打开(滑入) → 关闭(滑出)。原版 country*view 的统一滑入协议。
const panels = new Map();  // name → { open(), close() }
let current = null;

const host = () => document.getElementById('panel-host');

// 注册一个面板
export function register(name, panel) {
  panels.set(name, panel);
}

// 打开指定面板(滑入), 同时关闭当前面板
export function open(name) {
  if (current && panels.has(current)) {
    try { panels.get(current).close(); } catch (err) { console.error('[router] close error:', err); }
  }
  current = name;
  const p = panels.get(name);
  if (p) {
    host().classList.add('open');
    try { p.open(); } catch (err) { console.error('[router] open error:', err); }
  }
}

// 关闭当前面板(滑出)
export function close() {
  if (current && panels.has(current)) {
    try { panels.get(current).close(); } catch (err) { console.error('[router] close error:', err); }
  }
  current = null;
  host().classList.remove('open');
}

// 所有已注册面板名(供顶栏生成按钮)
export function names() {
  return [...panels.keys()];
}
