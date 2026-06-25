// 外交面板: 宣战/阵营/和谈(显式 declare_war 入口)
import { h, clear } from '../core/el.js';
import { declareWar, createFaction, joinFaction, whitePeace } from '../engine/commands.js';
import { register } from '../core/router.js';
import { log, refresh } from '../main.js';

export function init() {
  register('外交', {
    open() {
      const host = document.getElementById('panel-host');
      clear(host);
      host.append(
        h('h3', { text: '外交' }),
        h('label', { text: '宣战: GER → FRA' }),
        h('button', { onclick() { declareWar('GER', 'FRA'); log('GER 宣战 FRA'); refresh(); } }, 'GER 宣 FRA'),
        h('label', { text: '白和: GER ↔ FRA' }),
        h('button', { onclick() { whitePeace('GER', 'FRA'); log('GER↔FRA 白和'); refresh(); } }, '白和'),
        h('hr', { style: { borderColor: '#0f3460', margin: '10px 0' } }),
        h('label', { text: '创建/加入阵营' }),
        h('button', { onclick() { createFaction('GER', 'Axis'); log('GER 创建 Axis'); refresh(); } }, 'GER 创建 Axis'),
        h('button', { onclick() { joinFaction('FRA', 'Axis'); log('FRA 加入 Axis'); refresh(); } }, 'FRA 加入 Axis'),
      );
    },
    close() {},
  });
}
