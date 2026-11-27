// 分档订阅曲线渲染测试：真组件 + data-testid 断言（i18n 回退返回 key）。
import { describe, it, expect } from "vitest";
import { render, screen } from "../../../test/render";
import { SubscriptionCurveView, type TierCardData } from "./SubscriptionCurve";

const HOUR = 3_600_000;
const NOW = 1_000 * HOUR;

function card(overrides: Partial<TierCardData> = {}): TierCardData {
  return {
    name: "weekly_limit",
    unit: "tokens",
    cycleMs: 7 * 24 * HOUR,
    resetAt: NOW + 3 * HOUR,
    curve: null,
    snapshotPct: 30,
    ...overrides,
  };
}

describe("SubscriptionCurveView — 分档小卡", () => {
  it("每档一张卡：档名 testid 在场，徽标显示周期", () => {
    const cards = [
      card({ name: "five_hour", cycleMs: 5 * HOUR }),
      card({ name: "weekly_limit", cycleMs: 7 * 24 * HOUR }),
    ];
    render(<SubscriptionCurveView cards={cards} />);
    expect(screen.getByTestId("detail-t3-curve")).not.toBeNull();
    expect(screen.getByTestId("detail-t3-curve-card-five_hour")).not.toBeNull();
    expect(screen.getByTestId("detail-t3-curve-card-weekly_limit")).not.toBeNull();
    expect(screen.getByTestId("detail-t3-cycle-badge-five_hour").textContent).toBe("5h");
    expect(screen.getByTestId("detail-t3-cycle-badge-weekly_limit").textContent).toBe("7d");
  });

  it("有真实轨迹：实线路径 + 虚线投影 + 重置线在场", () => {
    const ws = NOW - 6 * HOUR;
    render(<SubscriptionCurveView cards={[card({
      curve: {
        windowStart: ws, resetAt: NOW + 3 * HOUR,
        actual: [{ t: NOW - 5 * HOUR, pct: 90 }, { t: NOW - 2 * HOUR, pct: 60 }, { t: NOW - HOUR, pct: 50 }],
        snapshotPct: 50,
        forecast: { lasts: true, exhaustAt: null },
      },
    })]} />);
    expect(screen.getByTestId("detail-t3-fit-weekly_limit")).not.toBeNull();
    expect(screen.getByTestId("detail-t3-forecast-weekly_limit")).not.toBeNull();
    expect(screen.getByTestId("detail-t3-reset-line-weekly_limit")).not.toBeNull();
  });

  it("无历史：显示快照 + 积累说明，不画轨迹", () => {
    render(<SubscriptionCurveView cards={[card({ curve: null, snapshotPct: 30 })]} />);
    expect(screen.getByTestId("detail-t3-snapshot-weekly_limit").textContent)
      .toContain("platform.detail.curveAccumulating");
    expect(screen.queryByTestId("detail-t3-fit-weekly_limit")).toBeNull();
    expect(screen.queryByTestId("detail-t3-forecast-weekly_limit")).toBeNull();
  });

  it("提前耗尽：投影终点取 exhaustAt", () => {
    const ws = NOW - 6 * HOUR;
    render(<SubscriptionCurveView cards={[card({
      curve: {
        windowStart: ws, resetAt: NOW + 10 * HOUR,
        actual: [{ t: NOW - 5 * HOUR, pct: 100 }, { t: NOW - 3 * HOUR, pct: 80 }],
        snapshotPct: 80,
        forecast: { lasts: false, exhaustAt: NOW + 5 * HOUR },
      },
    })]} />);
    expect(screen.getByTestId("detail-t3-forecast-weekly_limit")).not.toBeNull();
    expect(screen.getByTestId("detail-t3-curve-card-weekly_limit").textContent)
      .toContain("platform.detail.curveExhausts");
  });

  it("计划消耗参考线：窗口起点 100% 到重置点 0% 的直线在场，坐标取窗口两端", () => {
    const ws = NOW - 6 * HOUR;
    const { unmount } = render(<SubscriptionCurveView cards={[card({
      curve: {
        windowStart: ws, resetAt: NOW + 3 * HOUR,
        actual: [{ t: NOW - 5 * HOUR, pct: 90 }, { t: NOW - HOUR, pct: 50 }],
        snapshotPct: 50,
        forecast: { lasts: true, exhaustAt: null },
      },
    })]} />);
    const plan = screen.getByTestId(`detail-t3-plan-weekly_limit`);
    expect(plan.getAttribute("x1")).toBe("8");
    expect(plan.getAttribute("y1")).toBe("8");
    expect(plan.getAttribute("x2")).toBe("552");
    expect(plan.getAttribute("y2")).toBe("112");
    unmount();
    // 无历史（无窗口）：不画参考线
    render(<SubscriptionCurveView cards={[card({ curve: null })]} />);
    expect(screen.queryByTestId(`detail-t3-plan-weekly_limit`)).toBeNull();
  });
});
