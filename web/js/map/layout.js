// 省份布局: 固定世界坐标系(1000×700) + 手写多边形 + 地形类型。
// 对齐 spec §2: 所有图层用世界坐标, 相机负责 worldToScreen 变换。
//
// 布局: 5列×2排网格。列边界 x=0/200/400/600/800/1000 严格对齐。
// 上排(1-5 GER) y=40~340, 下排(6-10 FRA) y=360~660, 中线 y=350 前线。
// 多边形拼满无缝(共享边坐标严格一致), 邻接沿 neighbors 数组(逻辑邻接)。
//
// neighbors(沿用 main.js setup):
//   1↔2,6,7 | 2↔1,3,6,7,8 | 3↔2,4,7,8,9 | 4↔3,5,8,9,10 | 5↔4,9,10
//   6↔1,2,7 | 7↔1,2,3,6,8 | 8↔2,3,4,7,9 | 9↔3,4,5,8,10 | 10↔4,5,9

export const WORLD_W = 1000;
export const WORLD_H = 700;

// 列边界(5列)与排边界(2排)。所有省多边形顶点引用这些值, 保证共享边吻合。
const X = [0, 200, 400, 600, 800, 1000]; // 列0..5
const TOP = 40, MID = 350, BOT = 660;     // 上排顶/中线/下排底

export const PROVINCES = {
  // 上排 GER(列1-5)
  1:  { terrain: 'plains',  poly: [[X[0],TOP],[X[1],TOP],[X[1],MID],[X[0],MID]],            centroid: [(X[0]+X[1])/2, (TOP+MID)/2] },
  2:  { terrain: 'forest',  poly: [[X[1],TOP],[X[2],TOP],[X[2],MID],[X[1],MID]],            centroid: [(X[1]+X[2])/2, (TOP+MID)/2] },
  3:  { terrain: 'hills',   poly: [[X[2],TOP],[X[3],TOP],[X[3],MID],[X[2],MID]],            centroid: [(X[2]+X[3])/2, (TOP+MID)/2] },
  4:  { terrain: 'plains',  poly: [[X[3],TOP],[X[4],TOP],[X[4],MID],[X[3],MID]],            centroid: [(X[3]+X[4])/2, (TOP+MID)/2] },
  5:  { terrain: 'forest',  poly: [[X[4],TOP],[X[5],TOP],[X[5],MID],[X[4],MID]],            centroid: [(X[4]+X[5])/2, (TOP+MID)/2] },
  // 下排 FRA(列1-5)
  6:  { terrain: 'plains',  poly: [[X[0],MID],[X[1],MID],[X[1],BOT],[X[0],BOT]],            centroid: [(X[0]+X[1])/2, (MID+BOT)/2] },
  7:  { terrain: 'urban',   poly: [[X[1],MID],[X[2],MID],[X[2],BOT],[X[1],BOT]],            centroid: [(X[1]+X[2])/2, (MID+BOT)/2] },
  8:  { terrain: 'hills',   poly: [[X[2],MID],[X[3],MID],[X[3],BOT],[X[2],BOT]],            centroid: [(X[2]+X[3])/2, (MID+BOT)/2] },
  9:  { terrain: 'plains',  poly: [[X[3],MID],[X[4],MID],[X[4],BOT],[X[3],BOT]],            centroid: [(X[3]+X[4])/2, (MID+BOT)/2] },
  10: { terrain: 'mountain',poly: [[X[4],MID],[X[5],MID],[X[5],BOT],[X[4],BOT]],            centroid: [(X[4]+X[5])/2, (MID+BOT)/2] },
};

export const TERRAIN_COLORS = {
  plains:   '#3a5a40',
  forest:   '#2d4a2b',
  hills:    '#5a4a3a',
  urban:    '#4a4a4a',
  mountain: '#6b5b4a',
};

export const TAG_COLORS = { GER: '#e94560', FRA: '#16c79a' };

// 取省多边形顶点(世界坐标)
export function provincePoly(id) {
  return PROVINCES[id]?.poly;
}
// 取省重心(世界坐标, 离线预算存入, 不每帧算)
export function provinceCentroid(id) {
  const c = PROVINCES[id]?.centroid;
  return c ? { x: c[0], y: c[1] } : null;
}
// 取省地形
export function provinceTerrain(id) {
  return PROVINCES[id]?.terrain;
}

// 点在多边形内(射线法)。pt={x,y} 世界坐标, poly=[[x,y],...]。
// 用于命中检测: 点击 → screenToWorld → pointInPolygon
export function pointInPolygon(pt, poly) {
  if (!poly || poly.length < 3) return false;
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const xi = poly[i][0], yi = poly[i][1];
    const xj = poly[j][0], yj = poly[j][1];
    const intersect = ((yi > pt.y) !== (yj > pt.y)) &&
      (pt.x < (xj - xi) * (pt.y - yi) / ((yj - yi) || 1e-9) + xi);
    if (intersect) inside = !inside;
  }
  return inside;
}
// 找点击世界坐标所在的省(遍历所有省多边形)。返回省 id 或 null。
export function provinceAt(worldPoint, provinceIds) {
  for (const id of provinceIds) {
    if (pointInPolygon(worldPoint, PROVINCES[id]?.poly)) return id;
  }
  return null;
}
