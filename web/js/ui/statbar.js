// 状态条组件(4 条迷你状态条, 复用于 NATO 牌子和部队卡片)
import { h } from '../core/el.js';

export function statbar(org, maxOrg, str, maxStr, eqRatio, mpRatio) {
  const bar = (cls, pct) => h('div', { class: 'mini-bar' }, [
    h('div', { class: cls, style: { width: Math.min(100, Math.max(0, pct)) + '%' } }),
  ]);
  const orgPct = maxOrg > 0 ? org / maxOrg * 100 : 0;
  const strPct = maxStr > 0 ? str / maxStr * 100 : 0;
  const eqPct = (eqRatio || 0) * 100;
  const mpPct = (mpRatio || 0) * 100;
  return h('div', { class: 'unit-card' }, [
    bar('org', orgPct),
    bar('str', strPct),
    bar('eq', eqPct),
    bar('mp', mpPct),
  ]);
}
