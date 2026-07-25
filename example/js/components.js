/* ===== components.js · 通用组件交互（Modal/Drawer/Collapse/Radio/Segmented/Switch/Rate/Tag/Dropdown） ===== */

// 入场动画触发
function triggerAnim(item, cls) {
  const el = item.querySelector('.target');
  if (!el) return;
  el.classList.remove('anim-fade', 'anim-slide', 'anim-scale', 'anim-bounce', 'anim-flip', 'anim-shake');
  void el.offsetWidth;
  el.classList.add(cls);
}

// Collapse
function toggleCollapse(header) { header.parentElement.classList.toggle('open'); }

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

// Radio Group
function pickRadio(btn) {
  const group = btn.parentElement;
  group.querySelectorAll('.radio-btn').forEach((b) => b.classList.remove('active'));
  btn.classList.add('active');
}

// Segmented
function pickSegmented(btn) {
  const group = btn.parentElement;
  group.querySelectorAll('.segmented-item').forEach((b) => b.classList.remove('active'));
  btn.classList.add('active');
}

// Switch
function toggleSwitch(el) { el.classList.toggle('active'); }

// Tag removable
function removeTag(el) { el.remove(); }

// Dropdown — 自定义 Select
function toggleDropdown(dd) {
  const wasOpen = dd.classList.contains('open');
  document.querySelectorAll('.dropdown.open').forEach((d) => d.classList.remove('open'));
  if (!wasOpen) dd.classList.add('open');
}
function pickDropdown(opt, label) {
  const dd = opt.closest('.dropdown');
  dd.querySelector('.dropdown-value').textContent = label;
  dd.querySelectorAll('.dropdown-option').forEach((o) => o.classList.remove('selected'));
  opt.classList.add('selected');
  dd.classList.remove('open');
}

// Rate（委托）
document.addEventListener('click', (e) => {
  const star = e.target.closest('.rate-star');
  if (!star) return;
  const group = star.closest('.rate');
  if (!group) return;
  const stars = Array.from(group.querySelectorAll('.rate-star'));
  const idx = stars.indexOf(star);
  stars.forEach((s, i) => s.classList.toggle('filled', i <= idx));
});

// 点外部关 dropdown / datepicker
document.addEventListener('click', (e) => {
  if (!e.target.closest('.dropdown')) document.querySelectorAll('.dropdown.open').forEach((d) => d.classList.remove('open'));
  if (!e.target.closest('.datepicker')) document.querySelectorAll('.datepicker.open').forEach((d) => d.classList.remove('open'));
});
