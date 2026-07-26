/* ===== motion.js · Ripple / Counter / Reveal · 进阶动效 ===== */

// Ripple — 按钮涟漪
document.addEventListener('click', (e) => {
  const btn = e.target.closest('.ripple');
  if (!btn) return;
  const r = btn.getBoundingClientRect();
  const wave = document.createElement('span');
  wave.className = 'ripple-wave';
  const size = Math.max(r.width, r.height);
  wave.style.width = wave.style.height = size + 'px';
  wave.style.left = (e.clientX - r.left - size / 2) + 'px';
  wave.style.top = (e.clientY - r.top - size / 2) + 'px';
  btn.appendChild(wave);
  setTimeout(() => wave.remove(), 600);
});

// Counter — 数字滚动（IntersectionObserver 触发一次）
function animateCounter(el) {
  const target = parseFloat(el.dataset.target);
  const decimals = parseInt(el.dataset.decimals || '0', 10);
  const dur = 1200;
  const start = performance.now();
  function tick(now) {
    const p = Math.min((now - start) / dur, 1);
    const eased = 1 - Math.pow(1 - p, 3);
    const val = target * eased;
    el.textContent = decimals ? val.toFixed(decimals) : Math.round(val).toLocaleString();
    if (p < 1) requestAnimationFrame(tick);
  }
  requestAnimationFrame(tick);
}

document.addEventListener('DOMContentLoaded', () => {
  const counterObs = new IntersectionObserver((entries) => {
    entries.forEach((en) => { if (en.isIntersecting) { animateCounter(en.target); counterObs.unobserve(en.target); } });
  }, { threshold: 0.3 });
  document.querySelectorAll('.counter[data-target]').forEach((el) => counterObs.observe(el));

  const revealObs = new IntersectionObserver((entries) => {
    entries.forEach((en, i) => {
      if (en.isIntersecting) { setTimeout(() => en.target.classList.add('in'), i * 120); revealObs.unobserve(en.target); }
    });
  }, { threshold: 0.15 });
  document.querySelectorAll('.reveal').forEach((el) => revealObs.observe(el));

  // 热力图渲染（7×14 网格，随机透明度 accent）
  const g = document.getElementById('heatmap-cells');
  if (g) {
    const cols = 14, rows = 7, cell = 18, gap = 2;
    for (let r = 0; r < rows; r++) for (let c = 0; c < cols; c++) {
      const v = Math.random();
      const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
      rect.setAttribute('x', c * (cell + gap));
      rect.setAttribute('y', r * (cell + gap));
      rect.setAttribute('width', cell);
      rect.setAttribute('height', cell);
      rect.setAttribute('rx', 3);
      rect.setAttribute('fill', 'var(--accent)');
      rect.setAttribute('opacity', v * 0.9 + 0.05);
      g.appendChild(rect);
    }
  }
});
