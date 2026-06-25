// 图层4: 战斗气泡(交战省进度/ VS / 预备队)
import { provincePos, TAG_COLORS } from './layout.js';

export function draw(ctx, view, { worldToScreen, W, H }) {
  const { battles, provinces, divisions } = view;
  if (!battles?.length || !provinces?.length) return;

  const ids = provinces.map(p => p.id);
  const divMap = {};
  if (divisions) for (const d of divisions) divMap[d.id] = d;

  for (const b of battles) {
    const pos = worldToScreen(provincePos(b.prov, ids, W, H));

    // 战斗气泡背景
    const atkSide = b.atk?.[0] ? divMap[b.atk[0]]?.owner : null;
    const defSide = b.def?.[0] ? divMap[b.def[0]]?.owner : null;

    const bx = pos.x - 42, bW = 84;
    ctx.fillStyle = 'rgba(10,10,26,0.85)';
    ctx.strokeStyle = '#e94560';
    ctx.lineWidth = 2;
    ctx.fillRect(bx, pos.y + 36, bW, 34);
    ctx.strokeRect(bx, pos.y + 36, bW, 34);

    // VS + 部队数
    ctx.fillStyle = '#e94560';
    ctx.font = 'bold 11px sans-serif';
    ctx.textAlign = 'center';
    const atkCount = (b.atk?.length || 0) + (b.res_atk?.length || 0);
    const defCount = (b.def?.length || 0) + (b.res_def?.length || 0);
    ctx.fillText(`${atkSide || '?'} ${atkCount} VS ${defSide || '?'} ${defCount}`, pos.x, pos.y + 60);
  }
}
