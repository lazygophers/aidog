/* ===== datepicker.js · 纯 JS 日历（全主题可控，无原生 input） ===== */
const DP_STATE = new WeakMap();
const WD = ['日', '一', '二', '三', '四', '五', '六'];
const MON = ['1月','2月','3月','4月','5月','6月','7月','8月','9月','10月','11月','12月'];

function dpInit(el) {
  const withTime = el.hasAttribute('data-with-time');
  const monthOnly = el.hasAttribute('data-month-only');
  DP_STATE.set(el, { view: new Date(), sel: null, withTime, monthOnly, hour: 9, min: 0 });
}
function dpEnsureId(el) { if (!el.id) el.id = 'dp-' + Math.random().toString(36).slice(2, 9); return el.id; }

function toggleDatepicker(el) {
  const wasOpen = el.classList.contains('open');
  document.querySelectorAll('.datepicker.open').forEach((d) => d.classList.remove('open'));
  if (!wasOpen) { el.classList.add('open'); dpRender(el); }
}

function dpRender(el) {
  const s = DP_STATE.get(el); if (!s) return;
  const p = el.querySelector('.datepicker-panel');
  const y = s.view.getFullYear(), m = s.view.getMonth();
  const title = s.monthOnly ? `${y} 年` : `${y} 年 ${MON[m]}`;
  let html = `<div class="dp-header">
    <button class="dp-nav" onclick="dpMove('${el.id}',-1)">‹</button>
    <span class="dp-title">${title}</span>
    <button class="dp-nav" onclick="dpMove('${el.id}',1)">›</button>
  </div>`;
  if (s.monthOnly) {
    html += '<div class="dp-days" style="grid-template-columns:repeat(3,1fr);gap:6px">';
    for (let i = 0; i < 12; i++) {
      const sel = s.sel && s.sel.getFullYear() === y && s.sel.getMonth() === i;
      html += `<button class="dp-day ${sel ? 'selected' : ''}" style="aspect-ratio:auto;padding:10px;border-radius:var(--radius-md)" onclick="dpPickMonth('${el.id}',${i})">${MON[i]}</button>`;
    }
    html += '</div>';
  } else {
    html += '<div class="dp-weekdays">';
    WD.forEach((w) => { html += `<div class="dp-weekday">${w}</div>`; });
    html += '</div><div class="dp-days">';
    const first = new Date(y, m, 1).getDay();
    const days = new Date(y, m + 1, 0).getDate();
    const prevDays = new Date(y, m, 0).getDate();
    const today = new Date();
    for (let i = 0; i < first; i++) html += `<button class="dp-day muted" disabled>${prevDays - first + 1 + i}</button>`;
    for (let d = 1; d <= days; d++) {
      const isToday = today.getFullYear() === y && today.getMonth() === m && today.getDate() === d;
      const sel = s.sel && s.sel.getFullYear() === y && s.sel.getMonth() === m && s.sel.getDate() === d;
      html += `<button class="dp-day ${isToday ? 'today' : ''} ${sel ? 'selected' : ''}" onclick="dpPickDay('${el.id}',${d})">${d}</button>`;
    }
    const total = first + days;
    const trail = (7 - total % 7) % 7;
    for (let i = 1; i <= trail; i++) html += `<button class="dp-day muted" disabled>${i}</button>`;
    html += '</div>';
    if (s.withTime) {
      html += `<div class="timepicker-panel">
        <div class="tp-select"><button class="tp-btn" onclick="dpHour('${el.id}',1)">▲</button><span class="tp-val">${String(s.hour).padStart(2,'0')}</span><button class="tp-btn" onclick="dpHour('${el.id}',-1)">▼</button></div>
        <span class="tp-colon">:</span>
        <div class="tp-select"><button class="tp-btn" onclick="dpMin('${el.id}',1)">▲</button><span class="tp-val">${String(s.min).padStart(2,'0')}</span><button class="tp-btn" onclick="dpMin('${el.id}',-1)">▼</button></div>
      </div>`;
    }
    html += `<div class="dp-footer"><button class="btn" onclick="dpToday('${el.id}')">今天</button><button class="btn btn-primary" onclick="dpConfirm('${el.id}')">确定</button></div>`;
  }
  p.innerHTML = html;
}

function dpMove(id, delta) {
  const el = document.getElementById(id); const s = DP_STATE.get(el);
  if (s.monthOnly) s.view.setFullYear(s.view.getFullYear() + delta);
  else s.view.setMonth(s.view.getMonth() + delta);
  dpRender(el);
}
function dpPickDay(id, d) {
  const el = document.getElementById(id); const s = DP_STATE.get(el);
  s.sel = new Date(s.view.getFullYear(), s.view.getMonth(), d);
  dpRender(el);
}
function dpPickMonth(id, mi) {
  const el = document.getElementById(id); const s = DP_STATE.get(el);
  s.sel = new Date(s.view.getFullYear(), mi, 1);
  dpFmt(el); el.classList.remove('open');
}
function dpHour(id, delta) {
  const el = document.getElementById(id); const s = DP_STATE.get(el);
  s.hour = (s.hour + delta + 24) % 24; dpRender(el);
}
function dpMin(id, delta) {
  const el = document.getElementById(id); const s = DP_STATE.get(el);
  s.min = (s.min + delta + 60) % 60; dpRender(el);
}
function dpToday(id) {
  const el = document.getElementById(id); const s = DP_STATE.get(el);
  s.sel = new Date(); s.view = new Date(s.sel); dpRender(el);
}
function dpConfirm(id) { const el = document.getElementById(id); dpFmt(el); el.classList.remove('open'); }

function dpFmt(el) {
  const s = DP_STATE.get(el);
  if (!s.sel) return;
  const val = el.querySelector('.datepicker-value');
  const inp = el.querySelector('.datepicker-input');
  inp.classList.remove('placeholder');
  const y = s.sel.getFullYear();
  const mo = String(s.sel.getMonth() + 1).padStart(2, '0');
  const d = String(s.sel.getDate()).padStart(2, '0');
  if (s.monthOnly) val.textContent = `${y}-${mo}`;
  else if (s.withTime) val.textContent = `${y}-${mo}-${d} ${String(s.hour).padStart(2,'0')}:${String(s.min).padStart(2,'0')}`;
  else val.textContent = `${y}-${mo}-${d}`;
}

document.addEventListener('DOMContentLoaded', () => {
  document.querySelectorAll('[data-datepicker]').forEach((el) => { dpEnsureId(el); dpInit(el); });
});
