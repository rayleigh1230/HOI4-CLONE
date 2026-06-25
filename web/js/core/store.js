// 视图状态容器: 持有完整 viewModel, setState 时做"路径级脏标记" diff,
// 只通知受影响的订阅者(对齐 spec §3.3 脏标记驱动, 避免每帧全量通知)。
//
// 顶层可观察 key(对齐 engine_get_state 的 viewModel 字段):
//   date / hour / player / divisions / battles / provinces / wars / factions
// 订阅者声明关注哪些 key, 仅当这些 key 变化时才被回调。
export const store = {
  state: null,
  _prev: null,        // 上一帧快照(用于 diff)
  _subsAll: new Set(),       // fn(fullState) — 全量订阅(topbar date 等)
  _subsKey: new Map(),       // key → Set<fn(fullState)> — 路径订阅(bindList)

  setState(next) {
    this._prev = this.state;
    this.state = next;
    const changed = diffKeys(this._prev, next);
    try {
      // 全量订阅者: 始终通知(它们内部自己判断, 如 date 显示)
      for (const fn of this._subsAll) {
        try { fn(next); } catch (err) { console.error('[store] 订阅回调出错:', err); }
      }
      // 路径订阅者: 仅当声明的 key 有变化才通知
      for (const [key, fns] of this._subsKey) {
        if (changed.has(key)) {
          for (const fn of fns) {
            try { fn(next); } catch (err) { console.error('[store] 订阅回调出错:', err); }
          }
        }
      }
    } finally {
      // 通知 canvas: 任一顶层 key 变化都意味着地图可能要重画
      // (具体哪些图层由 main.js 在订阅时 markDirty, 这里只负责 store 自身)
    }
  },
};

// 全量订阅: 任一状态变化都触发。返回取消订阅函数。
export function subscribe(fn) {
  store._subsAll.add(fn);
  if (store.state) fn(store.state);  // 立即触发一次(初始化)
  return () => store._subsAll.delete(fn);
}

// 路径订阅: 仅当任一 keys 变化时触发。keys 为顶层 key 数组(如 ['divisions'])。
// 返回取消订阅函数。对齐 spec §3.3: bindList 只在数据真变时重建, 避免无谓全量刷新。
export function subscribeKeys(keys, fn) {
  for (const k of keys) {
    if (!store._subsKey.has(k)) store._subsKey.set(k, new Set());
    store._subsKey.get(k).add(fn);
  }
  if (store.state) fn(store.state);  // 立即触发一次(初始化)
  return () => {
    for (const k of keys) store._subsKey.get(k)?.delete(fn);
  };
}

// 计算 prev → next 之间哪些顶层 key 发生了变化。
// 对数组(divisions/battles/provinces)用 JSON 串比较(数据量小, 够用且正确);
// 对标量(date 等)直接比对引用/值。
function diffKeys(prev, next) {
  const changed = new Set();
  if (!prev) {
    // 首帧: 全部视为变化
    if (next) for (const k of Object.keys(next)) changed.add(k);
    return changed;
  }
  const keys = new Set([...Object.keys(prev), ...Object.keys(next)]);
  for (const k of keys) {
    const a = prev[k], b = next[k];
    if (a === b) continue;
    // 对象/数组: 深比(用 JSON, 数据量小)。注意 undefined vs 缺失也要算变。
    const sa = JSON.stringify(a), sb = JSON.stringify(b);
    if (sa !== sb) changed.add(k);
  }
  return changed;
}
