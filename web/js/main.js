// 启动入口: 装配 WASM + store + canvas + input + 图层 + 完整 setup
import { loadWasm } from './engine/wasm.js';
import { getState } from './engine/state.js';
import { setPlayer, runSetup, tick, deployTemplate, supply } from './engine/commands.js';
import { store } from './core/store.js';
import * as canvas from './core/canvas.js';
import * as input from './core/input.js';
import * as terrain from './map/layerTerrain.js';
import * as province from './map/layerProvince.js';
import * as unit from './map/layerUnit.js';
import * as order from './map/layerOrder.js';
import * as combat from './map/layerCombat.js';
import * as overlay from './map/layerOverlay.js';
import { init as initDeploy } from './views/deployPanel.js';
import { init as initDiplo } from './views/diplomacyPanel.js';
import { init as initUnitPanel } from './views/unitPanel.js';
import { init as initCombat } from './views/combatPanel.js';
import * as drawer from './ui/drawer.js';
import * as orderMenu from './ui/orderMenu.js';
import { render as renderTopbar } from './ui/topbar.js';
import { render as renderBottombar } from './ui/bottombar.js';
import { statbar } from './ui/statbar.js';
import { h } from './core/el.js';
import { setProvinceController } from './engine/commands.js';
import { provinceAt } from './map/layout.js';
import { selectProvince } from './map/layerProvince.js';
import * as combatLayer from './map/layerCombat.js';
import { setFrontPulse } from './map/layerOverlay.js';
import { setCombatPulse } from './map/layerCombat.js';
import { open as openPanel } from './core/router.js';

// ===== tick 循环 + store 刷新 =====
let autoTimer = null;

export function refresh() {
  store.setState(getState());
  canvas.render(store.state);
}

export function doTick(h) {
  tick(h);
  refresh();
}

// 动画循环: 驱动前线脉冲 + 战斗图标闪烁(rAF)。
// 注: spec §4.4 提"layerUnit 订阅 divisions 脏标记", 但当前 canvas.render 全层重画
// + 本 rAF 持续触发, 牌子已在每次 render 时刷新(含 org/str 实时值)。
// 路径级 markDirty 优化在"无 rAF 全量重画"时才有意义, 当前 rAF 已保证牌子实时, 不额外订阅。
let animPhase = 0;
function animLoop() {
  animPhase += 0.08;
  setFrontPulse(animPhase);
  setCombatPulse(animPhase);
  const view = store.state;
  if (view) canvas.render(view);
  requestAnimationFrame(animLoop);
}

export function toggleTime() {
  if (autoTimer) { clearInterval(autoTimer); autoTimer = null; return false; }
  autoTimer = setInterval(() => doTick(1), 200);
  return true;
}

// ===== 日志(调试用) =====
export function log(msg) {
  const el = document.getElementById('log');
  if (!el) return;
  el.innerHTML += msg + '<br>';
  el.scrollTop = el.scrollHeight;
}

// ===== 主入口 =====
async function main() {
  await loadWasm();
  document.getElementById('loading').style.display = 'none';
  document.getElementById('game').style.display = 'block';

  // 初始化 canvas + input
  canvas.init();
  input.init();

  // 注册 6 图层
  canvas.addLayer('terrain', 0, terrain.draw);
  canvas.addLayer('province', 1, province.draw);
  canvas.addLayer('unit', 2, unit.draw);
  canvas.addLayer('order', 3, order.draw);
  canvas.addLayer('combat', 4, combat.draw);
  canvas.addLayer('overlay', 5, overlay.draw);

  // 注册系统面板
  initDeploy();
  initDiplo();
  initUnitPanel();
  initCombat();

  // 顶栏 + 底栏渲染(时间控制在 bottombar, 对齐 spec §7.1)
  renderTopbar();
  renderBottombar();

  // ===== 点击交互(同步注册, 立即生效) =====
  let selectedDiv = null;
  let deployTarget = null;

  // 部署全局入口(给 deployPanel 用)
  window._deployTemplate = (tmpl) => { deployTarget = tmpl; };

  input.onHit((wp, sx, sy) => {
    const view = store.state;
    if (!view?.provinces?.length) return false;
    const ids = view.provinces.map(p => p.id);

    // 命中优先级 1: 战斗图标(点击开战斗面板)。对齐 spec §5.4
    const cam = canvas.getCamera();
    const icons = combatLayer.combatIcons(view, (p) => canvas.worldToScreen(p), cam.zoom);
    for (const ic of icons) {
      if (Math.hypot(ic.x - sx, ic.y - sy) <= ic.r) {
        openPanel('交战');
        return true;
      }
    }

    // 命中优先级 2: 省份多边形(pointInPolygon)。对齐 spec §3.4
    const best = provinceAt(wp, ids);
    if (best == null) return false;

    // 上帝模式(切控制权)
    const ctrlMode = window._controlMode || false;
    if (ctrlMode) {
      const p = view.provinces.find(x => x.id === best);
      if (p) { setProvinceController(best, p.controller === 'GER' ? 'FRA' : 'GER'); refresh(); }
      return true;
    }

    // 部署模式
    if (deployTarget) {
      deployTemplate(best, deployTarget);
      deployTarget = null;
      drawer.close();
      refresh();
      return true;
    }

    // 已选师 → 点省弹命令菜单
    if (selectedDiv) {
      orderMenu.show(selectedDiv, best);
      selectedDiv = null;
      return true;
    }

    // 选师或弹抽屉
    const divs = view.divisions?.filter(d => d.loc === best) || [];
    const p = view.provinces.find(x => x.id === best);
    selectProvince(best);
    if (divs.length > 0) {
      selectedDiv = divs[0].id;
      drawer.open([
        h('h3', { text: `📍 省${best} [${p?.controller || '?'}]` }),
        ...divs.map(d =>
          h('div', { class: 'div-card ' + (d.owner === 'GER' ? 'attacker' : 'defender') }, [
            h('div', { text: `${d.owner} 师#${d.id} ${d.template || ''}`, style: { fontWeight: 'bold', marginBottom: '4px' } }),
            statbar(d.org, d.max_org, d.str, d.max_str, d.eq_ratio, d.mp_ratio),
          ])
        ),
      ]);
    } else {
      drawer.open(h('h3', { text: `📍 省${best} [${p?.controller || '?'}] — 无部队` }));
    }
    refresh();
    return true;
  });

  input.onBackground(() => { selectedDiv = null; drawer.close(); refresh(); });

  // 初始化场景(新基础构造: create_state + create_province state= + 显式 declare_war)
  setPlayer('GER');
  const script = `
create_state = { id = 1 owner = GER name = "GER Front" }
create_state = { id = 2 owner = FRA name = "FRA Front" }
create_province = { id = 1 state = 1 neighbors = { 2 6 7 } }
create_province = { id = 2 state = 1 neighbors = { 1 3 6 7 8 } }
create_province = { id = 3 state = 1 neighbors = { 2 4 7 8 9 } }
create_province = { id = 4 state = 1 neighbors = { 3 5 8 9 10 } }
create_province = { id = 5 state = 1 neighbors = { 4 9 10 } }
create_province = { id = 6 state = 2 neighbors = { 1 2 7 } }
create_province = { id = 7 state = 2 neighbors = { 1 2 3 6 8 } }
create_province = { id = 8 state = 2 neighbors = { 2 3 4 7 9 } }
create_province = { id = 9 state = 2 neighbors = { 3 4 5 8 10 } }
create_province = { id = 10 state = 2 neighbors = { 4 5 9 } }
declare_war = { attacker = GER defender = FRA }
`;
  runSetup(script);
  supply('GER');
  supply('FRA');

  // 部署初始部队(模板建师: 数据驱动)
  deployTemplate('GER', 1, 'Infanterie-Division');
  deployTemplate('GER', 2, 'Panzer-Division');
  deployTemplate('FRA', 7, 'Division d\'Infanterie');
  deployTemplate('FRA', 8, 'Division d\'Infanterie');

  refresh();
  console.log('[demo] ✓ 引擎+图层跑通, 10省对垒, GER vs FRA, 4 个师(步+甲)');
  requestAnimationFrame(animLoop);  // 启动动画循环(前线/战斗脉冲)
}

main();
