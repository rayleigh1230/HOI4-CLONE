// 启动入口: 装配 WASM + store + canvas + input + 图层 + 完整 setup
import { loadWasm } from './engine/wasm.js';
import { getState } from './engine/state.js';
import { setPlayer, runSetup, tick, deployTemplate, supply } from './engine/commands.js';
import { store, subscribe } from './core/store.js';
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
import { init as initUnit } from './views/unitPanel.js';
import { init as initCombat } from './views/combatPanel.js';

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
  initUnit();
  initCombat();

  // 顶栏渲染
  import('./ui/topbar.js').then(({ render }) => render());

  // 渲染循环: store 变化 → canvas 重绘
  subscribe(() => canvas.render(store.state));

  // ===== 两段式交互(选师→点省弹菜单 / 点省弹抽屉) =====
  let selectedDiv = null;
  let deployTarget = null;  // 部署模式: 选的模板名, 待点省
  import('./map/layerProvince.js').then(({ selectProvince, getSelected }) => {
    import('./map/layout.js').then(({ provincePos }) => {
      import('./ui/topbar.js').then(({ isControlMode }) => {
        import('./ui/orderMenu.js').then(menu => {
          import('./ui/drawer.js').then(drawer => {
            import('./ui/statbar.js').then(({ statbar }) => {
              import('./core/el.js').then(({ h }) => {
                import('./engine/commands.js').then(cmd => {

  input.onHit((wp, sx, sy) => {
    const view = store.state;
    if (!view?.provinces?.length) return false;

    // 找最近的省份(worldToScreen 逆算)
    const ids = view.provinces.map(p => p.id);
    let best = null, bestD = 44;
    for (const p of view.provinces) {
      const sp = canvas.worldToScreen(provincePos(p.id, ids, window.innerWidth, window.innerHeight));
      const d = Math.hypot(sp.x - (sx), sp.y - (sy));
      if (d < bestD) { bestD = d; best = p.id; }
    }
    if (best == null) return false;

    // 上帝模式: 切控制权
    if (isControlMode()) {
      const p = view.provinces.find(x => x.id === best);
      if (p) {
        cmd.setProvinceController(best, p.controller === 'GER' ? 'FRA' : 'GER');
        log(`省${best} 控制权切换`);
        refresh();
      }
      return true;
    }

    // 部署模式: 选省建师
    if (deployTarget) {
      cmd.deployTemplate(view.player || 'GER', best, deployTarget);
      log(`部署 ${deployTarget}→省${best}`);
      deployTarget = null;
      drawer.close();
      refresh();
      return true;
    }

    // 两段式: 已选师 → 弹命令菜单
    if (selectedDiv) {
      menu.show(selectedDiv, best);
      selectedDiv = null;
      return true;
    }

    // 选师(选中该省第一个师) 或 弹抽屉
    const divs = view.divisions?.filter(d => d.loc === best) || [];
    if (divs.length > 0) {
      selectedDiv = divs[0].id;
      log(`选中师#${selectedDiv}, 点目标省选命令`);
      // 弹抽屉显示该省部队
      const p = view.provinces.find(x => x.id === best);
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
      const p = view.provinces.find(x => x.id === best);
      drawer.open(h('h3', { text: `📍 省${best} [${p?.controller || '?'}] — 无部队` }));
    }
    selectProvince(best);
    refresh();
    return true;
  });

  input.onBackground(() => {
    selectedDiv = null;
    drawer.close();
    refresh();
  });

  // 部署函数暴露到全局(顶栏/部署面板调用)
  window._deployTemplate = (tmpl) => { deployTarget = tmpl; };

                }));
              });
            });
          });
        });
      });
    });
  });

  // 顶栏渲染
  import('./ui/topbar.js').then(({ render }) => render());

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
}

main();
