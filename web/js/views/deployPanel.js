// 部署面板: 选模板→点省建师(数据驱动 template 路径)
import { h, clear } from '../core/el.js';
import { getTemplates } from '../engine/state.js';
import { register } from '../core/router.js';

export function init() {
  register('部署', {
    open() {
      const host = document.getElementById('panel-host');
      clear(host);
      const templates = getTemplates();
      const sel = h('select', {});
      for (const t of templates) {
        sel.append(h('option', { value: t, text: t }));
      }
      const status = h('div', { style: { color: '#7ec8e3', fontSize: '12px', marginTop: '8px' } });
      host.append(
        h('h3', { text: '部署师' }),
        h('label', { text: '模板' }),
        sel,
        h('button', {
          onclick() {
            const tmpl = sel.value;
            status.textContent = `已选模板「${tmpl}」, 点地图省份部署`;
            window._deployTemplate(tmpl);
          },
          text: '选省部署',
        }),
        status,
      );
    },
    close() {},
  });
}
