// 交战视窗: landcombatview 风格。攻守双方 + 师条目(兵种+org/str竖条+soft/hard/defense)。
// 对齐 spec §5.3。两种入口:
//   1. 顶栏"交战"按钮 → router 打开, 列出所有战斗
//   2. 点战斗气泡 → openBattle(id) 直接打开该战斗详情(左侧出框, 不跳路由列表)
import { h, clear } from '../core/el.js';
import { bindList } from '../core/bind.js';
import { register } from '../core/router.js';

const SYMBOLS = { infantry: '▦', armor: '◆', artillery: '◎', mechanized: '◇' };
function symOf(template) {
  if (!template) return SYMBOLS.infantry;
  const t = template.toLowerCase();
  if (t.includes('panzer') || t.includes('armor')) return SYMBOLS.armor;
  if (t.includes('artiller')) return SYMBOLS.artillery;
  if (t.includes('mecan') || t.includes('motor')) return SYMBOLS.mechanized;
  return SYMBOLS.infantry;
}

// 师条目: 兵种 + org/str条 + soft/hard/defense。对齐 division_combat_attacker_entry。
function divEntry(d) {
  const orgPct = d.max_org > 0 ? (d.org / d.max_org * 100) : 0;
  const strPct = d.max_str > 0 ? (d.str / d.max_str * 100) : 0;
  return h('div', { class: 'combat-div ' + (d.owner === 'GER' ? 'ger' : 'fra') }, [
    h('span', { class: 'sym', text: symOf(d.template) }),
    h('div', { class: 'bars' }, [
      h('div', { class: 'mini-bar' }, [h('div', { class: 'org', style: { width: orgPct + '%' } })]),
      h('div', { class: 'mini-bar' }, [h('div', { class: 'str', style: { width: strPct + '%' } })]),
    ]),
    h('div', { class: 'stats', text: `SA${Math.round(d.soft_attack||0)} HA${Math.round(d.hard_attack||0)} DEF${Math.round(d.defense||0)}` }),
  ]);
}

function progOf(b, divMap) {
  let mx = 0, ox = 0;
  for (const id of b.atk || []) { const d = divMap[id]; if (d) { mx += d.max_org; ox += d.org; } }
  return mx > 0 ? Math.max(0, Math.min(1, (mx - ox) / mx)) : 0;
}

// 渲染单场战斗详情到给定容器
function renderBattle(host, b, state) {
  const divs = state.divisions || [];
  const divMap = {}; for (const d of divs) divMap[d.id] = d;
  const atkOwner = b.atk?.[0] && divMap[b.atk[0]] ? divMap[b.atk[0]].owner : '?';
  const defOwner = b.def?.[0] && divMap[b.def[0]] ? divMap[b.def[0]].owner : '?';
  const prog = progOf(b, divMap);
  const totalWidth = (b.atk || []).reduce((s, id) => s + (divMap[id]?.combat_width || 0), 0);
  host.append(h('div', { class: 'combat-window' }, [
    h('div', { class: 'combat-title', text: `⚔ 省${b.prov} 交战` }),
    h('div', { class: 'mini-bar', style: { height: '8px', margin: '6px 0' } },
      [h('div', { class: 'str', style: { width: (prog * 100) + '%' } })]),
    h('div', { text: `进度 ${Math.round(prog * 100)}%  |  宽度 ${Math.round(totalWidth)}`, style: { fontSize: '11px', color: '#9ab', marginBottom: '8px' } }),
    h('div', { class: 'combat-cols' }, [
      h('div', { class: 'combat-side' }, [
        h('div', { class: 'side-hd', text: `攻方 ${atkOwner} (${(b.atk||[]).length}师 +${(b.res_atk||[]).length}预备)` }),
        ...(b.atk || []).map(id => divMap[id] ? divEntry(divMap[id]) : null).filter(Boolean),
      ]),
      h('div', { class: 'combat-side' }, [
        h('div', { class: 'side-hd', text: `守方 ${defOwner} (${(b.def||[]).length}师 +${(b.res_def||[]).length}预备)` }),
        ...(b.def || []).map(id => divMap[id] ? divEntry(divMap[id]) : null).filter(Boolean),
      ]),
    ]),
  ]));
}

export function init() {
  register('交战', {
    open() {
      const host = document.getElementById('panel-host');
      clear(host);
      host.append(h('h3', { text: '交战视窗' }));
      const list = h('div', {});
      bindList(list, 'battles', (b, i, state) => {
        const wrapper = h('div', {});
        renderBattle(wrapper, b, state);
        return wrapper;
      });
      host.append(list);
    },
    close() {},
  });
}

// 点战斗气泡直接打开该战斗详情(左侧出框)。对齐用户反馈问题3。
// battleId 对应 get_state battles[].id。需 main.js 传入 store.state。
// 自带关闭条(不经过 router.open, 故自行注入关闭按钮)。
export function openBattle(battleId, state) {
  const host = document.getElementById('panel-host');
  clear(host);
  // 关闭条(标题 + ✖️)
  host.append(h('div', { class: 'panel-close-bar' }, [
    h('span', { class: 'panel-title', text: '战斗详情' }),
    h('button', { class: 'panel-close', onclick: () => host.classList.remove('open'), text: '✖️' }),
  ]));
  const b = (state.battles || []).find(x => x.id === battleId);
  if (!b) {
    host.append(h('div', { text: '该战斗已结束', style: { color: '#9ab', padding: '12px' } }));
  } else {
    renderBattle(host, b, state);
  }
  host.classList.add('open');
}
