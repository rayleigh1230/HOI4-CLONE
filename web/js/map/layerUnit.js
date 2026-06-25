// 图层2: NATO 部队牌(兵种符号 + org/str 迷你条 + 数量 + 国旗)
import { provincePos, TAG_COLORS } from './layout.js';

const SYMBOLS = { infantry: '▦', armor: '◆', artillery: '◎' };

export function draw(ctx, view, { worldToScreen, W, H }) {
  const { divisions, provinces } = view;
  if (!provinces?.length) return;

  const ids = provinces.map(p => p.id);

  // 按省份聚合部队
  const byProv = {};
  if (divisions) {
    for (const d of divisions) {
      if (!byProv[d.loc]) byProv[d.loc] = [];
      byProv[d.loc].push(d);
    }
  }

  for (const [provId, divs] of Object.entries(byProv)) {
    const pos = worldToScreen(provincePos(Number(provId), ids, W, H));
    const count = divs.length;

    // 只画前 3 个师的兵种符号, 超过的聚合显示 +N
    for (let i = 0; i < Math.min(count, 3); i++) {
      const d = divs[i];
      let sym = SYMBOLS.infantry;
      if (d.template && (d.template.includes('Panzer') || d.template.includes('Armor') || d.template.includes('Mécanique'))) {
        sym = SYMBOLS.armor;
      } else if (d.template && d.template.includes('Artiller')) {
        sym = SYMBOLS.artillery;
      }
      const ownerColor = TAG_COLORS[d.owner] || '#fff';
      ctx.fillStyle = ownerColor;
      ctx.font = 'bold 14px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(sym, pos.x - 20 + i * 14, pos.y + 32);
    }

    if (count > 3) {
      ctx.fillStyle = '#ffd700';
      ctx.font = 'bold 10px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('+' + (count - 3), pos.x + 25, pos.y + 32);
    }

    // 部队数量小标签
    if (count > 0) {
      ctx.fillStyle = '#ffd700';
      ctx.font = 'bold 10px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText('×' + count, pos.x, pos.y + 18);
    }
  }
}
