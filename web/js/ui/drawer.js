// 底部抽屉(点省/师弹部队信息 + 部署入口, 移动端主交互)
import { h, clear } from '../core/el.js';

export function open(contents) {
  const el = document.getElementById('drawer');
  clear(el);
  if (Array.isArray(contents)) contents.forEach(c => el.append(c));
  else if (contents) el.append(contents);
  el.classList.add('open');
}

export function close() {
  document.getElementById('drawer').classList.remove('open');
}
