// 图层1: 政治着色(按 controller 颜色) + 选中省金色高亮
import { provincePos, TAG_COLORS } from './layout.js';

let selectedProvince = null;

export function selectProvince(id) { selectedProvince = id; }
export function getSelected() { return selectedProvince; }

export function draw(ctx, view, { worldToScreen, W, H }) {
  const { provinces } = view;
  if (!provinces?.length) return;

  const ids = provinces.map(p => p.id);

  for (const p of provinces) {
    const c = worldToScreen(provincePos(p.id, ids, W, H));
    const color = TAG_COLORS[p.controller] || '#666';
    const isSel = p.id === selectedProvince;

    ctx.fillStyle = color + (isSel ? '66' : '33');
    ctx.strokeStyle = isSel ? '#ffd700' : color;
    ctx.lineWidth = isSel ? 3 : 2;
    ctx.beginPath();
    ctx.arc(c.x, c.y, 26, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();

    // 省号
    ctx.fillStyle = '#fff';
    ctx.font = '12px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('省' + p.id, c.x, c.y + 4);

    // controller tag
    ctx.fillStyle = color;
    ctx.font = '9px sans-serif';
    ctx.fillText(p.controller, c.x, c.y - 18);
  }
}
