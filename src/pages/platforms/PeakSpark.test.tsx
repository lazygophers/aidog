// PeakSpark 24h 条：命中时段点亮（含跨天首尾两段、全天、空窗三态）。
import { describe, it, expect } from "vitest";
import { render } from "../../test/render";
import { PeakSpark } from "./formSections";
import type { TimeWindow } from "../domains/platforms/defaults";

const litCount = (c: HTMLElement) =>
  Array.from(c.querySelectorAll("i")).filter(
    (i) => (i as HTMLElement).style.background.includes("color-warning")
  ).length;

function spark(w: Partial<TimeWindow>) {
  const { container } = render(
    <PeakSpark w={{ start_hour: 0, end_hour: 24, multiplier: 1, ...w } as TimeWindow} />
  );
  return container;
}

describe("PeakSpark — 24 段点亮逻辑", () => {
  it("普通窗口 9-12 → 3 段亮", () => {
    expect(litCount(spark({ start_hour: 9, end_hour: 12 }))).toBe(3);
  });
  it("跨天窗口 22-6 → 首尾两段共 8 段亮", () => {
    expect(litCount(spark({ start_hour: 22, end_hour: 6 }))).toBe(8);
  });
  it("全天 0-24 → 24 段全亮", () => {
    expect(litCount(spark({ start_hour: 0, end_hour: 24 }))).toBe(24);
  });
  it("空窗 start===end → 0 段亮", () => {
    expect(litCount(spark({ start_hour: 5, end_hour: 5 }))).toBe(0);
  });
});
