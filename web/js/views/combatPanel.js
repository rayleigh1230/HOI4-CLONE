// 交战视窗: 战斗双方 + 预备队(从 bindList 渲染)
import { h, clear } from '../core/el.js';
import { bindList } from '../core/bind.js';
import { register } from '../core/router.js';

export function init() {
  register('交战', {
    open() {
      const host = document.getElementById('panel-host');
      clear(host);
      host.append(h('h3', { text: '交战视窗' }));
      const list = h('div', {});
      bindList(list, 'battles', (b, i, state) => {
        const divs = state.divisions || [];
        const divMap = {};
        for (const d of divs) divMap[d.id] = d;

        const atkOwner = b.atk?.[0] && divMap[b.atk[0]] ? divMap[b.atk[0]].owner : '?';
        const defOwner = b.def?.[0] && divMap[b.def[0]] ? divMap[b.def[0]].owner : '?';

        const unitCard = (id) => {
          const d = divMap[id];
          if (!d) return h('span', { text: `?` });
          const cls = d.owner === 'GER' ? 'ger' : 'fra';
          const orgPct = d.max_org > 0 ? (d.org / d.max_org * 100) : 0;
          const strPct = d.max_str > 0 ? (d.str / d.max_str * 100) : 0;
          return h('div', { class: 'unit-card ' + cls }, [
            h('div', { text: `${d.owner}#${d.id}`, style: { fontSize: '10px', fontWeight: 'bold' } }),
            h('div', { class: 'mini-bar' }, [h('div', { class: 'org', style: { width: orgPct + '%' } })]),
            h('div', { class: 'mini-bar' }, [h('div', { class: 'str', style: { width: strPct + '%' } })]),
          ]);
        };

        return h('div', { class: 'combat-window' }, [
          h('div', { class: 'combat-title', text: `⚔️ 省${b.prov} 交战` }),
          h('div', { text: `攻(${atkOwner}): ${b.atk?.join(',') || ''} VS 守(${defOwner}): ${b.def?.join(',') || ''}`, style: { fontSize: '11px', marginBottom: '4px' } }),
          h('div', { style: { display: 'flex', gap: '4px', flexWrap: 'wrap' } }, (b.atk || []).map(unitCard)),
          h('div', { style: { display: 'flex', gap: '4px', flexWrap: 'wrap', marginTop: '4px' } }, (b.def || []).map(unitCard)),
        ]);
      });
      host.append(list);
    },
    close() {},
  });
}
