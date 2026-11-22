import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (k: string, d?: string, o?: Record<string, unknown>) => (d ?? k).replace(/\{\{(\w+)\}\}/g, (_, x) => String(o?.[x])),
  }),
}));
const sessionDetail = vi.fn();
vi.mock("../../services/api/clientUsage", () => ({ clientUsageApi: { sessionDetail: (...a: unknown[]) => sessionDetail(...a) } }));

import { formatClockTime } from "../../utils/formatters";
import { SessionSparkline, curvePath, smoothSeries, windowBuckets } from "./SessionSparkline";

const pt = (t: number, n = 10) => ({ callTimeMs: t, inputTokens: n, outputTokens: 0, cacheReadTokens: 0, cacheWriteTokens: 0 });
// NOW 所在秒恰是 5 秒整：NOW 落在末桶（第 59 桶）起点后 0.5 秒
const NOW = 1_800_000_000_500;
// 窗口第 b 个 5 秒桶的起点毫秒戳（末桶 = NOW 所在桶）
const bucketMs = (b: number) => (Math.floor(NOW / 5000) - 59 + b) * 5000;

describe("windowBuckets（最近 5 分钟，5 秒一桶）", () => {
  it("60 桶按绝对 5 秒对齐：同桶不同秒求和，首桶计入，窗口外丢弃", () => {
    const b = windowBuckets([pt(NOW - 301_000, 7), pt(NOW - 295_000, 3), pt(NOW - 1000, 5), pt(NOW - 3000, 4)], NOW);
    expect(b).toHaveLength(60);
    expect(b[0]).toBe(3);
    expect(b[58]).toBe(9);
    expect(b.reduce((a, v) => a + v, 0)).toBe(12);
  });
});

describe("smoothSeries / curvePath", () => {
  const spike = Array.from({ length: 60 }, (_, i) => (i === 30 ? 100 : 0));

  it("平滑后非负、总量守恒、尖峰被摊开（σ=1 桶、半径 3 桶）", () => {
    const s = smoothSeries(spike);
    expect(s).toHaveLength(60);
    expect(Math.min(...s)).toBeGreaterThanOrEqual(0);
    expect(s.reduce((a, v) => a + v, 0)).toBeCloseTo(100, 6);
    expect(Math.max(...s)).toBeLessThan(50);
    expect(s[27]).toBeGreaterThan(0);
    expect(s[26]).toBe(0);
  });

  it("单调插值：尖峰输入的曲线与控制点 y 全落在 [0, H]，不低于基线、不过冲", () => {
    const d = curvePath(spike, 100, 24);
    const ys = [...d.matchAll(/(-?[\d.]+),(-?[\d.]+)/g)].map((m) => Number(m[2]));
    expect(d.startsWith("M")).toBe(true);
    expect(d).toContain("C");
    expect(ys.length).toBeGreaterThan(60);
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

  const rect = (width: number) => ({ left: 100, top: 0, width, height: 24, right: 100 + width, bottom: 24, x: 100, y: 0, toJSON: () => ({}) });
  const mount = async (points: ReturnType<typeof pt>[], width: number) => {
    sessionDetail.mockResolvedValue({ points, modelChain: [], agents: [] });
    render(<SessionSparkline clientCode="codex" sessionId="s" lastCallMs={NOW - 1000} />);
    await act(async () => {});
    const spark = screen.getByTestId("sessions-row-spark");
    vi.spyOn(spark, "getBoundingClientRect").mockReturnValue(rect(width));
    return spark;
  };

  it("首桶的调用在跨入下一个 5 秒桶后滑出窗口；卸载后计时器清零", async () => {
    sessionDetail.mockResolvedValue({ points: [pt(NOW - 295_000, 3), pt(NOW - 1000, 5)], modelChain: [], agents: [] });
    const { unmount } = render(<SessionSparkline clientCode="codex" sessionId="s" lastCallMs={NOW - 1000} />);
    await act(async () => {});
    const spark = screen.getByTestId("sessions-row-spark");
    expect(spark).toHaveAttribute("data-bars", "60");
    expect(spark).toHaveAttribute("data-total", "8");
    await act(async () => { vi.advanceTimersByTime(4000); });
    expect(spark).toHaveAttribute("data-total", "8");
    await act(async () => { vi.advanceTimersByTime(1000); });
    expect(spark).toHaveAttribute("data-total", "5");
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("画单条平滑曲线；宽屏悬浮显示所在 5 秒桶：同桶不同秒合并为 总和÷5，区间恰 5 秒、起点 5 秒整，移出隐藏，点击不冒泡", async () => {
    sessionDetail.mockResolvedValue({ points: [pt(NOW - 1000, 600), pt(NOW - 4000, 400), pt(NOW - 60_000, 5)], modelChain: [], agents: [] });
    const rowClick = vi.fn();
    render(<div onClick={rowClick}><SessionSparkline clientCode="codex" sessionId="s" lastCallMs={NOW - 1000} /></div>);
    await act(async () => {});
    const spark = screen.getByTestId("sessions-row-spark");
    const curve = screen.getByTestId("sessions-spark-curve");
    expect(curve.getAttribute("d")).toContain("C");
    expect(spark.querySelectorAll("path[data-testid='sessions-spark-curve']")).toHaveLength(1);
    expect(curve.getAttribute("d")).not.toMatch(/V/);

    // 140px 容 60 桶（一像素约 2.1 秒）：第 135 像素横跨第 57/58 桶边界，只取像素中心所在的第 58 桶
    vi.spyOn(spark, "getBoundingClientRect").mockReturnValue(rect(140));
    fireEvent.mouseMove(spark, { clientX: 235.3, clientY: 10 });
    const tip = screen.getByTestId("sessions-spark-tip");
    expect(tip).toHaveAttribute("data-span", "58-59");
    expect(tip).toHaveAttribute("data-rate", "200");
    const start = Number(tip.getAttribute("data-start"));
    const end = Number(tip.getAttribute("data-end"));
    expect(start).toBe(bucketMs(58));
    expect(start % 5000).toBe(0);
    expect(end - start).toBe(5000);
    expect(tip.textContent).toContain(`${formatClockTime(bucketMs(58))}–${formatClockTime(bucketMs(59))}`);
    expect(tip.textContent).toContain("200 tokens/s");
    expect(tip.parentElement).toBe(document.body);
    expect(screen.getByTestId("sessions-spark-guide")).toBeInTheDocument();

    // 末像素 = 末桶（当前所在 5 秒），无调用
    fireEvent.mouseMove(spark, { clientX: 239.9, clientY: 10 });
    expect(screen.getByTestId("sessions-spark-tip")).toHaveAttribute("data-start", String(bucketMs(59)));
    expect(screen.getByTestId("sessions-spark-tip").textContent).toContain("0 tokens/s");

    fireEvent.click(spark);
    expect(rowClick).not.toHaveBeenCalled();

    fireEvent.mouseLeave(spark);
    expect(screen.queryByTestId("sessions-spark-tip")).toBeNull();
    expect(screen.queryByTestId("sessions-spark-guide")).toBeNull();
  });

  it("宽屏 600px：任意像素都是单桶 5 秒区间", async () => {
    const spark = await mount([pt(NOW - 6000, 50)], 600);
    for (const x of [100.2, 679.6, 680.4, 699.9]) {
      fireEvent.mouseMove(spark, { clientX: x, clientY: 10 });
      const tip = screen.getByTestId("sessions-spark-tip");
      const start = Number(tip.getAttribute("data-start"));
      expect(Number(tip.getAttribute("data-end")) - start).toBe(5000);
      expect(start % 5000).toBe(0);
    }
    fireEvent.mouseMove(spark, { clientX: 675, clientY: 10 });
    expect(screen.getByTestId("sessions-spark-tip")).toHaveAttribute("data-rate", "10");
  });

  it("窄宽 49px：像素覆盖多个桶 → 区间向外对齐到桶边界，显示多桶平均", async () => {
    // 第 47 像素覆盖桶 [57.55, 58.78) → 向外对齐 [57, 59)；第 58 桶 1000 tokens → 200/s，两桶平均 100/s
    const spark = await mount([pt(NOW - 4000, 1000)], 49);
    fireEvent.mouseMove(spark, { clientX: 147.1, clientY: 10 });
    const tip = screen.getByTestId("sessions-spark-tip");
    expect(tip).toHaveAttribute("data-span", "57-59");
    expect(tip).toHaveAttribute("data-rate", "100");
    expect(tip).toHaveAttribute("data-start", String(bucketMs(57)));
    expect(tip).toHaveAttribute("data-end", String(bucketMs(59)));
    expect(tip.textContent).toContain(`${formatClockTime(bucketMs(57))}–${formatClockTime(bucketMs(59))}`);
  });
});

describe("formatClockTime", () => {
  it("本地时钟 24 小时制 HH:MM:SS，补零", () => {
    expect(formatClockTime(new Date(2026, 10, 22, 14, 3, 21, 999).getTime())).toBe("14:03:21");
    expect(formatClockTime(new Date(2026, 10, 22, 0, 0, 5).getTime())).toBe("00:00:05");
  });
});
