// ── 路径生长动画（spec §B2：stroke-dasharray draw-in）──
// 两条路：Recharts 系列用 useDrawIn（recharts 内置 draw-in 动画）；
// 自研 SVG path（热力图连线 / 内联曲线）用 useDrawInPath（实测路径长度做 dash 生长）。
import { useEffect, useRef, type RefObject } from "react";

/**
 * Recharts Line/Area/Bar 的生长动画 props 展开进系列组件即可：
 * `<Line {...useDrawIn()} dataKey="cost" />`。
 *
 * **只在首次挂载时动画，之后的数据更新不再重播**。recharts 的生长动画由 react-smooth
 * 驱动，每一帧都是一次 React 提交：一轮 700 ms 动画 ≈ 42 次提交。恒开时每次数据刷新
 * （代理跑完一个请求就会刷）都要把整张图从零重画一遍，实测 Stats 页一次刷新 57 次提交里
 * 有 42 次出自这里（票 11 病灶 A 的真正根因——不是缺 React.memo，memo 治不了挂载期提交）。
 * 首屏那一次生长动画是设计要的，保留；重播是纯消耗，去掉。
 */
export function useDrawIn(durationMs = 700) {
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
  }, []);
  return {
    isAnimationActive: !mounted.current,
    animationBegin: 0,
    animationDuration: durationMs,
  };
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
