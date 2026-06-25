// 图层5: UI 覆盖层。选中沿多边形金色描边 + 重心标签 + 前线脉冲。对齐 spec §3.3。
import { provincePoly, provinceCentroid, TAG_COLORS } from './layout.js';
import { getSelected } from './layerProvince.js';

// 前线脉冲动画相位(由 main.js 的 rAF 循环更新, 见 Task 9)
export let frontPulse = 0;
export function setFrontPulse(v) { frontPulse = v; }

function tracePath(ctx, poly, worldToScreen) {
  ctx.beginPath();
  for (let i = 0; i < poly.length; i++) {
    const s = worldToScreen({ x: poly[i][0], y: poly[i][1] });
    if (i === 0) ctx.moveTo(s.x, s.y); else ctx.lineTo(s.x, s.y);
  }
  ctx.closePath();
}

export function draw(ctx, view, { worldToScreen }) {
  const sel = getSelected();
  const { provinces } = view;

  // 选中省: 金色描边(沿多边形)
  if (sel != null && provinces?.length) {
    const poly = provincePoly(sel);
    if (poly) {
      const c = provinceCentroid(sel);
      if (c) {
        const sc = worldToScreen(c);
        ctx.save();
        ctx.strokeStyle = '#ffd700';
        ctx.lineWidth = 3.5;
        ctx.shadowColor = 'rgba(255,215,0,0.8)';
        ctx.shadowBlur = 10;
        tracePath(ctx, poly, worldToScreen);
        ctx.stroke();
        ctx.restore();
        // 标签
        ctx.fillStyle = '#ffd700';
        ctx.font = 'bold 11px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(`◆ 省${sel}`, sc.x, sc.y - 34);
      }
    }
  }

  // 前线: controller 不同的相邻省, 在两省重心连线中点画脉冲圆(简化, 精确共享边留后续)
  if (provinces?.length) {
    const pulseAlpha = 0.4 + 0.4 * Math.sin(frontPulse);
    ctx.save();
    ctx.strokeStyle = `rgba(233,69,96,${pulseAlpha})`;
    ctx.lineWidth = 3;
    for (const p of provinces) {
      const pColor = TAG_COLORS[p.controller];
      for (const nId of p.neighbors) {
        const nb = provinces.find(x => x.id === nId);
        if (!nb) continue;
        const nColor = TAG_COLORS[nb.controller];
        if (pColor && nColor && pColor !== nColor && p.id < nId) {
          // 两重心连线中点画脉冲圆(标记交战前线)
          const a = worldToScreen(provinceCentroid(p.id));
          const b = worldToScreen(provinceCentroid(nId));
          const mx = (a.x + b.x) / 2, my = (a.y + b.y) / 2;
          ctx.beginPath();
          ctx.arc(mx, my, 5 + 2 * Math.sin(frontPulse), 0, Math.PI * 2);
          ctx.stroke();
        }
      }
    }
    ctx.restore();
  }
}
