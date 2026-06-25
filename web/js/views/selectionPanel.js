// 部队列表面板: 框选后左侧弹出, 显示框选中的师列表。
// 每师卡片: 兵种 + 模板 + org/str条 + soft/hard/def。点卡片可选中该师下令。对齐用户反馈问题2。
import { h, clear } from '../core/el.js';
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

export function init() {
  register('选中部队', {
    open() {},  // 内容由 showSelection 动态填充, 不走静态 open
    close() {},
  });
}

// 显示框选结果。divisions = 师数组, onSelect = 点某师的回调。
export function showSelection(divisions, onSelect) {
  const host = document.getElementById('panel-host');
  clear(host);
  host.classList.add('open');
  // 注入标题 + 关闭条(router.open 风格, 但这里直接渲染)
  host.append(h('div', { class: 'panel-close-bar' }, [
    h('span', { class: 'panel-title', text: `选中 ${divisions.length} 个师` }),
    h('button', { class: 'panel-close', onclick: closePanel, text: '✖️' }),
  ]));

  if (divisions.length === 0) {
    host.append(h('div', { text: '未框选到部队', style: { color: '#9ab', padding: '12px' } }));
    return;
  }

  for (const d of divisions) {
    const orgPct = d.max_org > 0 ? (d.org / d.max_org * 100) : 0;
    const strPct = d.max_str > 0 ? (d.str / d.max_str * 100) : 0;
    const card = h('div', {
      class: 'div-card ' + (d.owner === 'GER' ? 'attacker' : 'defender'),
      style: { cursor: 'pointer' },
      onclick: () => onSelect(d.id),
    }, [
      h('div', { class: 'div-card-hd' }, [
        h('span', { class: 'div-sym', text: symOf(d.template), style: { color: d.owner === 'GER' ? '#e94560' : '#16c79a' } }),
        h('span', { text: `${d.owner} 师#${d.id}`, style: { fontWeight: 'bold' } }),
        h('span', { text: d.template || '', style: { color: '#9ab', fontSize: '11px' } }),
      ]),
      h('div', { class: 'bars-row' }, [
        h('div', { class: 'mini-bar' }, [h('div', { class: 'org', style: { width: orgPct + '%' } })]),
        h('div', { class: 'mini-bar' }, [h('div', { class: 'str', style: { width: strPct + '%' } })]),
      ]),
      h('div', { class: 'div-stats', text: `SA${Math.round(d.soft_attack||0)} HA${Math.round(d.hard_attack||0)} DEF${Math.round(d.defense||0)}  📍省${d.loc}` }),
    ]);
    host.append(card);
  }
}

export function closePanel() {
  document.getElementById('panel-host').classList.remove('open');
}
