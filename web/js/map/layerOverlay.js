// 图层5: UI 覆盖层。选中省金色描边 + 前线脉冲 + 拖拽下令箭头 + 悬停省高亮。
import { provincePoly, provinceCentroid, TAG_COLORS } from './layout.js';
import { getSelected } from './layerProvince.js';
import { getDragOrder } from '../core/input.js';

// 前线脉冲动画相位(由 main.js rAF 更新)
export let frontPulse = 0;
export function setFrontPulse(v) { frontPulse = v; }

// 拖拽下令时悬停的目标省(金色高亮, 区别于 selectedProvince)
let hoverProvince = null;
export function setHoverProvince(id) { hoverProvince = id; }
export function getHoverProvince() { return hoverProvince; }

function tracePath(ctx, poly, worldToScreen) {
  ctx.beginPath();
  for (let i = 0; i < poly.length; i++) {
    const s = worldToScreen({ x: poly[i][0], y: poly[i][1] });
    if (i === 0) ctx.moveTo(s.x, s.y); else ctx.lineTo(s.x, s.y);
  }
  ctx.closePath();
}

// 画某省的描边高亮(给定颜色/线宽)
function highlightProvince(ctx, id, worldToScreen, color, width, blur) {
  const poly = provincePoly(id);
  if (!poly) return;
  ctx.save();
  ctx.strokeStyle = color;
  ctx.lineWidth = width;
  if (blur) { ctx.shadowColor = color; ctx.shadowBlur = blur; }
  tracePath(ctx, poly, worldToScreen);
  ctx.stroke();
  ctx.restore();
}

export function draw(ctx, view, { worldToScreen }) {
  const sel = getSelected();
  const { provinces } = view;

  // 拖拽目标省高亮(亮金, 比选中更醒目, 带强光晕) — 拖拽中实时跟随鼠标
  if (hoverProvince != null) {
    highlightProvince(ctx, hoverProvince, worldToScreen, '#ffd700', 4, 14);
  }
  // 选中省描边(金, 较细)
  if (sel != null && sel !== hoverProvince) {
    highlightProvince(ctx, sel, worldToScreen, '#ffd700', 3, 8);
    const c = provinceCentroid(sel);
    if (c) {
      const sc = worldToScreen(c);
      ctx.fillStyle = '#ffd700';
      ctx.font = 'bold 11px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(`◆ 省${sel}`, sc.x, sc.y - 34);
    }
  }

  // 前线脉冲
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

  // 拖拽下令箭头: 兵牌起点(世界) → 鼠标当前位置(屏幕)。input.getDragOrder 给起点屏幕坐标。
  const drag = getDragOrder();
  if (drag) {
    const from = drag.fromScreen;
    // from 是屏幕坐标(按下时记录), 鼠标当前位置由 input 在 onDragMove 时已更新到 dragOrder? 
    // 实际上 getDragOrder 只返回 fromScreen + fromDiv, 当前鼠标位置需 input 另存。
    // 这里用 drag.curScreen(input 拖动时更新)。见 input.js onMove。
    const to = drag.curScreen || from;
    ctx.save();
    ctx.strokeStyle = '#e94560';
    ctx.lineWidth = 4;
    ctx.setLineDash([]);
    ctx.beginPath();
    ctx.moveTo(from.x, from.y);
    ctx.lineTo(to.x, to.y);
    ctx.stroke();
    // 箭头头
    const ang = Math.atan2(to.y - from.y, to.x - from.x);
    const head = 16;
    ctx.fillStyle = '#e94560';
    ctx.beginPath();
    ctx.moveTo(to.x, to.y);
    ctx.lineTo(to.x - head * Math.cos(ang - 0.4), to.y - head * Math.sin(ang - 0.4));
    ctx.lineTo(to.x - head * 0.5 * Math.cos(ang), to.y - head * 0.5 * Math.sin(ang));
    ctx.lineTo(to.x - head * Math.cos(ang + 0.4), to.y - head * Math.sin(ang + 0.4));
    ctx.closePath();
    ctx.fill();
    ctx.restore();
  }
}
