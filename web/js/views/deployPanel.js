// 部署面板: 选国家 + 选模板 → 点省建师(数据驱动 template 路径)。
// 对齐用户反馈: 部署需区分国家(可部署 GER 或 FRA 师)。
import { h, clear } from '../core/el.js';
import { getTemplates } from '../engine/state.js';
import { register } from '../core/router.js';

export function init() {
  register('部署', {
    open() {
      const host = document.getElementById('panel-host');
      clear(host);
      const templates = getTemplates();
      const player = (window._store?.state?.player) || 'GER';
      // 模板选择
      const tmplSel = h('select', {});
      for (const t of templates) tmplSel.append(h('option', { value: t, text: t }));
      const status = h('div', { style: { color: '#7ec8e3', fontSize: '12px', marginTop: '8px' } });
      host.append(
        h('h3', { text: '部署师' }),
        h('div', { text: `国家: ${player} (当前视角)`, style: { color: '#9ab', fontSize: '12px', marginBottom: '8px' } }),
        h('label', { text: '模板' }), tmplSel,
        h('button', {
          onclick() {
            const tmpl = tmplSel.value;
            status.textContent = `已选 ${player} ${tmpl}, 点地图省份部署`;
            window._deployTemplate(player, tmpl);  // owner 锁定 player(国家视角)
          },
          text: '选省部署',
        }),
        status,
      );
    },
    close() {},
  });
}
