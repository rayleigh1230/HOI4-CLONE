// 图层5: UI 覆盖层(选中高亮 / tooltip)。对齐 spec §6.1。
// 选中高亮从 layerProvince 分离至此: 该层只画"交互态", 不画基础省份。
// 选中逻辑(selectProvince/getSelected)仍由 layerProvince 持有状态, 此处只读。
import { provincePos } from './layout.js';
import { getSelected } from './layerProvince.js';

export function draw(ctx, view, { worldToScreen, W, H }) {
  const sel = getSelected();
  const { provinces } = view;
  if (sel == null || !provinces?.length) return;

  const ids = provinces.map(p => p.id);
  const p = provinces.find(x => x.id === sel);
  if (!p) return;

  const c = worldToScreen(provincePos(sel, ids, W, H));

  // 金色高亮环(选中省)
  ctx.save();
  ctx.strokeStyle = '#ffd700';
  ctx.lineWidth = 3.5;
  ctx.shadowColor = 'rgba(255,215,0,0.8)';
  ctx.shadowBlur = 10;
  ctx.beginPath();
  ctx.arc(c.x, c.y, 30, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();

  // 选中标签
  ctx.fillStyle = '#ffd700';
  ctx.font = 'bold 11px sans-serif';
  ctx.textAlign = 'center';
  ctx.fillText(`◆ 省${sel}`, c.x, c.y - 34);
}
