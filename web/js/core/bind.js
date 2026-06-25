// 声明式数据绑定: 对齐原版 scripted_gui 的 visible/properties/dynamic_lists 模型
import { subscribe } from './store.js';
import { clear } from './el.js';

// path 驱动值解析: "a.b.0.c" → obj.a.b[0].c
function resolve(obj, path) {
  if (!obj || !path) return obj;
  return path.split('.').reduce((o, k) => (o == null ? o : o[k]), obj);
}

// bindText: 数据变 → fn 收到解析后的值(用于文本/数值实时显示)
export function bindText(path, fn) {
  return subscribe((state) => {
    const val = resolve(state, path);
    fn(val);
  });
}

// bindWhen: path 解析值满足 pred 则显示元素, 否则隐藏
export function bindWhen(el, path, pred) {
  return subscribe((state) => {
    const val = resolve(state, path);
    el.style.display = pred(val) ? '' : 'none';
  });
}

// bindEnabled: path 解析值满足 pred 则启用按钮, 否则灰掉
export function bindEnabled(el, path, pred) {
  return subscribe((state) => {
    const val = resolve(state, path);
    el.disabled = !pred(val);
  });
}

// bindList: path 解析为数组, 每项用 renderItem 渲染, 结果填入 container
// renderItem(item, index, fullState) → DOM element
export function bindList(container, path, renderItem) {
  return subscribe((state) => {
    const arr = resolve(state, path) || [];
    clear(container);
    for (let i = 0; i < arr.length; i++) {
      const item = renderItem(arr[i], i, state);
      if (item) container.append(item);
    }
  });
}
