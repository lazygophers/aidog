import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (k: string, d?: string, o?: Record<string, unknown>) => (d ?? k).replace(/\{\{(\w+)\}\}/g, (_, x) => String(o?.[x])),
  }),
}));
const sessionDetail = vi.fn();
vi.mock("../../services/api/clientUsage", () => ({ clientUsageApi: { sessionDetail: (...a: unknown[]) => sessionDetail(...a) } }));

import { SessionSparkline, curvePath, secondBuckets, smoothSeries } from "./SessionSparkline";

const pt = (t: number, n = 10) => ({ callTimeMs: t, inputTokens: n, outputTokens: 0, cacheReadTokens: 0, cacheWriteTokens: 0 });
const NOW = 1_800_000_000_500;

describe("secondBuckets（最近 5 分钟逐秒）", () => {
  it("now-1s / now-299s 计入对应格，now-301s 排除，同秒求和", () => {
    const b = secondBuckets([pt(NOW - 301_000, 7), pt(NOW - 299_000, 3), pt(NOW - 1000, 5), pt(NOW - 1000 + 200, 4)], NOW);
    expect(b).toHaveLength(300);
    expect(b[0]).toBe(3);
    expect(b[298]).toBe(9);
    expect(b.reduce((a, v) => a + v, 0)).toBe(12);
  });
});

describe("smoothSeries / curvePath", () => {
  const spike = Array.from({ length: 300 }, (_, i) => (i === 150 ? 100 : 0));

  it("平滑后非负、总量守恒、尖峰被摊开", () => {
    const s = smoothSeries(spike);
    expect(s).toHaveLength(300);
    expect(Math.min(...s)).toBeGreaterThanOrEqual(0);
    expect(s.reduce((a, v) => a + v, 0)).toBeCloseTo(100, 6);
    expect(Math.max(...s)).toBeLessThan(50);
    expect(s[147]).toBeGreaterThan(0);
  });

  it("单调插值：尖峰输入的曲线与控制点 y 全落在 [0, H]，不低于基线、不过冲", () => {
    const d = curvePath(spike, 100, 24);
    const ys = [...d.matchAll(/(-?[\d.]+),(-?[\d.]+)/g)].map((m) => Number(m[2]));
    expect(d.startsWith("M")).toBe(true);
    expect(d).toContain("C");
    expect(ys.length).toBeGreaterThan(300);
    expect(Math.max(...ys)).toBeLessThanOrEqual(24);
    expect(Math.min(...ys)).toBeGreaterThanOrEqual(0);
  });
});

describe("SessionSparkline 实时滑动", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });
  afterEach(() => {
    vi.useRealTimers();
    sessionDetail.mockReset();
  });

  it("now-299s 的调用推进 2 秒后滑出窗口；卸载后计时器清零", async () => {
    sessionDetail.mockResolvedValue({ points: [pt(NOW - 299_000, 3), pt(NOW - 1000, 5)], modelChain: [], agents: [] });
    const { unmount } = render(<SessionSparkline clientCode="codex" sessionId="s" lastCallMs={NOW - 1000} />);
    await act(async () => {});
    const spark = screen.getByTestId("sessions-row-spark");
    expect(spark).toHaveAttribute("data-bars", "300");
    expect(spark).toHaveAttribute("data-total", "8");
    await act(async () => { vi.advanceTimersByTime(2000); });
    expect(spark).toHaveAttribute("data-total", "5");
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("画单条平滑曲线（不是逐秒竖线）；悬浮映射到所在秒并显示原始速度，移出隐藏，点击不冒泡到行", async () => {
    sessionDetail.mockResolvedValue({ points: [pt(NOW - 9000, 1234), pt(NOW - 60_000, 5)], modelChain: [], agents: [] });
    const rowClick = vi.fn();
    render(<div onClick={rowClick}><SessionSparkline clientCode="codex" sessionId="s" lastCallMs={NOW - 9000} /></div>);
    await act(async () => {});
    const spark = screen.getByTestId("sessions-row-spark");
    const curve = screen.getByTestId("sessions-spark-curve");
    expect(curve.getAttribute("d")).toContain("C");
    expect(spark.querySelectorAll("path[data-testid='sessions-spark-curve']")).toHaveLength(1);
    expect(curve.getAttribute("d")).not.toMatch(/V/);

    vi.spyOn(spark, "getBoundingClientRect").mockReturnValue({ left: 100, top: 0, width: 150, height: 24, right: 250, bottom: 24, x: 100, y: 0, toJSON: () => ({}) });
    // 150px 容 300 秒：x=245.25 落在第 145 像素，覆盖秒 [290, 292)；时间偏移取区间最新一秒（第 291 秒 = 8 秒前）
    fireEvent.mouseMove(spark, { clientX: 245.25, clientY: 10 });
    const tip = screen.getByTestId("sessions-spark-tip");
    expect(tip).toHaveAttribute("data-second", "290");
    expect(tip).toHaveAttribute("data-span", "290-292");
    expect(tip).toHaveAttribute("data-rate", "617");
    expect(tip.textContent).toContain("8 秒前");
    expect(tip.textContent).toContain("617 tokens/s");
    expect(tip.parentElement).toBe(document.body);
    expect(screen.getByTestId("sessions-spark-guide")).toBeInTheDocument();

    fireEvent.mouseMove(spark, { clientX: 249.9, clientY: 10 });
    expect(screen.getByTestId("sessions-spark-tip").textContent).toContain("刚刚");
    expect(screen.getByTestId("sessions-spark-tip").textContent).toContain("0 tokens/s");

    fireEvent.click(spark);
    expect(rowClick).not.toHaveBeenCalled();

    fireEvent.mouseLeave(spark);
    expect(screen.queryByTestId("sessions-spark-tip")).toBeNull();
    expect(screen.queryByTestId("sessions-spark-guide")).toBeNull();
  });

  const rect = (width: number) => ({ left: 100, top: 0, width, height: 24, right: 100 + width, bottom: 24, x: 100, y: 0, toJSON: () => ({}) });

  it("窄宽 49px：光标所在单秒为 0、像素覆盖区间内有非零秒 → 显示区间平均速度", async () => {
    sessionDetail.mockResolvedValue({ points: [pt(NOW - 9000, 600)], modelChain: [], agents: [] });
    render(<SessionSparkline clientCode="codex" sessionId="s" lastCallMs={NOW - 9000} />);
    await act(async () => {});
    const spark = screen.getByTestId("sessions-row-spark");
    vi.spyOn(spark, "getBoundingClientRect").mockReturnValue(rect(49));
    // 第 47 像素覆盖秒 [287, 293)，调用在第 290 秒；x=147.1 单秒映射为第 288 秒（值 0）
    fireEvent.mouseMove(spark, { clientX: 147.1, clientY: 10 });
    const tip = screen.getByTestId("sessions-spark-tip");
    expect(tip).toHaveAttribute("data-rate", "100");
    expect(tip).toHaveAttribute("data-span", "287-293");
  });

  it("宽度足够（≤1 秒/像素）退化为单秒原始值", async () => {
    sessionDetail.mockResolvedValue({ points: [pt(NOW - 9000, 600)], modelChain: [], agents: [] });
    render(<SessionSparkline clientCode="codex" sessionId="s" lastCallMs={NOW - 9000} />);
    await act(async () => {});
    const spark = screen.getByTestId("sessions-row-spark");
    vi.spyOn(spark, "getBoundingClientRect").mockReturnValue(rect(600));
    fireEvent.mouseMove(spark, { clientX: 100 + 290 * 2 + 0.5, clientY: 10 });
    const tip = screen.getByTestId("sessions-spark-tip");
    expect(tip).toHaveAttribute("data-rate", "600");
    expect(tip).toHaveAttribute("data-span", "290-291");
  });
});
