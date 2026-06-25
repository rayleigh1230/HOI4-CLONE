// 状态读取: get_state(每帧数据视图) + get_templates(静态模板列表, 启动后不变)
import { getWasm, readCString } from './wasm.js';

// 取当前世界状态(JSON 快照, 含 divisions/battles/provinces/date/wars/factions)
export function getState() {
  const ptr = getWasm().exports.engine_get_state();
  return JSON.parse(readCString(ptr));
}

// 取所有可用模板名(部署面板下拉用, 启动后不变, 调一次缓存)
export function getTemplates() {
  const ptr = getWasm().exports.engine_get_templates();
  return JSON.parse(readCString(ptr));
}
