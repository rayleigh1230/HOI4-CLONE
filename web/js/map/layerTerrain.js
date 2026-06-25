// 图层0: 地形底色 + 省界(邻接虚线)
import { provincePos } from './layout.js';

let _log = false;
export function draw(ctx, view, { worldToScreen, W, H }) {
  if (!view) { console.log('[terrain] view is null'); return; }
  const { provinces } = view;
  if (!provinces?.length) { if (!_log) { console.log('[terrain] no provinces, view keys:', Object.keys(view)); _log = true; } return; }
  if (!_log) { console.log('[terrain] provinces:', provinces.length, 'first:', provinces[0]); _log = true; }

  const ids = provinces.map(p => p.id);

  // 邻接虚线
  ctx.strokeStyle = '#0f3460';
  ctx.lineWidth = 1.5;
  ctx.setLineDash([4, 4]);
  for (const p of provinces) {
    const a = worldToScreen(provincePos(p.id, ids, W, H));
    for (const n of p.neighbors) {
      const nb = provinces.find(x => x.id === n);
      if (!nb) continue;
      const b = worldToScreen(provincePos(n, ids, W, H));
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.stroke();
    }
  }
  ctx.setLineDash([]);

  // 临时: 画一个大绿圆确认本层执行
  ctx.strokeStyle = '#00ff00';
  ctx.lineWidth = 3;
  ctx.beginPath();
  ctx.arc(W/2, H/2, 100, 0, Math.PI*2);
  ctx.stroke();
}
