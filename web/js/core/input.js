// 统一输入层: PointerEvent 归一化鼠标+触屏 + 手势识别。
// 手势分工(对齐原版 HOI4):
//   桌面: 左键拖=框选 / 左键点=选中 / 右键拖=平移 / 滚轮=缩放
//   触屏: 单指拖=平移 / 单指点=选中 (触屏暂不支持框选, 避免与平移冲突)
import { pan, zoomBy, resetCamera, screenToWorld, getCamera } from './canvas.js';

let pointers = new Map();  // pointerId → { x, y, pointerType }
let lastSingle = null;     // { x, y, moved, button } 单指追踪
let pinchStart = null;     // { dist, cx, cy }

const CLICK_THRESHOLD = 5;   // 拖拽超过此距离判定为"拖动"而非"点击"
let boxSelect = null;        // 框选状态 { x0, y0, x1, y1 } (屏幕坐标), null=未框选

let onHitCallbacks = [];        // fn(worldPos, sx, sy) → true(已消费) / false
let onBackgroundCallbacks = []; // fn()
let onBoxCallbacks = [];        // fn(screenRect {x0,y0,x1,y1}) → 用世界坐标框住的师列表

export function init() {
  const canvas = document.getElementById('map');
  canvas.addEventListener('pointerdown', onDown);
  canvas.addEventListener('pointermove', onMove);
  canvas.addEventListener('pointerup', onUp);
  canvas.addEventListener('pointercancel', onUp);
  canvas.addEventListener('pointerleave', onUp);
  canvas.addEventListener('contextmenu', (e) => e.preventDefault()); // 屏蔽右键菜单(右键用于平移)
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const rect = canvas.getBoundingClientRect();
    zoomBy(e.deltaY < 0 ? 1.1 : 0.9, e.clientX - rect.left, e.clientY - rect.top);
  }, { passive: false });
}

function isTouch() {
  for (const p of pointers.values()) if (p.pointerType === 'touch') return true;
  return false;
}

function onDown(e) {
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY, pointerType: e.pointerType });
  if (pointers.size === 1) {
    lastSingle = { x: e.clientX, y: e.clientY, moved: false, button: e.button };
    // 桌面左键按下: 记录框选起点(移动超过阈值才启动框选)
    if (e.pointerType !== 'touch' && e.button === 0) {
      boxSelect = { x0: e.clientX, y0: e.clientY, x1: e.clientX, y1: e.clientY, active: false };
    }
  } else if (pointers.size === 2) {
    // 双指 → 捏合缩放(触屏) 或 取消框选
    boxSelect = null;
    const pts = [...pointers.values()];
    pinchStart = {
      dist: Math.hypot(pts[0].x - pts[1].x, pts[0].y - pts[1].y),
      cx: (pts[0].x + pts[1].x) / 2,
      cy: (pts[0].y + pts[1].y) / 2,
    };
  }
}

function onMove(e) {
  if (!pointers.has(e.pointerId)) return;
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY, pointerType: e.pointerType });

  if (pointers.size === 1 && lastSingle) {
    const dx = e.clientX - lastSingle.x;
    const dy = e.clientY - lastSingle.y;
    if (Math.hypot(dx, dy) > CLICK_THRESHOLD) lastSingle.moved = true;

    if (lastSingle.pointerType === undefined) lastSingle.pointerType = e.pointerType;

    if (e.pointerType === 'touch') {
      // 触屏单指拖 = 平移
      if (lastSingle.moved) pan(dx, dy);
    } else {
      // 桌面: 左键拖 = 框选, 右键拖(按钮2) = 平移
      if (lastSingle.button === 0 && boxSelect) {
        // 启动/更新框选
        boxSelect.active = true;
        boxSelect.x1 = e.clientX; boxSelect.y1 = e.clientY;
      } else if (lastSingle.button === 2 && lastSingle.moved) {
        pan(dx, dy);
      }
    }
    lastSingle.x = e.clientX; lastSingle.y = e.clientY;
  } else if (pointers.size === 2 && pinchStart) {
    const pts = [...pointers.values()];
    const d = Math.hypot(pts[0].x - pts[1].x, pts[0].y - pts[1].y);
    const rect = document.getElementById('map').getBoundingClientRect();
    zoomBy(d / pinchStart.dist, pinchStart.cx - rect.left, pinchStart.cy - rect.top);
    pinchStart.dist = d;
  }
}

function onUp(e) {
  const wasButton = lastSingle?.button;
  const wasTouch = lastSingle?.pointerType === 'touch';
  pointers.delete(e.pointerId);

  // 框选完成(桌面左键拖动结束)
  if (boxSelect && boxSelect.active) {
    const rect = normalizeRect(boxSelect);
    boxSelect = null;
    // 通知框选回调(交给 main.js 算框住的师)
    for (const fn of [...onBoxCallbacks]) {
      if (fn(rect)) return;
    }
    return;
  }
  boxSelect = null;

  // 单指/单点 未移动 = 点击
  if (pointers.size === 0 && lastSingle && !lastSingle.moved) {
    handleClick(e.clientX, e.clientY);
  }

  if (pointers.size === 1 && !lastSingle?.moved) {
    const pts = [...pointers.values()];
    lastSingle = { x: pts[0].x, y: pts[0].y, moved: false };
  } else if (pointers.size === 0) {
    lastSingle = null;
    pinchStart = null;
  }
}

function normalizeRect(b) {
  return {
    x0: Math.min(b.x0, b.x1), y0: Math.min(b.y0, b.y1),
    x1: Math.max(b.x0, b.x1), y1: Math.max(b.y0, b.y1),
  };
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

// 当前框选矩形(屏幕坐标, 供 overlay 画框)。无框选返回 null。
export function getBoxRect() {
  return boxSelect && boxSelect.active ? normalizeRect(boxSelect) : null;
}

export function onHit(fn) { onHitCallbacks.push(fn); }
export function onBackground(fn) { onBackgroundCallbacks.push(fn); }
export function onBoxSelect(fn) { onBoxCallbacks.push(fn); }
