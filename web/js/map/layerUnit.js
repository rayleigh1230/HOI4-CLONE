// 图层2: 完整 NATO 76×24 部队牌。对齐 mapicons.gui:unit_counter。
// 兵种符号 + org/str竖条 + 数量 + 国旗色边框。同省多师 = 牌堆合并。对齐 spec §4。
import { provinceCentroid, TAG_COLORS } from './layout.js';

const SYMBOLS = { infantry: '▦', armor: '◆', artillery: '◎', mechanized: '◇' };

// 按 template 名推断兵种
function unitSymbol(template) {
  if (!template) return SYMBOLS.infantry;
  const t = template.toLowerCase();
  if (t.includes('panzer') || t.includes('armor') || t.includes('blind')) return SYMBOLS.armor;
  if (t.includes('artiller')) return SYMBOLS.artillery;
  if (t.includes('mecan') || t.includes('mech') || t.includes('motor')) return SYMBOLS.mechanized;
  return SYMBOLS.infantry;
}

// 画一张 NATO 牌(76×24 世界单位, 经缩放)。cx,cy = 牌子中心屏幕坐标。
// divisions = 该省所有师; battles = 战斗列表(战斗师描红)。
function drawCard(ctx, cx, cy, divisions, battles, zoom) {
  const w = 76 * zoom, h = 24 * zoom;  // 牌子尺寸随缩放调整
  const x = cx - w / 2, y = cy - h / 2;
  const owner = divisions[0].owner;
  const color = TAG_COLORS[owner] || '#fff';

  // 牌子背景 + 国旗色边框
  ctx.fillStyle = 'rgba(10,10,26,0.85)';
  ctx.fillRect(x, y, w, h);
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.5;
  ctx.strokeRect(x, y, w, h);

  // 兵种符号(左)
  const sym = unitSymbol(divisions[0].template);
  ctx.fillStyle = color;
  ctx.font = `bold ${14 * zoom}px sans-serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(sym, x + 11 * zoom, cy);

  // org/str 竖条(取堆内平均)
  const avgOrg = divisions.reduce((s, d) => s + (d.org / Math.max(d.max_org, 1)), 0) / divisions.length;
  const avgStr = divisions.reduce((s, d) => s + (d.str / Math.max(d.max_str, 1)), 0) / divisions.length;
  const barH = h - 6 * zoom, barTop = y + 3 * zoom;
  // org 条(绿)
  ctx.fillStyle = '#2a3a2a';
  ctx.fillRect(x + 26 * zoom, barTop, 4 * zoom, barH);
  ctx.fillStyle = '#4cd964';
  ctx.fillRect(x + 26 * zoom, barTop + barH * (1 - avgOrg), 4 * zoom, barH * avgOrg);
  // str 条(红)
  ctx.fillStyle = '#3a2a2a';
  ctx.fillRect(x + 33 * zoom, barTop, 4 * zoom, barH);
  ctx.fillStyle = '#ff6b6b';
  ctx.fillRect(x + 33 * zoom, barTop + barH * (1 - avgStr), 4 * zoom, barH * avgStr);

  // 数量(右, 堆叠数)
  ctx.fillStyle = '#fff';
  ctx.font = `bold ${11 * zoom}px sans-serif`;
  ctx.fillText(String(divisions.length), x + 52 * zoom, cy);

  // 堆叠角标(>1 时右下角)
  if (divisions.length > 1) {
    ctx.fillStyle = '#ffd700';
    ctx.font = `bold ${9 * zoom}px sans-serif`;
    ctx.fillText('+' + (divisions.length - 1), x + 66 * zoom, y + h - 4 * zoom);
  }

  // 战斗中的师 → 牌子描红边
  const inCombat = divisions.some(d => battles?.some(b => b.atk?.includes(d.id) || b.def?.includes(d.id)));
  if (inCombat) {
    ctx.strokeStyle = '#ff3030';
    ctx.lineWidth = 2.5;
    ctx.strokeRect(x - 1, y - 1, w + 2, h + 2);
  }

  // 撤退中 → 灰色蒙版
  if (divisions.some(d => d.retreating)) {
    ctx.fillStyle = 'rgba(100,100,100,0.4)';
    ctx.fillRect(x, y, w, h);
  }
  ctx.textBaseline = 'alphabetic';  // 复位, 避免影响后续图层
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
    // 牌子画在重心上方
    drawCard(ctx, sc.x, sc.y - 28 * zoom, divs, battles, zoom);
  }
}
