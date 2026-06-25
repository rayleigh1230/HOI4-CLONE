// 图层4: 战斗指示。带进度数字小圆(可点击), 对齐 mapicons.gui:land_combat_mapicon。
// 进度 = 攻方 org 损耗比(前端现算, spec §6.2 方案A)。对齐 spec §5.1。
import { provinceCentroid } from './layout.js';

export let combatPulse = 0;
export function setCombatPulse(v) { combatPulse = v; }

// 计算某战斗的进度(攻方 org 损耗比, 0-1)
function battleProgress(battle, divisions) {
  const divMap = {};
  if (divisions) for (const d of divisions) divMap[d.id] = d;
  let maxOrgSum = 0, orgSum = 0;
  for (const id of battle.atk || []) {
    const d = divMap[id];
    if (d) { maxOrgSum += d.max_org; orgSum += d.org; }
  }
  if (maxOrgSum <= 0) return 0;
  return Math.max(0, Math.min(1, (maxOrgSum - orgSum) / maxOrgSum));
}

// 取所有战斗图标的屏幕位置+半径(供 main.js 点击命中用)
export function combatIcons(view, worldToScreen, zoom) {
  const out = [];
  if (!view.battles) return out;
  for (const b of view.battles) {
    const c = provinceCentroid(b.prov);
    if (!c) continue;
    const sc = worldToScreen(c);
    out.push({ battleId: b.id, prov: b.prov, x: sc.x, y: sc.y - 50 * zoom, r: 16 * zoom });
  }
  return out;
}

export function draw(ctx, view, { worldToScreen, camera }) {
  const { battles, divisions } = view;
  if (!battles?.length) return;
  const zoom = camera.zoom;

  for (const b of battles) {
    const c = provinceCentroid(b.prov);
    if (!c) continue;
    const sc = worldToScreen(c);
    const cx = sc.x, cy = sc.y - 50 * zoom;
    const r = 16 * zoom;
    const prog = battleProgress(b, divisions);

    // 脉冲外圈
    const pulseR = r + 3 * Math.sin(combatPulse);
    ctx.strokeStyle = `rgba(233,69,96,${0.5 + 0.3 * Math.sin(combatPulse)})`;
    ctx.lineWidth = 2;
    ctx.beginPath(); ctx.arc(cx, cy, pulseR, 0, Math.PI * 2); ctx.stroke();

    // 圆底
    ctx.fillStyle = 'rgba(60,10,20,0.92)';
    ctx.beginPath(); ctx.arc(cx, cy, r, 0, Math.PI * 2); ctx.fill();
    ctx.strokeStyle = '#ff3030';
    ctx.lineWidth = 1.5;
    ctx.stroke();

    // 进度数字
    ctx.fillStyle = '#fff';
    ctx.font = `bold ${10 * zoom}px sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(Math.round(prog * 100) + '%', cx, cy);
    ctx.textBaseline = 'alphabetic';

    // 下方细进度条
    const barW = 26 * zoom;
    ctx.fillStyle = '#333';
    ctx.fillRect(cx - barW / 2, cy + r + 2, barW, 3);
    ctx.fillStyle = '#ff6b6b';
    ctx.fillRect(cx - barW / 2, cy + r + 2, barW * prog, 3);
  }
}
