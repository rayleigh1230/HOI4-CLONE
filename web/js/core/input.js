// 统一输入层: PointerEvent 归一化鼠标+触屏 + 手势识别。
// 手势分工(对齐原版 HOI4 拖拽下令):
//   桌面: 左键拖兵牌=拉箭头下令 / 左键拖空白=平移 / 左键点=选中 / 右键拖=平移 / 滚轮=缩放
//   触屏: 单指拖=平移 / 单指点=选中
//
// 拖拽下令流程: 按下时 main.js 判定是否命中兵牌(onDownCheck) → 命中则进入下令模式,
// 拖动中实时回调(onDragMove, 带当前世界坐标), 松开时回调(onDragEnd, 带终点世界坐标)。
import { pan, zoomBy, screenToWorld } from './canvas.js';

let pointers = new Map();  // pointerId → { x, y, pointerType }
let lastSingle = null;     // { x, y, moved, button, pointerType }

const CLICK_THRESHOLD = 5;
// 拖拽下令状态: null 或 { fromScreen: {x,y}, fromDiv: 师id }。非 null 时拖动=拉箭头。
let dragOrder = null;
let panning = false;       // 是否在平移(避免与下令混)

let onHitCallbacks = [];
let onBackgroundCallbacks = [];
// 按下时判定: 返回 { divId } 表示命中兵牌(进入下令), 否则 null(走平移/选中)
let onDownCheckCallbacks = [];
// 拖拽下令拖动中: fn({ x, y } 屏幕坐标, world {x,y})
let onDragMoveCallbacks = [];
// 拖拽下令松开: fn(world {x,y})
let onDragEndCallbacks = [];

export function init() {
  const canvas = document.getElementById('map');
  canvas.addEventListener('pointerdown', onDown);
  canvas.addEventListener('pointermove', onMove);
  canvas.addEventListener('pointerup', onUp);
  canvas.addEventListener('pointercancel', onUp);
  canvas.addEventListener('pointerleave', onUp);
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const rect = canvas.getBoundingClientRect();
    zoomBy(e.deltaY < 0 ? 1.1 : 0.9, e.clientX - rect.left, e.clientY - rect.top);
  }, { passive: false });
}

function onDown(e) {
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY, pointerType: e.pointerType });
  if (pointers.size === 1) {
    lastSingle = { x: e.clientX, y: e.clientY, moved: false, button: e.button, pointerType: e.pointerType };
    // 桌面左键: 先问 main.js 是否命中兵牌 → 命中且可下令才进入拖拽下令模式
    if (e.pointerType !== 'touch' && e.button === 0) {
      const rect = document.getElementById('map').getBoundingClientRect();
      const hit = runDownCheck(e.clientX - rect.left, e.clientY - rect.top);
      if (hit && hit.divId != null && hit.canCommand) {
        dragOrder = { fromScreen: { x: e.clientX - rect.left, y: e.clientY - rect.top }, fromDiv: hit.divId };
      }
    }
  } else if (pointers.size === 2) {
    dragOrder = null; panning = false;
    const pts = [...pointers.values()];
    const rect = document.getElementById('map').getBoundingClientRect();
    pinchStart = {
      dist: Math.hypot(pts[0].x - pts[1].x, pts[0].y - pts[1].y),
      cx: (pts[0].x + pts[1].x) / 2 - rect.left,
      cy: (pts[0].y + pts[1].y) / 2 - rect.top,
    };
  }
}
let pinchStart = null;

function onMove(e) {
  if (!pointers.has(e.pointerId)) return;
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY, pointerType: e.pointerType });

  if (pointers.size === 1 && lastSingle) {
    const dx = e.clientX - lastSingle.x, dy = e.clientY - lastSingle.y;
    if (Math.hypot(dx, dy) > CLICK_THRESHOLD) lastSingle.moved = true;
    const rect = document.getElementById('map').getBoundingClientRect();
    const sx = e.clientX - rect.left, sy = e.clientY - rect.top;

    if (dragOrder) {
      // 拖拽下令中: 更新当前位置(供 overlay 画箭头) + 通知 main.js(高亮悬停省)
      dragOrder.curScreen = { x: sx, y: sy };
      const wp = screenToWorld({ x: sx, y: sy });
      for (const fn of onDragMoveCallbacks) fn({ x: sx, y: sy }, wp);
    } else if (lastSingle.pointerType === 'touch') {
      if (lastSingle.moved) { panning = true; pan(dx, dy); }
    } else {
      // 桌面: 右键拖 或 左键拖空白 = 平移
      if ((lastSingle.button === 2 || lastSingle.button === 0) && lastSingle.moved) {
        panning = true; pan(dx, dy);
      }
    }
    lastSingle.x = e.clientX; lastSingle.y = e.clientY;
  } else if (pointers.size === 2 && pinchStart) {
    const pts = [...pointers.values()];
    const d = Math.hypot(pts[0].x - pts[1].x, pts[0].y - pts[1].y);
    zoomBy(d / pinchStart.dist, pinchStart.cx, pinchStart.cy);
    pinchStart.dist = d;
  }
}

function onUp(e) {
  const wasDragOrder = dragOrder;
  pointers.delete(e.pointerId);

  if (wasDragOrder) {
    const rect = document.getElementById('map').getBoundingClientRect();
    const sx = e.clientX - rect.left, sy = e.clientY - rect.top;
    const wp = screenToWorld({ x: sx, y: sy });
    const info = { fromScreen: wasDragOrder.fromScreen, curScreen: { x: sx, y: sy }, fromDiv: wasDragOrder.fromDiv };
    dragOrder = null;
    for (const fn of onDragEndCallbacks) fn(wp, info);
    if (pointers.size === 0) { lastSingle = null; panning = false; }
    return;
  }

  // 单指未移动 = 点击
  if (pointers.size === 0 && lastSingle && !lastSingle.moved) {
    handleClick(e.clientX, e.clientY);
  }
  if (pointers.size === 1 && !lastSingle?.moved) {
    const pts = [...pointers.values()];
    lastSingle = { x: pts[0].x, y: pts[0].y, moved: false, pointerType: pts[0].pointerType };
  } else if (pointers.size === 0) {
    lastSingle = null; pinchStart = null; panning = false;
  }
}

function handleClick(cx, cy) {
  const canvas = document.getElementById('map');
  const rect = canvas.getBoundingClientRect();
  const sx = cx - rect.left, sy = cy - rect.top;
  const wp = screenToWorld({ x: sx, y: sy });
  for (let i = onHitCallbacks.length - 1; i >= 0; i--) {
    if (onHitCallbacks[i](wp, sx, sy)) return;
  }
  for (const fn of onBackgroundCallbacks) fn();
}

function runDownCheck(sx, sy) {
  for (const fn of onDownCheckCallbacks) {
    const r = fn(sx, sy);
    if (r) return r;
  }
  return null;
}

// 当前拖拽下令状态(供 overlay 画箭头用)。null 或 { fromScreen, fromDiv }
export function getDragOrder() { return dragOrder; }

export function onHit(fn) { onHitCallbacks.push(fn); }
export function onBackground(fn) { onBackgroundCallbacks.push(fn); }
export function onDownCheck(fn) { onDownCheckCallbacks.push(fn); }
export function onDragMove(fn) { onDragMoveCallbacks.push(fn); }
export function onDragEnd(fn) { onDragEndCallbacks.push(fn); }
