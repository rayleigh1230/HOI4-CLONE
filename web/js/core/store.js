// 视图状态容器: 持有完整 viewModel, setState 时通知所有订阅者
// (简化版: 全量通知; 路径级脏标记优化留后续, 当前数据量小够用)
export const store = {
  state: null,
  listeners: new Set(),  // fn(fullState)

  setState(next) {
    this.state = next;
    for (const fn of this.listeners) {
      try { fn(next); } catch (err) { console.error('[store] 订阅回调出错:', err); }
    }
  },
};

// 订阅状态变化。fn 收到完整 state。返回取消订阅函数。
// 注: 本版是全量订阅(任一变化都触发)。路径级订阅在数据量大时再加。
export function subscribe(fn) {
  store.listeners.add(fn);
  if (store.state) fn(store.state);  // 立即触发一次(初始化)
  return () => store.listeners.delete(fn);
}
