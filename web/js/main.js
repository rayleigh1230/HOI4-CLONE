// 启动入口: 装配 WASM + 初始化场景
// core/store、canvas、input、views 在 Task 10-14 逐步接入
import { loadWasm } from './engine/wasm.js';
import { getState, getTemplates } from './engine/state.js';
import { setPlayer, runSetup } from './engine/commands.js';

async function main() {
  await loadWasm();
  document.getElementById('loading').style.display = 'none';
  document.getElementById('game').style.display = 'block';

  // 初始化场景(占位: Task 14 会换成完整 10 省 + declare_war setup)
  setPlayer('GER');
  const setup = `create_state = { id = 1 owner = GER }
create_province = { id = 1 state = 1 }`;
  runSetup(setup);

  // 验证数据流: 确认新字段(date/wars/factions/templates)都返回了
  console.log('[demo] templates:', getTemplates());
  console.log('[demo] state:', getState());
  console.log('[demo] ✅ UI 骨架空壳跑通, 待 Task 10-14 接入 core/canvas/views');
}

main();
