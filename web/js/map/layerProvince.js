// 图层1: 政治着色。多边形描边 controller 色 + 淡填充 + 省号。对齐 spec §3.2。
// 选中高亮在 layerOverlay, 这里只画基础省。
import { provincePoly, provinceCentroid, TAG_COLORS } from './layout.js';

let selectedProvince = null;
export function selectProvince(id) { selectedProvince = id; }
export function getSelected() { return selectedProvince; }

function tracePath(ctx, poly, worldToScreen) {
  ctx.beginPath();
  for (let i = 0; i < poly.length; i++) {
    const s = worldToScreen({ x: poly[i][0], y: poly[i][1] });
    if (i === 0) ctx.moveTo(s.x, s.y); else ctx.lineTo(s.x, s.y);
  }
  ctx.closePath();
}

export function draw(ctx, view, { worldToScreen, camera, W, H }) {
  const { provinces } = view;
  if (!provinces?.length) return;

  for (const p of provinces) {
    const poly = provincePoly(p.id);
    if (!poly) continue;
    const color = TAG_COLORS[p.controller] || '#666';

    // 淡填充 controller 色(alpha≈0.18, 地形底色透出)
    tracePath(ctx, poly, worldToScreen);
    ctx.fillStyle = color + '2e';  // 0x2e ≈ 18% alpha
    ctx.fill();

    // 描边 controller 色
    tracePath(ctx, poly, worldToScreen);
    ctx.strokeStyle = color;
    ctx.lineWidth = 2;
    ctx.stroke();

    // 省号(重心)
    const c = provinceCentroid(p.id);
    if (c) {
      const sc = worldToScreen(c);
      ctx.fillStyle = '#fff';
      ctx.font = 'bold 13px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('省' + p.id, sc.x, sc.y + 4);
    }
  }
}
