// 统一输入层: PointerEvent 归一化鼠标+触屏 + 手势识别(拖拽/捏合/点击)
import { pan, zoomBy, resetCamera, screenToWorld } from './canvas.js';

let pointers = new Map();  // pointerId → { x, y }
let lastSingle = null;    // { x, y, moved }
let pinchStart = null;    // { dist, cx, cy }

const HIT_RADIUS = 44;  // 世界坐标命中半径(触屏 44px ≈ 拇指)
let onHitCallbacks = [];     // fn(worldPos, sx, sy) → true(已消费) / false
let onBackgroundCallbacks = [];

export function init() {
  const canvas = document.getElementById('map');
  canvas.addEventListener('pointerdown', onDown);
  canvas.addEventListener('pointermove', onMove);
  canvas.addEventListener('pointerup', onUp);
  canvas.addEventListener('pointercancel', onUp);
  canvas.addEventListener('pointerleave', onUp);
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const rect = canvas.getBoundingClientRect();
    zoomBy(e.deltaY < 0 ? 1.1 : 0.9, e.clientX - rect.left, e.clientY - rect.top);
  }, { passive: false });
}

function onDown(e) {
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
  if (pointers.size === 1) {
    lastSingle = { x: e.clientX, y: e.clientY, moved: false };
  } else if (pointers.size === 2) {
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
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });

  if (pointers.size === 1 && lastSingle) {
    const dx = e.clientX - lastSingle.x;
    const dy = e.clientY - lastSingle.y;
    if (Math.hypot(dx, dy) > 5) {
      lastSingle.moved = true;
    }
    if (lastSingle.moved) {
      pan(dx, dy);
    }
    lastSingle.x = e.clientX;
    lastSingle.y = e.clientY;
  } else if (pointers.size === 2 && pinchStart) {
    const pts = [...pointers.values()];
    const d = Math.hypot(pts[0].x - pts[1].x, pts[0].y - pts[1].y);
    const rect = document.getElementById('map').getBoundingClientRect();
    zoomBy(d / pinchStart.dist, pinchStart.cx - rect.left, pinchStart.cy - rect.top);
    pinchStart.dist = d;
  }
}

function onUp(e) {
  pointers.delete(e.pointerId);

  // 单指按下未移动 → 判定为点击
  if (pointers.size === 0 && lastSingle && !lastSingle.moved) {
    handleClick(e.clientX, e.clientY);
  }

  // 剩余一指继续追踪
  if (pointers.size === 1 && !lastSingle?.moved) {
    const pts = [...pointers.values()];
    lastSingle = { x: pts[0].x, y: pts[0].y, moved: false };
  } else if (pointers.size === 0) {
    lastSingle = null;
    pinchStart = null;
  }
}

function handleClick(cx, cy) {
  const canvas = document.getElementById('map');
  const rect = canvas.getBoundingClientRect();
  const sx = cx - rect.left;
  const sy = cy - rect.top;
  const wp = screenToWorld({ x: sx, y: sy });

  // 命中回调(按注册逆序, 第一层消费后停止)
  for (let i = onHitCallbacks.length - 1; i >= 0; i--) {
    if (onHitCallbacks[i](wp, sx, sy)) return;
  }
  // 没命中 → 背景点击
  for (const fn of onBackgroundCallbacks) fn();
}

// 注册命中回调(最晚注册的在最上层, 最先消费)
export function onHit(fn) { onHitCallbacks.push(fn); }

// 注册背景点击回调
export function onBackground(fn) { onBackgroundCallbacks.push(fn); }
