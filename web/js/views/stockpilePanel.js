// 仓库徽章渲染(顶栏用, 只读): 显示库存总量 + 生产线数量。
// 由 main.js refresh() 每帧调用, 不用订阅。
export function renderStockpileBadge(el, state) {
  if (!el) return;
  const tag = state?.player || '';
  const countries = state?.countries || [];
  const country = countries.find(c => c.tag === tag);
  if (!country) { el.innerHTML = ''; return; }
  const total = Object.values(country.stockpile || {}).reduce((a, b) => a + b, 0);
  const lineCount = (country.production_lines || []).length;
  el.innerHTML = `\u{1F4E6} ${total.toFixed(0)} \u00B7 \u{1F3ED} ${lineCount}`;
  el.title = `库存 ${total.toFixed(1)} 件, ${lineCount} 条生产线`;
}
