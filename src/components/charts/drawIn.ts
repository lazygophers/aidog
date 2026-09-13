// ── 路径生长动画（spec §B2：stroke-dasharray draw-in）──
// 两条路：Recharts 系列用 drawInProps（recharts 内置 draw-in 动画）；
// 自研 SVG path（热力图连线 / 内联曲线）用 useDrawInPath（实测路径长度做 dash 生长）。
import { useEffect, type RefObject } from "react";

/**
 * Recharts Line/Area/Bar 的生长动画 props 展开进系列组件即可：
 * `<Line {...drawInProps()} dataKey="cost" />`。
 */
export function drawInProps(durationMs = 700) {
  return {
    isAnimationActive: true,
    animationBegin: 0,
    animationDuration: durationMs,
  } as const;
}

/**
 * 自定义 SVG path 的 stroke-dasharray 生长动画：测 getTotalLength，
 * dashoffset 从全长过渡到 0。元素未挂载 / 无 getTotalLength（测试环境）/ 零长路径 → 跳过。
 */
export function useDrawInPath(ref: RefObject<SVGPathElement | null>, durationMs = 700) {
  useEffect(() => {
    const path = ref.current;
    if (!path || typeof path.getTotalLength !== "function") return;
    const len = path.getTotalLength();
    if (!(len > 0)) return;
    path.style.transition = "none";
    path.style.strokeDasharray = `${len}`;
    path.style.strokeDashoffset = `${len}`;
    // 先落初始 dashoffset 再下一帧开过渡，否则浏览器合并两笔 style 不播动画。
    const raf = requestAnimationFrame(() => {
      path.style.transition = `stroke-dashoffset ${durationMs}ms ease-out`;
      path.style.strokeDashoffset = "0";
    });
    return () => cancelAnimationFrame(raf);
  }, [ref, durationMs]);
}
