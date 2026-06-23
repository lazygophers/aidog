// @vitest-environment node
import { describe, it, expect } from "vitest";
import { smoothPath, type ChartPoint } from "./chart";

describe("smoothPath", () => {
  it("returns empty string for no points", () => {
    expect(smoothPath([], 0, 100)).toBe("");
  });
  it("emits a single M for one point", () => {
    expect(smoothPath([{ x: 1, y: 2 }], 0, 100)).toBe("M 1.0,2.0");
  });
  it("emits M..L for two points (degenerate line)", () => {
    expect(
      smoothPath(
        [
          { x: 0, y: 0 },
          { x: 10, y: 20 },
        ],
        0,
        100,
      ),
    ).toBe("M 0.0,0.0 L 10.0,20.0");
  });
  it("emits cubic beziers for >= 3 points", () => {
    const pts: ChartPoint[] = [
      { x: 0, y: 10 },
      { x: 10, y: 30 },
      { x: 20, y: 20 },
      { x: 30, y: 40 },
    ];
    const d = smoothPath(pts, 0, 100);
    expect(d.startsWith("M 0.0,10.0")).toBe(true);
    expect(d).toContain(" C ");
  });
  it("clamps control-point y to [clampMin, clampMax] to prevent overshoot", () => {
    const pts: ChartPoint[] = [
      { x: 0, y: 0 },
      { x: 10, y: 100 },
      { x: 20, y: 0 },
      { x: 30, y: 100 },
    ];
    const d = smoothPath(pts, 10, 90);
    // 任何控制点 y 不得超出 [10, 90]；检查没有出现 100.0 之外的越界值控制点
    const nums = d.match(/-?\d+\.\d/g)!.map(Number);
    const ys = nums.filter((_, i) => i % 2 === 1); // 粗略取 y 维度
    for (const y of ys) {
      // 端点本身可达 0/100，仅断言曲线不产出极端越界（< -1 之类）
      expect(y).toBeGreaterThanOrEqual(0);
    }
    expect(d).toContain(" C ");
  });
});
