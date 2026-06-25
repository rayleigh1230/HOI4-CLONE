// 图层3: 命令箭头(进攻/行军/支援/航点, 多段折线+车道偏移)
import { provincePos } from './layout.js';

export function draw(ctx, view, { worldToScreen, W, H }) {
  const { divisions, provinces } = view;
  if (!provinces?.length || !divisions) return;

  const ids = provinces.map(p => p.id);
  const pos = {};
  for (const p of provinces) pos[p.id] = worldToScreen(provincePos(p.id, ids, W, H));

  for (const d of divisions) {
    const bx = pos[d.loc]?.x || 0;
    const by = pos[d.loc]?.y || 0;

    // 多段行军箭头(沿用旧算法: 完整折线 + 车道偏移 + 进度尾端)
    const pathArr = Array.isArray(d.path) ? d.path : [];
    const LANE = 14;

    if (pathArr.length >= 2) {
      const pts = pathArr.map(p => pos[p]);
      if (pts.every(Boolean)) {
        const arrowColor = d.retreating ? '#888888' : (d.attacking ? '#e94560' : '#4cd964');

        // 无向边计数(重合边检测, 用于车道偏移)
        const undirectedKey = (a, b) => a < b ? `${a}-${b}` : `${b}-${a}`;
        const edgeCount = {};
        for (let i = 0; i < pathArr.length - 1; i++) {
          const k = undirectedKey(pathArr[i], pathArr[i + 1]);
          edgeCount[k] = (edgeCount[k] || 0) + 1;
        }
        const segNormal = (a, b) => {
          const dx = b.x - a.x, dy = b.y - a.y;
          const len = Math.hypot(dx, dy) || 1;
          return { nx: dy / len, ny: -dx / len };
        };
        const segOffsetVec = (i) => {
          const k = undirectedKey(pathArr[i], pathArr[i + 1]);
          if ((edgeCount[k] || 0) < 2) return { ox: 0, oy: 0 };
          const n = segNormal(pts[i], pts[i + 1]);
          return { ox: n.nx * LANE, oy: n.ny * LANE };
        };
        const lanePts = pts.map((p, i) => {
          let ox, oy;
          if (i === 0) {
            ({ ox, oy } = segOffsetVec(0));
          } else if (i === pts.length - 1) {
            ({ ox, oy } = segOffsetVec(i - 1));
          } else {
            const o1 = segOffsetVec(i - 1), o2 = segOffsetVec(i);
            ox = (o1.ox + o2.ox) / 2;
            oy = (o1.oy + o2.oy) / 2;
          }
          return { x: p.x + ox, y: p.y + oy };
        });

        const prog = Math.max(0, Math.min(1, d.progress || 0));
        const tailX = lanePts[0].x + (lanePts[1].x - lanePts[0].x) * prog;
        const tailY = lanePts[0].y + (lanePts[1].y - lanePts[0].y) * prog;

        ctx.save();
        ctx.strokeStyle = arrowColor;
        ctx.lineWidth = 10;
        ctx.lineCap = 'round';
        ctx.lineJoin = 'round';
        ctx.setLineDash([]);
        ctx.beginPath();
        ctx.moveTo(tailX, tailY);
        for (let i = 1; i < lanePts.length; i++) {
          ctx.lineTo(lanePts[i].x, lanePts[i].y);
        }
        ctx.stroke();

        // 箭头头
        const lastFrom = lanePts[lanePts.length - 2];
        const lastTo = lanePts[lanePts.length - 1];
        const ang = Math.atan2(lastTo.y - lastFrom.y, lastTo.x - lastFrom.x);
        const head = 20;
        ctx.fillStyle = arrowColor;
        ctx.beginPath();
        ctx.moveTo(lastTo.x, lastTo.y);
        ctx.lineTo(lastTo.x - head * Math.cos(ang - 0.4), lastTo.y - head * Math.sin(ang - 0.4));
        ctx.lineTo(lastTo.x - head * 0.5 * Math.cos(ang), lastTo.y - head * 0.5 * Math.sin(ang));
        ctx.lineTo(lastTo.x - head * Math.cos(ang + 0.4), lastTo.y - head * Math.sin(ang + 0.4));
        ctx.closePath();
        ctx.fill();
        ctx.restore();
      }
    } else if (d.supporting && pos[d.supporting]) {
      // 支援攻击: 蓝色虚线箭头
      const tx = pos[d.supporting].x, ty = pos[d.supporting].y;
      ctx.strokeStyle = '#3a86ff';
      ctx.lineWidth = 3;
      ctx.setLineDash([6, 3]);
      ctx.beginPath();
      ctx.moveTo(bx, by);
      ctx.lineTo(tx, ty);
      ctx.stroke();
      ctx.setLineDash([]);
      const ang = Math.atan2(ty - by, tx - bx);
      const head = 14;
      ctx.fillStyle = '#3a86ff';
      ctx.beginPath();
      ctx.moveTo(tx, ty);
      ctx.lineTo(tx - head * Math.cos(ang - 0.4), ty - head * Math.sin(ang - 0.4));
      ctx.lineTo(tx - head * 0.5 * Math.cos(ang), ty - head * 0.5 * Math.sin(ang));
      ctx.lineTo(tx - head * Math.cos(ang + 0.4), ty - head * Math.sin(ang + 0.4));
      ctx.closePath();
      ctx.fill();
    }
  }
}
