import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (k: string) => k }) }));
const sessionDetail = vi.fn();
vi.mock("../../services/api/clientUsage", () => ({ clientUsageApi: { sessionDetail: (...a: unknown[]) => sessionDetail(...a) } }));

import { SessionSparkline, secondBuckets } from "./SessionSparkline";

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
});
