// Canvas 管家: 相机(pan/zoom) + 图层注册 + 坐标转换 + 脏标记重绘
let canvasEl, ctx, dpr;
const layers = [];  // [{ name, z, draw, dirty }]
const camera = { x: 0, y: 0, zoom: 1 };
let fullRedraw = true;

export function init() {
  canvasEl = document.getElementById('map');
  dpr = window.devicePixelRatio || 1;
  ctx = canvasEl.getContext('2d');
  resize();
  window.addEventListener('resize', resize);
}

function resize() {
  const W = canvasEl.clientWidth, H = canvasEl.clientHeight;
  canvasEl.width = W * dpr;
  canvasEl.height = H * dpr;
  fullRedraw = true;
}

// 注册图层: name 唯一, z 越小越先画(底层), drawFn(ctx, view, { worldToScreen, camera, W, H })
export function addLayer(name, z, drawFn) {
  layers.push({ name, z, draw: drawFn, dirty: true });
  layers.sort((a, b) => a.z - b.z);
}

// 标记某层需要重绘(数据变化时调用)
export function markDirty(layerName) {
  const l = layers.find(l => l.name === layerName);
  if (l) l.dirty = true;
}

// 标记全部重绘(相机变换/大小调整时)
export function markAllDirty() { fullRedraw = true; }

// 相机操作
export function pan(dx, dy) {
  camera.x += dx;
  camera.y += dy;
  fullRedraw = true;
}

export function zoomBy(f, cx, cy) {
  // 以屏幕点(cx, cy)为锚点缩放(该点世界坐标保持不变)
  const wx = (cx - camera.x) / camera.zoom;
  const wy = (cy - camera.y) / camera.zoom;
  camera.zoom = Math.max(0.3, Math.min(5, camera.zoom * f));
  camera.x = cx - wx * camera.zoom;
  camera.y = cy - wy * camera.zoom;
  fullRedraw = true;
}

export function resetCamera() {
  camera.x = 0;
  camera.y = 0;
  camera.zoom = 1;
  fullRedraw = true;
}

// 世界坐标 → 屏幕坐标(图层绘制用)
export function worldToScreen(p) {
  return { x: p.x * camera.zoom + camera.x, y: p.y * camera.zoom + camera.y };
}

// 屏幕坐标 → 世界坐标(hit-test 用)
export function screenToWorld(p) {
  return { x: (p.x - camera.x) / camera.zoom, y: (p.y - camera.y) / camera.zoom };
}

// 获取相机状态(图层可读)
export function getCamera() { return { ...camera }; }

// 渲染: 每帧调一次, 传入 viewModel。只重绘脏层或全层。
export function render(view) {
  if (!ctx) return;
  const W = canvasEl.clientWidth, H = canvasEl.clientHeight;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, W, H);

  for (const l of layers) {
    if (fullRedraw || l.dirty) {
      ctx.save();
      l.draw(ctx, view, { worldToScreen, camera, W, H });
      ctx.restore();
      l.dirty = false;
    }
  }
  fullRedraw = false;
}
