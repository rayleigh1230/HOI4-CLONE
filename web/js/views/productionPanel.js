// 生产管理面板: 显示生产线 + 仓库 + 资源概览。
// 注册为 router 面板(左滑入), 复用 panel-host / store 订阅体系。
// 对齐 diplomacyPanel.js / combatPanel.js 的注册模式。
import { h, clear } from '../core/el.js';
import { register } from '../core/router.js';
import { subscribeKeys } from '../core/store.js';
import { setLineFactories, removeProductionLine } from '../engine/commands.js';
import { refresh, log } from '../main.js';

export function init() {
  register('生产', {
    open() {
      const host = document.getElementById('panel-host');
      clear(host);
      host.append(h('h3', { text: '生产管理' }));

      // 玩家标签显示
      const tagLabel = h('div', { style: { fontSize: '12px', color: '#7ec8e3', marginBottom: '8px' } });
      host.append(tagLabel);

      // === 生产线区域(订阅 countries 实时刷新) ===
      const linesBox = h('div', { class: 'diplo-section' });
      const stockBox = h('div', { class: 'diplo-section' });

      const render = (state) => {
        const tag = state?.player || 'GER';
        tagLabel.textContent = `国家: ${tag}`;
        const countries = state?.countries || [];
        const country = countries.find(c => c.tag === tag);

        // --- 生产线 ---
        clear(linesBox);
        linesBox.append(h('div', { class: 'diplo-hd', text: `生产线 (${country ? country.production_lines.length : 0})` }));
        if (!country || country.production_lines.length === 0) {
          linesBox.append(h('div', { class: 'diplo-empty', text: '无生产线' }));
        } else {
          for (const line of country.production_lines) {
            linesBox.append(lineRow(tag, line));
          }
        }

        // --- 仓库(按 chassis 分组) ---
        clear(stockBox);
        const stockpile = country?.stockpile || {};
        const chassisMap = {};
        for (const v of Object.keys(stockpile)) {
          const c = v.replace(/_\d+$/, '');
          if (!chassisMap[c]) chassisMap[c] = [];
          chassisMap[c].push({ variant: v, amount: stockpile[v] });
        }
        stockBox.append(h('div', { class: 'diplo-hd', text: '仓库' }));
        const chassisKeys = Object.keys(chassisMap);
        if (chassisKeys.length === 0) {
          stockBox.append(h('div', { class: 'pp-empty', text: '空' }));
        } else {
          for (const c of chassisKeys) {
            const group = h('div', { class: 'pp-stock-group' }, [
              h('div', { class: 'pp-stock-chassis', text: c }),
            ]);
            for (const item of chassisMap[c]) {
              group.append(h('div', { class: 'pp-stock-variant',
                text: `${item.variant}: ${item.amount.toFixed(1)}` }));
            }
            stockBox.append(group);
          }
        }
      };

      subscribeKeys(['countries', 'player'], render);

      host.append(linesBox, stockBox);
    },
    close() {},
  });
}

// 单条生产线: 编号 + 变体名 + 工厂数 + 效率 + 增/减/删按钮
function lineRow(tag, line) {
  const effs = line.efficiencies || [];
  const activeEffs = effs.slice(0, line.active);
  const avgEff = activeEffs.length
    ? activeEffs.reduce((a, b) => a + b, 0) / activeEffs.length
    : 0;
  return h('div', { class: 'pp-line' }, [
    h('div', { class: 'pp-line-head' }, [
      h('span', { class: 'pp-line-id', text: `#${line.id}` }),
      h('span', { class: 'pp-line-variant', text: line.variant }),
      h('span', { class: 'pp-line-factories', text: `[${line.active}/15]` }),
      h('span', { class: 'pp-line-eff', text: `eff ${(avgEff * 100).toFixed(0)}%` }),
    ]),
    h('div', { class: 'pp-line-actions' }, [
      h('button', {
        class: 'secondary',
        text: '\u2212',
        onclick() {
          setLineFactories(tag, line.id, Math.max(0, line.active - 1));
          refresh();
        },
      }),
      h('button', {
        class: 'secondary',
        text: '+',
        onclick() {
          setLineFactories(tag, line.id, line.active + 1);
          refresh();
        },
      }),
      h('button', {
        text: '删除',
        onclick() {
          if (confirm(`删除生产线 #${line.id}?`)) {
            removeProductionLine(tag, line.id);
            log(`删除生产线 #${line.id}`);
            refresh();
          }
        },
      }),
    ]),
  ]);
}
