// 部队列表: 全部队概览(用 bindList 绑定 divisions 数组)
import { h, clear } from '../core/el.js';
import { bindList } from '../core/bind.js';
import { register } from '../core/router.js';
import { changeTemplate } from '../engine/commands.js';
import { getTemplates } from '../engine/state.js';
import { log, refresh } from '../main.js';

let tmplNames = [];

export function init() {
  register('部队', {
    open() {
      const host = document.getElementById('panel-host');
      clear(host);
      tmplNames = getTemplates();
      host.append(h('h3', { text: '全部队' }));
      const list = h('div', {});
      bindList(list, 'divisions', (d, i, state) => {
        const status = d.annihilated ? '💀歼灭' : d.retreating ? '🚩撤退' : d.attacking ? '⚔️进攻' : d.dest ? '🚶移动' : '🛡️驻扎';
        const orgPct = d.max_org > 0 ? (d.org / d.max_org * 100) : 0;
        const strPct = d.max_str > 0 ? (d.str / d.max_str * 100) : 0;
        return h('div', { class: 'div-card ' + (d.owner === 'GER' ? 'attacker' : 'defender') }, [
          h('div', { style: { display: 'flex', justifyContent: 'space-between', fontSize: '12px' } }, [
            h('span', { text: `${d.owner} 师#${d.id} 📍${d.loc}  ${status}` }),
            h('span', { text: `${d.template || '(无模板)'}` }),
          ]),
          h('div', { style: { fontSize: '11px', color: '#9ab', margin: '4px 0' } }, [
            h('div', { text: `Organisation: ${d.org.toFixed(1)}/${d.max_org.toFixed(0)} (${orgPct.toFixed(0)}%)` }),
            h('div', { text: `HP: ${d.str.toFixed(1)}/${d.max_str.toFixed(0)} (${strPct.toFixed(0)}%)` }),
          ]),
          // 换模板下拉(只对有模板引用的师显示)
          d.template ? h('select', {
            onchange(e) {
              const newTmpl = e.target.value;
              if (newTmpl && newTmpl !== d.template) {
                changeTemplate(d.id, newTmpl);
                log(`师#${d.id} 换模板: ${d.template} → ${newTmpl}`);
                refresh();
              }
            },
          }, [
            h('option', { value: d.template, text: `当前: ${d.template}` }),
            ...tmplNames.filter(t => t !== d.template).map(t => h('option', { value: t, text: t })),
          ]) : null,
        ]);
      });
      host.append(list);
    },
    close() {},
  });
}
