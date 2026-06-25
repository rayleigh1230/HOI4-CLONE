// 声明式数据绑定: 对齐原版 scripted_gui 的 visible/properties/dynamic_lists 模型。
// 对齐 spec §3.2/§3.3: 绑定声明关注的数据 key, 仅当该 key 脏时才重算/重建。
import { subscribe, subscribeKeys } from './store.js';
import { clear } from './el.js';

// path 驱动值解析: "a.b.0.c" → obj.a.b[0].c
function resolve(obj, path) {
  if (!obj || !path) return obj;
  return path.split('.').reduce((o, k) => (o == null ? o : o[k]), obj);
}

// 取 path 的顶层 key("divisions.0.org" → "divisions"; "date" → "date")。
// 用于路径订阅: 只在顶层 key 变化时重算。
function topKey(path) {
  if (!path) return null;
  return path.split('.')[0];
}

// bindText: 数据变 → fn 收到解析后的值(用于文本/数值实时显示)
export function bindText(path, fn) {
  const k = topKey(path);
  const run = (state) => fn(resolve(state, path));
  return k ? subscribeKeys([k], run) : subscribe(run);
}

// bindWhen: path 解析值满足 pred 则显示元素, 否则隐藏
export function bindWhen(el, path, pred) {
  const k = topKey(path);
  const run = (state) => { el.style.display = pred(resolve(state, path)) ? '' : 'none'; };
  return k ? subscribeKeys([k], run) : subscribe(run);
}

// bindEnabled: path 解析值满足 pred 则启用按钮, 否则灰掉
export function bindEnabled(el, path, pred) {
  const k = topKey(path);
  const run = (state) => { el.disabled = !pred(resolve(state, path)); };
  return k ? subscribeKeys([k], run) : subscribe(run);
}

// bindList: path 解析为数组, 每项用 renderItem 渲染, 结果填入 container。
// 仅当 path 对应的顶层 key 变化时才重建(对齐 spec §3.3 脏标记), 避免无谓重建。
// renderItem(item, index, fullState) → DOM element
//
// 注: 重建会清空 container 内的 <select>, 导致用户正在选的下拉被刷掉。
// 因此本函数只在数据真变时才 clear+重建, 解决"tick 中 select 选中态丢失"问题。
export function bindList(container, path, renderItem) {
  const k = topKey(path);
  const run = (state) => {
    const arr = resolve(state, path) || [];
    clear(container);
    for (let i = 0; i < arr.length; i++) {
      const item = renderItem(arr[i], i, state);
      if (item) container.append(item);
    }
  };
  return k ? subscribeKeys([k], run) : subscribe(run);
}
