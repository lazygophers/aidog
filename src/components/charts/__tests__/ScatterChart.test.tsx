import { describe, it, expect } from "vitest";
import { render, screen } from "../../../test/render";
import { ScatterChart, scatterPoints } from "../ScatterChart";
import type { ScatterHistogram } from "../../../services/api/types/generated/ScatterHistogram";

// 3×3 网格（duration 0-3000ms step1000，cost 0-0.06 step0.02；counts 行=duration bin 列=cost bin）
const hist: ScatterHistogram = {
  duration_bins: [0, 1000, 2000, 3000],
  cost_bins: [0, 0.02, 0.04, 0.06],
  counts: [
    [10, 0, 5],
    [0, 3, 0],
    [0, 0, 1],
  ],
};

describe("scatterPoints", () => {
  it("非零格 → bin 中心点，零格剔除", () => {
    expect(scatterPoints(hist)).toEqual([
      { x: 500, y: 0.01, count: 10 },
      { x: 500, y: 0.05, count: 5 },
      { x: 1500, y: 0.03, count: 3 },
      { x: 2500, y: 0.05, count: 1 },
    ]);
  });
  it("全空矩阵（空窗口）→ 空数组不抛", () => {
    expect(scatterPoints({ duration_bins: [], cost_bins: [], counts: [] })).toEqual([]);
    expect(scatterPoints({ duration_bins: [0, 1000], cost_bins: [0, 1], counts: [[0]] })).toEqual([]);
  });
});

describe("ScatterChart", () => {
  it("renders scatter symbols sized by count with axis tick formatting", () => {
    const { container } = render(<ScatterChart histogram={hist} />);
    expect(container.querySelector("svg")).toBeTruthy();
    // 4 个非零 bin → 4 个散点符号
    expect(container.querySelectorAll(".recharts-scatter-symbol").length).toBe(4);
    // 轴刻度走 formatters：X=延迟（ms/s），Y=成本（$）
    const ticks = Array.from(container.querySelectorAll(".recharts-cartesian-axis-tick-value")).map(
      (el) => el.textContent,
    );
    expect(ticks.some((tx) => tx != null && /\d+(\.\d+)? (ms|s|min)/.test(tx))).toBe(true);
    expect(ticks.some((tx) => tx != null && tx.includes("$"))).toBe(true);
    // 轴标签（x=延迟 y=成本；测试 i18n 回传 key）
    expect(screen.getByText("charts.scatterX")).toBeTruthy();
    expect(screen.getByText("charts.scatterY")).toBeTruthy();
  });

  it("renders honest empty state for empty matrix", () => {
    render(<ScatterChart histogram={{ duration_bins: [], cost_bins: [], counts: [] }} />);
    expect(screen.getByText("charts.noData")).toBeTruthy();
  });
});
