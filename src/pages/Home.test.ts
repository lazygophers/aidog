// Home 命令面板（#36）纯函数单测：sparkline / 趋势线共用归一化。
import { describe, it, expect } from "vitest";
import { normPoints } from "./Home";

describe("normPoints", () => {
  it("空序列 → 空点集", () => {
    expect(normPoints([], 100, 22)).toEqual([]);
  });

  it("单点 → 居中", () => {
    expect(normPoints([5], 100, 22)).toEqual([[50, 11]]);
  });

  it("min-max 归一化：最大值顶 pad、最小值落 h-pad，x 均分覆盖全宽", () => {
    const pts = normPoints([0, 5, 10], 100, 22, 2);
    expect(pts[0]).toEqual([0, 20]);   // 最小值 → 底部
    expect(pts[1]).toEqual([50, 11]);  // 中值 → 中线
    expect(pts[2]).toEqual([100, 2]);  // 最大值 → 顶部
  });

  it("平坦序列 → 全部落中线（不除零）", () => {
    expect(normPoints([7, 7, 7], 90, 20, 2)).toEqual([[0, 10], [45, 10], [90, 10]]);
  });

  it("负值 / 小数序列同规则", () => {
    const pts = normPoints([-2, 2], 40, 10, 1);
    expect(pts[0][1]).toBeCloseTo(9);
    expect(pts[1][1]).toBeCloseTo(1);
  });
});
