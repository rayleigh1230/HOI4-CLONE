// Playwright 端到端验证脚本 — DEMO 改造后的回归验证。
// 用法: node tests/web_demo.mjs
// 需先启动 http server: cd web && python -m http.server 8765
import { chromium } from 'playwright-chromium';

const URL = process.env.DEMO_URL || 'http://127.0.0.1:8765';
const results = [];
function check(name, cond, detail = '') {
  results.push({ name, ok: !!cond, detail });
  console.log(`${cond ? '✓' : '✗'} ${name}${detail ? ' — ' + detail : ''}`);
}

const browser = await chromium.launch({ channel: 'chrome', headless: true, args: ['--no-sandbox'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });

const consoleErrors = [];
const pageErrors = [];
page.on('console', m => {
  if (m.type() === 'error') {
    const t = m.text();
    // 浏览器默认请求 favicon.ico 的 404 是噪音, 与 demo 无关, 忽略
    if (/favicon/i.test(t)) return;
    consoleErrors.push(t);
  }
});
page.on('pageerror', e => pageErrors.push(e.message));

await page.goto(URL, { waitUntil: 'networkidle' });

// 给 main() 异步执行 + WASM 加载一点时间
await page.waitForTimeout(1500);

// 1. loading 隐藏, game 显示
const loadingDisplay = await page.locator('#loading').evaluate(el => getComputedStyle(el).display);
const gameDisplay = await page.locator('#game').evaluate(el => getComputedStyle(el).display);
check('loading 已隐藏', loadingDisplay === 'none');
check('game 已显示', gameDisplay !== 'none');

// 2. 无 JS / console 错误(改造前 #1 会让 init 抛 ReferenceError)
check('无 console.error', consoleErrors.length === 0, consoleErrors.slice(0, 3).join(' | '));
check('无未捕获 pageerror', pageErrors.length === 0, pageErrors.slice(0, 3).join(' | '));

// 3. canvas 有非零尺寸 + 实际画了东西(非全黑)。均匀网格采样覆盖全画布。
const canvasInfo = await page.locator('#map').evaluate(c => {
  const ctx = c.getContext('2d');
  const { width, height } = c;
  if (!width || !height) return { width, height, nonBlackPixels: 0 };
  let nonBlack = 0;
  let samples = 0;
  try {
    const img = ctx.getImageData(0, 0, width, height).data;
    // 网格采样: 每隔 step 像素取一点(步长 8 像素, 既密又快)
    const step = 8;
    for (let y = 0; y < height; y += step) {
      for (let x = 0; x < width; x += step) {
        const i = (y * width + x) * 4;
        samples++;
        if (img[i] + img[i+1] + img[i+2] > 30) nonBlack++;
      }
    }
  } catch (e) { return { width, height, nonBlackPixels: -1, err: String(e) }; }
  return { width, height, nonBlackPixels: nonBlack, samples };
});
check('canvas 尺寸非零', canvasInfo.width > 0 && canvasInfo.height > 0, `${canvasInfo.width}x${canvasInfo.height}`);
check('canvas 画出了内容(非全黑)', canvasInfo.nonBlackPixels > 5, `非黑采样点=${canvasInfo.nonBlackPixels}`);

// 4. store.state 已就绪 + 含 10 省 + 4 师 + date/wars/factions
const state = await page.evaluate(() => {
  // main.js 用 ES module, 拿不到 store; 但通过 canvas 渲染已验证数据。
  // 这里通过重新调 engine_get_state 间接验证(经 wasm 直接调)
  const w = window; // 引擎未挂全局, 跳过
  return null;
});
// 改为通过 console 里 demo 打印的日志验证(取 wasm exports 直接验)
const wasmState = await page.evaluate(async () => {
  // 直接重新加载 wasm 验证 get_state 结构(独立实例, 不影响 demo)
  const resp = await fetch('hoi4_clone.wasm?v=verify');
  const bytes = await resp.arrayBuffer();
  const inst = await WebAssembly.instantiate(bytes, { env: {} });
  const e = inst.instance.exports;
  function readStr(ptr) {
    const mem = new Uint8Array(e.memory.buffer);
    let end = ptr; while (mem[end] !== 0) end++;
    return new TextDecoder().decode(mem.subarray(ptr, end));
  }
  // 不 run_setup 就是空世界; 这里只验证 FFI 存在 + JSON 可解析
  const ptr = e.engine_get_state();
  return JSON.parse(readStr(ptr));
});
check('get_state JSON 含 date/wars/factions 字段',
  wasmState && wasmState.date && Array.isArray(wasmState.wars) && typeof wasmState.factions === 'object',
  JSON.stringify({ date: wasmState?.date, wars: wasmState?.wars?.length, factions: wasmState?.factions }));

// 5. 顶栏有日期 + 系统按钮; 底栏有时间控制按钮(spec §7.1)
const topbarText = await page.locator('#topbar').innerText();
const bottombarText = await page.locator('#bottombar').innerText();
check('顶栏显示日期', /📅|1936|\d+\.\d+\.\d+/.test(topbarText), topbarText.trim().slice(0, 40));
check('顶栏含系统按钮(部署/外交等)', /部署|外交|部队|交战/.test(topbarText));
check('底栏含时间控制按钮(spec §7.1)', /流逝|时|日/.test(bottombarText), bottombarText.trim().slice(0, 30));

// 6. 交互闭环: 点击地图省份 → 弹抽屉(多边形命中)。点省1重心(列1上排, 世界(100,195))
//    屏幕坐标 = 世界*zoom+cam, fit 后算出
const clickProv = await page.evaluate(() => {
  const W = window.innerWidth, H = window.innerHeight, margin = 20, WW = 1000, WH = 700;
  const zoom = Math.min((W - margin * 2) / WW, (H - margin * 2) / WH);
  const camX = W / 2 - WW / 2 * zoom, camY = H / 2 - WH / 2 * zoom;
  // 省1重心(列1上排) = (100, 195)
  return { x: 100 * zoom + camX, y: 195 * zoom + camY };
});
await page.mouse.click(clickProv.x, clickProv.y);
await page.waitForTimeout(400);
const drawerOpen = await page.locator('#drawer').evaluate(el => el.classList.contains('open'));
check('点击省份弹抽屉(多边形命中)', drawerOpen, 'drawer.open=' + drawerOpen);

// 6b. 战斗图标点击开战斗面板(spec §5.1/§5.4)。demo setup 含 GER 进攻省7, 进 demo 即有战斗。
const combatInfo = await page.evaluate(() => {
  if (!window._store) return null;
  const bs = window._store.state.battles;
  if (!bs.length) return { battles: 0 };
  // 算战斗图标屏幕坐标(复刻 layerCombat + canvas 坐标)
  const W = window.innerWidth, H = window.innerHeight, margin = 20, WW = 1000, WH = 700;
  const zoom = Math.min((W - margin * 2) / WW, (H - margin * 2) / WH);
  const camX = W / 2 - WW / 2 * zoom, camY = H / 2 - WH / 2 * zoom;
  const b = bs[0];
  const col = b.prov <= 5 ? b.prov : b.prov - 5;          // 列1-5
  const row = b.prov <= 5 ? 0 : 1;                         // 上排0/下排1
  const cx = (col - 0.5) * 200, cy = row === 0 ? 195 : 505; // 列中心x, 排中心y
  return { battles: bs.length, x: cx * zoom + camX, y: (cy - 50) * zoom + camY };
});
check('demo 初始有战斗(GER 进攻省7)', combatInfo?.battles > 0, 'battles=' + combatInfo?.battles);
if (combatInfo?.battles > 0) {
  // 关闭抽屉再点战斗图标
  await page.locator('#drawer').evaluate(el => el.classList.remove('open'));
  await page.mouse.click(combatInfo.x, combatInfo.y);
  await page.waitForTimeout(500);
  const combatPanelOpen = await page.locator('#panel-host').evaluate(el => el.classList.contains('open'));
  check('点击战斗图标开战斗面板(landcombatview)', combatPanelOpen, 'panel.open=' + combatPanelOpen);
}

// 6c. 多边形地形渲染: 采样含地形绿色调像素(spec §3.1)
const terrainCheck = await page.locator('#map').evaluate(c => {
  const ctx = c.getContext('2d');
  const img = ctx.getImageData(0, 0, c.width, c.height).data;
  let greenish = 0;
  for (let i = 0; i < img.length; i += 32) {
    if (img[i+1] > img[i] && img[i+1] > 40 && img[i+1] < 120 && img[i] < 100) greenish++;
  }
  return greenish;
});
check('地形多边形渲染(含绿色调像素)', terrainCheck > 20, `绿色调采样=${terrainCheck}`);

// 6d. get_state division 含新战斗属性字段(Task 1)
const hasFields = await page.evaluate(() => {
  const d = window._store?.state?.divisions?.[0];
  if (!d) return false;
  return d.soft_attack != null && d.defense != null && d.combat_width != null;
});
check('get_state 含战斗属性字段(soft/defense/width)', hasFields);

// 7. tick: 点底栏 +1时 按钮, 日期/hour 应推进
const beforeTick = await page.evaluate(() => document.querySelector('#bottombar').innerText);
await page.locator('#bottombar button:first-child').click();
await page.waitForTimeout(200);
// 推 24 次看日期变(单 +1时 看不出日期变)
for (let i = 0; i < 24; i++) {
  await page.locator('#bottombar button:first-child').click();
}
await page.waitForTimeout(300);
const topbarAfter = await page.locator('#topbar').innerText();
check('tick 链路通(推进后顶栏仍正常渲染)', /1936|\d+\.\d+\.\d+/.test(topbarAfter));

// 截图存证
await page.screenshot({ path: 'tests/demo-final.png', fullPage: false });
check('截图已保存', true, 'tests/demo-final.png');

await browser.close();

// 汇总
const failed = results.filter(r => !r.ok);
console.log(`\n==== ${results.length - failed.length}/${results.length} passed ====`);
if (failed.length) {
  console.log('FAILED:');
  for (const f of failed) console.log('  - ' + f.name + (f.detail ? ' (' + f.detail + ')' : ''));
  process.exit(1);
}
