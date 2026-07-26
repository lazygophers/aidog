// ── motion hooks · 萤火虫动效（对齐 example/js/motion.js）──
// useReveal: IntersectionObserver 触发一次淡入上移（stagger 可选）
// useCounter: raf 缓动数字滚动（cubic ease-out，进入视口触发一次）
// ponytail: 纯 React hook + stdlib，无新依赖。SSR-safe（Tauri webview 恒 client）。

import { useEffect, useRef, useState } from "react";

/** 进入视口一次后置 in。staggerMs > 0 时延迟激活（逐项错峰）。 */
export function useReveal<T extends HTMLElement = HTMLDivElement>(staggerMs = 0) {
  const ref = useRef<T | null>(null);
  const [shown, setShown] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (!el || shown) return;
    if (typeof IntersectionObserver === "undefined") {
      setShown(true);
      return;
    }
    const obs = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          if (staggerMs > 0) {
            window.setTimeout(() => setShown(true), staggerMs);
          } else {
            setShown(true);
          }
          obs.disconnect();
        }
      },
      { threshold: 0.12 },
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [shown, staggerMs]);
  return { ref, shown };
}

/**
 * 数字滚动：target 目标值，decimals 小数位，durMs 时长。
 * 返回 [ref, display]；ref 绑到容器，进入视口后启动一次滚动到 target。
 * ponytail: requestAnimationFrame + cubic ease-out，stdlib only。
 */
export function useCounter(target: number, decimals = 0, durMs = 1200) {
  const ref = useRef<HTMLSpanElement | null>(null);
  const [display, setDisplay] = useState(decimals ? target.toFixed(decimals) : String(target));
  const startedRef = useRef(false);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const run = () => {
      if (startedRef.current) return;
      startedRef.current = true;
      const start = performance.now();
      const tick = (now: number) => {
        const p = Math.min((now - start) / durMs, 1);
        const eased = 1 - Math.pow(1 - p, 3);
        const val = target * eased;
        setDisplay(decimals ? val.toFixed(decimals) : Math.round(val).toLocaleString());
        if (p < 1) requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    };
    if (typeof IntersectionObserver === "undefined") {
      run();
      return;
    }
    const obs = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          run();
          obs.disconnect();
        }
      },
      { threshold: 0.3 },
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [target, decimals, durMs]);

  return { ref, display };
}

/** Ripple 涟漪 onClick handler：绑到按钮/卡片，点击生成扩散波。 */
export function makeRipple(e: React.MouseEvent<HTMLElement>) {
  const btn = e.currentTarget;
  if (btn.querySelector(".ripple-wave")) return;
  const r = btn.getBoundingClientRect();
  const wave = document.createElement("span");
  wave.className = "ripple-wave";
  const size = Math.max(r.width, r.height);
  wave.style.width = wave.style.height = `${size}px`;
  wave.style.left = `${e.clientX - r.left - size / 2}px`;
  wave.style.top = `${e.clientY - r.top - size / 2}px`;
  btn.appendChild(wave);
  window.setTimeout(() => wave.remove(), 600);
}
