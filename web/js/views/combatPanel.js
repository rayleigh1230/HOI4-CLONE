// 交战视窗: landcombatview 风格。攻守双方 + 师条目(兵种+org/str竖条+soft/hard/defense)。
// 对齐 spec §5.3。点战斗图标打开本面板(Task 11 main.js 接入)。
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

// 师条目(对齐 division_combat_attacker_entry): 兵种 + org/str条 + soft/hard/defense
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

// 战斗进度(攻方 org 损耗比)
function progOf(b, divMap) {
  let mx = 0, ox = 0;
  for (const id of b.atk || []) { const d = divMap[id]; if (d) { mx += d.max_org; ox += d.org; } }
  return mx > 0 ? Math.max(0, Math.min(1, (mx - ox) / mx)) : 0;
}

export function init() {
  register('交战', {
    open() {
      const host = document.getElementById('panel-host');
      clear(host);
      host.append(h('h3', { text: '交战视窗' }));
      const list = h('div', {});
      bindList(list, 'battles', (b, i, state) => {
        const divs = state.divisions || [];
        const divMap = {}; for (const d of divs) divMap[d.id] = d;
        const atkOwner = b.atk?.[0] && divMap[b.atk[0]] ? divMap[b.atk[0]].owner : '?';
        const defOwner = b.def?.[0] && divMap[b.def[0]] ? divMap[b.def[0]].owner : '?';
        const prog = progOf(b, divMap);
        const totalWidth = (b.atk || []).reduce((s, id) => s + (divMap[id]?.combat_width || 0), 0);

        return h('div', { class: 'combat-window' }, [
          h('div', { class: 'combat-title', text: `⚔ 省${b.prov} 交战` }),
          // 进度条 + 宽度
          h('div', { class: 'mini-bar', style: { height: '8px', margin: '6px 0' } },
            [h('div', { class: 'str', style: { width: (prog * 100) + '%' } })]),
          h('div', { text: `进度 ${Math.round(prog * 100)}%  |  宽度 ${Math.round(totalWidth)}`, style: { fontSize: '11px', color: '#9ab', marginBottom: '8px' } }),
          // 攻守两栏
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
        ]);
      });
      host.append(list);
    },
    close() {},
  });
}
