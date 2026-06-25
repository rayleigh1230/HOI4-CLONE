// 省份坐标布局(沿用旧 drawMap 的 provincePos 算法: 上下两排对垒)
export function provincePos(id, allIds, w, h) {
  const sorted = [...allIds].sort((a, b) => a - b);
  const half = Math.ceil(sorted.length / 2);
  const topIds = sorted.slice(0, half);
  const botIds = sorted.slice(half);
  const row = topIds.includes(id) ? 'top' : 'bottom';
  const rowIds = row === 'top' ? topIds : botIds;
  const colIdx = rowIds.indexOf(id);
  const colN = rowIds.length;
  const margin = 40;
  const usable = w - margin * 2;
  const x = colN <= 1 ? w / 2 : margin + (usable * colIdx / (colN - 1));
  const y = row === 'top' ? h * 0.27 : h * 0.73;
  return { x, y };
}

export const TAG_COLORS = { GER: '#e94560', FRA: '#16c79a' };
