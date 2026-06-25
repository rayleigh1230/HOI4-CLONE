// 图层0: 地形底色 + 省界(邻接虚线)
import { provincePos } from './layout.js';

export function draw(ctx, view, { worldToScreen, W, H }) {
  const { provinces } = view;
  if (!provinces?.length) return;

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
}
