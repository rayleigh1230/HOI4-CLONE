// 外交面板: 国家选择 + 当前战争/阵营状态展示 + 宣战/和谈/阵营操作。
// 对齐用户反馈: 完善外交系统(不再是写死的 GER/FRA 按钮)。
import { h, clear } from '../core/el.js';
import { declareWar, createFaction, joinFaction, whitePeace } from '../engine/commands.js';
import { register } from '../core/router.js';
import { subscribe } from '../core/store.js';
import { log, refresh } from '../main.js';

const NATIONS = ['GER', 'FRA'];

export function init() {
  register('外交', {
    open() {
      const host = document.getElementById('panel-host');
      clear(host);
      host.append(h('h3', { text: '外交' }));

      // === 当前战争状态(实时订阅 wars) ===
      const warsBox = h('div', { class: 'diplo-section' });
      const renderWars = (state) => {
        clear(warsBox);
        const wars = state?.wars || [];
        warsBox.append(h('div', { class: 'diplo-hd', text: `当前战争 (${wars.length})` }));
        if (wars.length === 0) {
          warsBox.append(h('div', { class: 'diplo-empty', text: '无进行中的战争' }));
        }
        for (const w of wars) {
          warsBox.append(h('div', { class: 'diplo-war' }, [
            h('span', { class: 'atk', text: w.atk.join('/') }),
            h('span', { text: ' ⚔ ', style: { color: '#e94560' } }),
            h('span', { class: 'def', text: w.def.join('/') }),
          ]));
        }
      };
      subscribe(renderWars);

      // === 阵营状态(实时订阅 factions) ===
      const facBox = h('div', { class: 'diplo-section' });
      const renderFactions = (state) => {
        clear(facBox);
        const fac = state?.factions || {};
        const tags = Object.keys(fac);
        facBox.append(h('div', { class: 'diplo-hd', text: '阵营归属' }));
        for (const t of NATIONS) {
          facBox.append(h('div', { class: 'diplo-row' }, [
            h('span', { text: t, style: { fontWeight: 'bold', width: '40px' } }),
            h('span', { text: fac[t] || '— 无阵营', style: { color: fac[t] ? '#7ec8e3' : '#666' } }),
          ]));
        }
      };
      subscribe(renderFactions);

      // === 操作区: 选两个国家执行外交动作 ===
      const aSel = h('select', {});
      const bSel = h('select', {});
      for (const t of NATIONS) {
        aSel.append(h('option', { value: t, text: t }));
        bSel.append(h('option', { value: t, text: t }));
      }
      bSel.value = NATIONS[1];  // 默认 A=GER B=FRA

      const opRow = (label, fn, danger) => h('button', {
        class: danger ? '' : 'secondary',
        style: { flex: 1 },
        onclick() {
          const a = aSel.value, b = bSel.value;
          if (a === b) { log('⚠️ 不能对自己执行外交动作'); return; }
          fn(a, b);
          refresh();
        },
      }, label);

      const facName = h('input', { placeholder: '阵营名(如 Axis)', style: { flex: 1 } });

      host.append(
        warsBox,
        facBox,
        h('div', { class: 'diplo-hd', text: '外交操作', style: { marginTop: '12px' } }),
        h('div', { class: 'diplo-op-row' }, [
          aSel, h('span', { text: '→', style: { padding: '0 6px' } }), bSel,
        ]),
        h('div', { class: 'diplo-op-row' }, [
          opRow('⚔️ 宣战', declareWar, true),
          opRow('🕊️ 白和', whitePeace, false),
        ]),
        h('div', { class: 'diplo-hd', text: '阵营', style: { marginTop: '8px' } }),
        h('div', { class: 'diplo-op-row' }, [
          facName,
          h('button', { class: 'secondary', onclick() {
            const name = facName.value.trim() || 'Axis';
            createFaction(aSel.value, name); log(`${aSel.value} 创建阵营 ${name}`); refresh();
          } }, '创建阵营'),
          h('button', { class: 'secondary', onclick() {
            const name = facName.value.trim() || 'Axis';
            joinFaction(aSel.value, name); log(`${aSel.value} 加入阵营 ${name}`); refresh();
          } }, '加入阵营'),
        ]),
      );
    },
    close() {},
  });
}
