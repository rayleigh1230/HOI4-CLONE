// 图层0: 地形底图。按 terrain 填充多边形, 替代全黑背景。对齐 spec §3.1。
import { PROVINCES, TERRAIN_COLORS, provincePoly } from './layout.js';

// offscreen 静态纹理(噪点), 只在尺寸变化时重新生成, 避免每帧重算
let textureCanvas = null;
function getTexture(W, H) {
  if (textureCanvas && textureCanvas.width === W && textureCanvas.height === H) return textureCanvas;
  textureCanvas = document.createElement('canvas');
  textureCanvas.width = W; textureCanvas.height = H;
  const tctx = textureCanvas.getContext('2d');
  const img = tctx.createImageData(W, H);
  for (let i = 0; i < img.data.length; i += 4) {
    const n = Math.random() * 18;
    img.data[i] = n; img.data[i + 1] = n; img.data[i + 2] = n;
    img.data[i + 3] = 26;  // 低透明度噪点, 增质感
  }
  tctx.putImageData(img, 0, 0);
  return textureCanvas;
}

export function draw(ctx, view, { worldToScreen, camera, W, H }) {
  // 整屏先填深色(多边形之外 = 海洋/地图边框)
  ctx.fillStyle = '#0a1a2a';
  ctx.fillRect(0, 0, W, H);

  // 填充每个省多边形(地形色)
  const provinceIds = (view.provinces || []).map(p => p.id);
  for (const id of provinceIds) {
    const poly = provincePoly(id);
    if (!poly) continue;
    ctx.beginPath();
    for (let i = 0; i < poly.length; i++) {
      const s = worldToScreen({ x: poly[i][0], y: poly[i][1] });
      if (i === 0) ctx.moveTo(s.x, s.y); else ctx.lineTo(s.x, s.y);
    }
    ctx.closePath();
    const terrain = PROVINCES[id]?.terrain || 'plains';
    ctx.fillStyle = TERRAIN_COLORS[terrain] || '#3a5a40';
    ctx.fill();
  }

  // 叠噪点纹理(低透明度, 不遮挡地形色)
  const tex = getTexture(W, H);
  if (tex) ctx.drawImage(tex, 0, 0);
}
