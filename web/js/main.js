// 启动入口: 装配 WASM + store + canvas + input + 图层 + 完整 setup
import { loadWasm } from './engine/wasm.js';
import { getState } from './engine/state.js';
import { setPlayer, runSetup, tick, deployTemplate, supply, moveDivision } from './engine/commands.js';
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
import { init as initCombat, openBattle } from './views/combatPanel.js';
import { init as initSelection, showSelection } from './views/selectionPanel.js';
import * as drawer from './ui/drawer.js';
import * as orderMenu from './ui/orderMenu.js';
import { render as renderTopbar } from './ui/topbar.js';
import { render as renderBottombar } from './ui/bottombar.js';
import { statbar } from './ui/statbar.js';
import { h } from './core/el.js';
import { setProvinceController } from './engine/commands.js';
import { provinceAt, provinceCentroid } from './map/layout.js';
import { selectProvince } from './map/layerProvince.js';
import * as combatLayer from './map/layerCombat.js';
import * as unitLayer from './map/layerUnit.js';
import { setFrontPulse } from './map/layerOverlay.js';
import { setCombatPulse } from './map/layerCombat.js';
import { open as openPanel, close as closePanel } from './core/router.js';

// ===== tick 循环 + store 刷新 =====
let autoTimer = null;

export function refresh() {
  store.setState(getState());
  canvas.render(store.state);
  window._store = store;  // 调试钩子: 供 Playwright 验证读 store.state(非生产代码, 但无害)
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
  initSelection();

  // 顶栏 + 底栏渲染(时间控制在 bottombar, 对齐 spec §7.1)
  renderTopbar();
  renderBottombar();

  // ===== 点击交互(同步注册, 立即生效) =====
  let selectedDiv = null;   // 当前选中师(用于"选师→点省下令")
  let deployTarget = null;

  // 部署全局入口(给 deployPanel 用)
  window._deployTemplate = (tmpl) => { deployTarget = tmpl; };

  // ESC: 关闭所有浮层(面板/抽屉/命令菜单/取消选师)。对齐用户反馈问题4。
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      orderMenu.hide();
      drawer.close();
      closePanel();
      selectedDiv = null;
      unitLayer.clearSelection();
      refresh();
    }
  });

  // 取某师牌子的屏幕包围矩形(与 layerUnit 绘制位置一致: 重心上方28*zoom, 76×24)
  function divCardRect(d, view, zoom) {
    const c = provinceCentroid(d.loc);
    if (!c) return null;
    const sc = canvas.worldToScreen(c);
    const w = 76 * zoom, h = 24 * zoom;
    return { x: sc.x - w / 2, y: sc.y - 28 * zoom - h / 2, w, h, div: d };
  }

  input.onHit((wp, sx, sy) => {
    const view = store.state;
    if (!view?.provinces?.length) return false;
    const cam = canvas.getCamera();
    const zoom = cam.zoom;

    // 命中优先级 1: 战斗图标 → 左侧出该战斗详情框(不跳路由列表)。对齐问题3。
    const icons = combatLayer.combatIcons(view, (p) => canvas.worldToScreen(p), zoom);
    for (const ic of icons) {
      if (Math.hypot(ic.x - sx, ic.y - sy) <= ic.r) {
        drawer.close(); orderMenu.hide();
        openBattle(ic.battleId, view);
        return true;
      }
    }

    // 命中优先级 2: 师牌子(点牌 = 选中师)。对齐问题1(点省/点师区分)。
    const divs = view.divisions || [];
    for (const d of divs) {
      const r = divCardRect(d, view, zoom);
      if (r && sx >= r.x && sx <= r.x + r.w && sy >= r.y && sy <= r.y + r.h) {
        selectedDiv = d.id;
        unitLayer.selectDivision(d.id);
        orderMenu.hide();
        drawer.open([
          h('h3', { text: `🎖 ${d.owner} 师#${d.id}` }),
          h('div', { class: 'div-card ' + (d.owner === 'GER' ? 'attacker' : 'defender') }, [
            h('div', { text: d.template || '(无模板)', style: { fontWeight: 'bold', marginBottom: '4px' } }),
            statbar(d.org, d.max_org, d.str, d.max_str, d.eq_ratio, d.mp_ratio),
            h('div', { text: `📍省${d.loc}  点击他省下令`, style: { fontSize: '11px', color: '#7ec8e3', marginTop: '6px' } }),
          ]),
        ]);
        refresh();
        return true;
      }
    }

    // 命中优先级 3: 省份多边形(pointInPolygon)。对齐 spec §3.4
    const ids = view.provinces.map(p => p.id);
    const best = provinceAt(wp, ids);
    if (best == null) return false;

    // 上帝模式(切控制权)
    if (window._controlMode) {
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

    // 已选师 → 点省弹命令菜单(下令后菜单自动消失, orderMenu 内部 hide)。对齐问题6。
    if (selectedDiv) {
      orderMenu.show(selectedDiv, best);
      refresh();
      return true;
    }

    // 点空省 → 只选中省(金色描边), 弹轻量省信息抽屉(不自动选师)
    const p = view.provinces.find(x => x.id === best);
    const provDivs = view.divisions?.filter(d => d.loc === best) || [];
    selectProvince(best);
    unitLayer.clearSelection();
    drawer.open([
      h('h3', { text: `📍 省${best} [${p?.controller || '?'}]` }),
      provDivs.length > 0
        ? h('div', { text: `驻军 ${provDivs.length} 个师 — 点地图部队牌选中`, style: { fontSize: '12px', color: '#9ab' } })
        : h('div', { text: '无部队驻扎', style: { fontSize: '12px', color: '#9ab' } }),
    ]);
    refresh();
    return true;
  });

  input.onBackground(() => {
    selectedDiv = null;
    unitLayer.clearSelection();
    drawer.close();
    orderMenu.hide();
    refresh();
  });

  // 框选(左键拖拽): 算框住的师 → 左侧出部队列表面板。对齐用户反馈问题2。
  input.onBoxSelect((rect) => {
    const view = store.state;
    if (!view?.divisions?.length) return false;
    const cam = canvas.getCamera();
    const zoom = cam.zoom;
    // 框选矩形是屏幕坐标, 师牌子也用屏幕坐标判断是否相交
    const selected = [];
    for (const d of view.divisions) {
      const r = divCardRect(d, view, zoom);
      if (!r) continue;
      // 牌子中心在框内, 或牌子与框相交 → 选中
      const cx = r.x + r.w / 2, cy = r.y + r.h / 2;
      if (cx >= rect.x0 && cx <= rect.x1 && cy >= rect.y0 && cy <= rect.y1) {
        selected.push(d);
      }
    }
    // 关闭其他浮层
    drawer.close();
    orderMenu.hide();
    if (selected.length > 0) {
      showSelection(selected, (divId) => {
        // 点列表里某师 → 选中它(回地图下令)
        selectedDiv = divId;
        unitLayer.selectDivision(divId);
        refresh();
      });
    } else {
      closePanel();
    }
    return true;
  });

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

  // 让 GER 师进攻 FRA 前线省(省7 有 FRA 驻军), 制造初始战斗 —
  // 这样一进 demo 就有战斗可视化内容(战斗小圆/战斗面板), 便于展示。
  // 玩家可观察战斗进程; 战斗结束后可再下令制造新战斗。
  moveDivision(1, 7);  // GER 师_gateway 1(省1)→省7(FRA), 邻接, 触发战斗

  refresh();
  console.log('[demo] ✓ 引擎+图层跑通, 10省对垒, GER vs FRA, 4 个师(步+甲), GER 进攻省7');
  requestAnimationFrame(animLoop);  // 启动动画循环(前线/战斗脉冲)
}

main();
