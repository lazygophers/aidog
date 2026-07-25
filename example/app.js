/* ===== Design System · app.js ===== */
// 主题初始化 + 持久化
(function initTheme() {
  const saved = localStorage.getItem('ds-theme');
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  const theme = saved || (prefersDark ? 'dark' : 'light');
  document.documentElement.setAttribute('data-theme', theme);
  document.addEventListener('DOMContentLoaded', () => updateThemeIcon(theme));
})();

function toggleTheme() {
  const cur = document.documentElement.getAttribute('data-theme');
  const next = cur === 'dark' ? 'light' : 'dark';
  document.documentElement.setAttribute('data-theme', next);
  localStorage.setItem('ds-theme', next);
  updateThemeIcon(next);
}

function updateThemeIcon(theme) {
  const icon = document.getElementById('themeIcon');
  const txt = document.getElementById('themeText');
  if (!icon) return;
  const sun = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/></svg>';
  const moon = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/></svg>';
  icon.innerHTML = theme === 'dark' ? sun : moon;
  if (txt) txt.textContent = theme === 'dark' ? '亮色' : '暗色';
}

// Tab 切换
document.addEventListener('DOMContentLoaded', () => {
  document.querySelectorAll('.tab').forEach(tab => {
    tab.addEventListener('click', () => {
      const target = tab.dataset.tab;
      document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
      document.querySelectorAll('.panel').forEach(p => p.classList.remove('active'));
      tab.classList.add('active');
      const panel = document.querySelector(`[data-panel="${target}"]`);
      if (panel) panel.classList.add('active');
    });
  });
});

// 入场动画触发
function triggerAnim(item, cls) {
  const el = item.querySelector('.target');
  if (!el) return;
  el.classList.remove('anim-fade', 'anim-slide', 'anim-scale', 'anim-bounce');
  void el.offsetWidth;
  el.classList.add(cls);
}

// Collapse
function toggleCollapse(header) {
  header.parentElement.classList.toggle('open');
}

// Modal
function openModal(id) { document.getElementById(id).classList.add('open'); }
function closeModal(id) { document.getElementById(id).classList.remove('open'); }

// Drawer — id 传 mask 的 id，面板约定为同 id + '-real'
function openDrawer(id) {
  document.getElementById(id).classList.add('open');
  const p = document.getElementById(id + '-real');
  if (p) p.classList.add('open');
}
function closeDrawer(id) {
  document.getElementById(id).classList.remove('open');
  const p = document.getElementById(id + '-real');
  if (p) p.classList.remove('open');
}

// Radio Group (segmented 按钮组)
function pickRadio(btn) {
  const group = btn.parentElement;
  group.querySelectorAll('.radio-btn').forEach(b => b.classList.remove('active'));
  btn.classList.add('active');
}

// Segmented
function pickSegmented(btn) {
  const group = btn.parentElement;
  group.querySelectorAll('.segmented-item').forEach(b => b.classList.remove('active'));
  btn.classList.add('active');
}

// Switch toggle
function toggleSwitch(el) { el.classList.toggle('active'); }

// Rate (委托：点击设值，data-rate 标记组内总数)
document.addEventListener('click', (e) => {
  const star = e.target.closest('.rate-star');
  if (!star) return;
  const group = star.closest('.rate');
  if (!group) return;
  const stars = Array.from(group.querySelectorAll('.rate-star'));
  const idx = stars.indexOf(star);
  stars.forEach((s, i) => s.classList.toggle('filled', i <= idx));
});

// Tag removable
function removeTag(el) { el.remove(); }
