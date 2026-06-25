// 图层2: 完整 NATO 76×24 部队牌。对齐 mapicons.gui:unit_counter。
// 兵种符号 + org/str竖条 + 数量 + 国旗色边框。同省多师 = 错位叠放牌堆(视觉看出堆叠)。
// 选中师 = 金色高亮边。对齐 spec §4 + 用户反馈问题1/5。
import { provinceCentroid, TAG_COLORS } from './layout.js';

const SYMBOLS = { infantry: '▦', armor: '◆', artillery: '◎', mechanized: '◇' };

function unitSymbol(template) {
  if (!template) return SYMBOLS.infantry;
  const t = template.toLowerCase();
  if (t.includes('panzer') || t.includes('armor') || t.includes('blind')) return SYMBOLS.armor;
  if (t.includes('artiller')) return SYMBOLS.artillery;
  if (t.includes('mecan') || t.includes('mech') || t.includes('motor')) return SYMBOLS.mechanized;
  return SYMBOLS.infantry;
}

// 选中态(单师选中, 金色高亮)。main.js 调 selectDivision。
let selectedDiv = null;
export function selectDivision(id) { selectedDiv = id; }
export function getSelectedDivision() { return selectedDiv; }
export function clearSelection() { selectedDiv = null; }

// 画一张 NATO 牌。cx,cy = 牌子中心屏幕坐标。
// division = 单个师; battles = 战斗列表; zoom = 缩放。
function drawCard(ctx, cx, cy, division, battles, zoom, isSelected) {
  const w = 76 * zoom, h = 24 * zoom;
  const x = cx - w / 2, y = cy - h / 2;
  const owner = division.owner;
  const color = TAG_COLORS[owner] || '#fff';

  // 牌子背景 + 国旗色边框
  ctx.fillStyle = 'rgba(10,10,26,0.88)';
  ctx.fillRect(x, y, w, h);
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.5;
  ctx.strokeRect(x, y, w, h);

  // 兵种符号(左)
  ctx.fillStyle = color;
  ctx.font = `bold ${14 * zoom}px sans-serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(unitSymbol(division.template), x + 11 * zoom, cy);

  // org/str 竖条
  const orgPct = division.max_org > 0 ? division.org / division.max_org : 0;
  const strPct = division.max_str > 0 ? division.str / division.max_str : 0;
  const barH = h - 6 * zoom, barTop = y + 3 * zoom;
  ctx.fillStyle = '#2a3a2a'; ctx.fillRect(x + 26 * zoom, barTop, 4 * zoom, barH);
  ctx.fillStyle = '#4cd964'; ctx.fillRect(x + 26 * zoom, barTop + barH * (1 - orgPct), 4 * zoom, barH * orgPct);
  ctx.fillStyle = '#3a2a2a'; ctx.fillRect(x + 33 * zoom, barTop, 4 * zoom, barH);
  ctx.fillStyle = '#ff6b6b'; ctx.fillRect(x + 33 * zoom, barTop + barH * (1 - strPct), 4 * zoom, barH * strPct);

  // 数量徽章(右)
  ctx.fillStyle = '#fff';
  ctx.font = `bold ${11 * zoom}px sans-serif`;
  ctx.fillText('1', x + 52 * zoom, cy);

  // 战斗中 → 红色脉冲边
  const inCombat = battles?.some(b => b.atk?.includes(division.id) || b.def?.includes(division.id));
  if (inCombat) {
    ctx.strokeStyle = '#ff3030'; ctx.lineWidth = 2.5;
    ctx.strokeRect(x - 1, y - 1, w + 2, h + 2);
  }
  // 选中 → 金色高亮边(最上层, 粗)
  if (isSelected) {
    ctx.strokeStyle = '#ffd700'; ctx.lineWidth = 3;
    ctx.strokeRect(x - 2, y - 2, w + 4, h + 4);
  }
  // 撤退 → 灰蒙版
  if (division.retreating) {
    ctx.fillStyle = 'rgba(100,100,100,0.4)';
    ctx.fillRect(x, y, w, h);
  }
  ctx.textBaseline = 'alphabetic';
}

export function draw(ctx, view, { worldToScreen, camera }) {
  const { divisions, provinces, battles } = view;
  if (!provinces?.length || !divisions) return;
  const zoom = camera.zoom;

  // 按省聚合
  const byProv = {};
  for (const d of divisions) {
    if (!byProv[d.loc]) byProv[d.loc] = [];
    byProv[d.loc].push(d);
  }

  for (const provId in byProv) {
    const divs = byProv[provId];
    const c = provinceCentroid(Number(provId));
    if (!c) continue;
    const sc = worldToScreen(c);
    const baseY = sc.y - 28 * zoom;

    if (divs.length === 1) {
      // 单师: 一张牌居中
      drawCard(ctx, sc.x, baseY, divs[0], battles, zoom, divs[0].id === selectedDiv);
    } else {
      // 多师: 错位叠放牌堆(底层牌往右下偏移露出, 最多显示 3 张, 其余聚合数字)
      const stackOffset = 7 * zoom;
      const showN = Math.min(divs.length, 3);
      // 先画底层(倒序, 后画的在上), 偏移量随层数增加
      for (let i = showN - 1; i >= 0; i--) {
        const off = i * stackOffset;
        drawCard(ctx, sc.x + off, baseY + off, divs[i], battles, zoom, divs[i].id === selectedDiv);
      }
      // 堆叠总数徽章(右上角, 醒目)
      ctx.fillStyle = '#ffd700';
      ctx.font = `bold ${10 * zoom}px sans-serif`;
      ctx.textAlign = 'center';
      ctx.fillText(`×${divs.length}`, sc.x + 40 * zoom, baseY - 18 * zoom + 8);
    }
  }
}
