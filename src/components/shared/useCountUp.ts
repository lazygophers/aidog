// useCountUp — G3 票 04 / 旧票 05 动效：统计数字 count-up（600ms，ease-out）。
// prefers-reduced-motion 或值不可数 → 直接返回目标值（降级不动画）。
import { useEffect, useRef, useState } from "react";

const reduced = () =>
  typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;

export function useCountUp(target: number, durationMs = 600): number {
  const [value, setValue] = useState(target);
  const fromRef = useRef(target);
  useEffect(() => {
    if (reduced() || !Number.isFinite(target)) {
      fromRef.current = target;
      setValue(target);
      return;
    }
    const from = fromRef.current;
    if (from === target) return;
    const t0 = performance.now();
    let raf = 0;
    const tick = (now: number) => {
      const p = Math.min(1, (now - t0) / durationMs);
      const eased = 1 - Math.pow(1 - p, 3); // ease-out cubic
      setValue(from + (target - from) * eased);
      if (p < 1) raf = requestAnimationFrame(tick);
      else fromRef.current = target;
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [target, durationMs]);
  return value;
}
