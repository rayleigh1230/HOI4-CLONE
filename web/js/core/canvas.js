// Canvas 管家: 相机(pan/zoom) + 图层注册 + 坐标转换 + 脏标记重绘
// 对齐 spec §6.2: render 只画 (layer.dirty || fullRedraw) 的层;
//                 相机变换/尺寸变化 → markAllDirty(); 数据变化 → markDirty(layerName)
let canvasEl, ctx, dpr;
const layers = [];
const camera = { x: 0, y: 0, zoom: 1 };
let _lastView = null;   // 最近一次 render 的 view, pan/zoom 后复用
let fullRedraw = true;  // 是否全层重绘(声明! init 后 true 保证首帧全画; resize/相机变换置 true)

export function init() {
  canvasEl = document.getElementById('map');
  dpr = window.devicePixelRatio || 1;
  ctx = canvasEl.getContext('2d');
  resize();
  window.addEventListener('resize', resize);
}

function resize() {
  const W = canvasEl.clientWidth, H = canvasEl.clientHeight;
  // 防御: 极小尺寸时给 1, 避免 0×0 backing store
  canvasEl.width = Math.max(1, Math.round(W * dpr));
  canvasEl.height = Math.max(1, Math.round(H * dpr));
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
export function markAllDirty() {
  for (const l of layers) l.dirty = true;
  fullRedraw = true;
}

// 相机操作
export function pan(dx, dy) {
  camera.x += dx;
  camera.y += dy;
  _requestRender();
}

export function zoomBy(f, cx, cy) {
  const wx = (cx - camera.x) / camera.zoom;
  const wy = (cy - camera.y) / camera.zoom;
  camera.zoom = Math.max(0.3, Math.min(5, camera.zoom * f));
  camera.x = cx - wx * camera.zoom;
  camera.y = cy - wy * camera.zoom;
  _requestRender();
}

export function resetCamera() {
  camera.x = 0;
  camera.y = 0;
  camera.zoom = 1;
  _requestRender();
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

// 实际绘制: clear + 全层重画(数据量小, 全量重画避免"只画脏层导致的黑屏",
// 对齐 3324e3a 的教训)。脏标记的价值在外部 _requestRender 门控(见下)。
export function render(view) {
  if (!ctx) return;
  _lastView = view;
  const W = canvasEl.clientWidth, H = canvasEl.clientHeight;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, W, H);
  for (const l of layers) {
    ctx.save();
    l.draw(ctx, view, { worldToScreen, camera, W, H });
    ctx.restore();
    l.dirty = false;
  }
  fullRedraw = false;
}

// 数据驱动的重绘请求: markDirty/markAllDirty 后由 store/main 调 render。
// 不立即重画, 避免一次 tick 里多次 setState 重复绘制。
function _requestRender() {
  markAllDirty();
  if (_lastView) render(_lastView);
}
