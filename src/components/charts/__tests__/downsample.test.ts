// @vitest-environment node
import { describe, it, expect } from "vitest";
import { downsampleLTTB, LTTB_THRESHOLD } from "../downsample";

const xOf = (p: { x: number; y: number }) => p.x;
const yOf = (p: { x: number; y: number }) => p.y;
const xy = (x: number, y: number) => ({ x, y });

describe("downsampleLTTB", () => {
  it("keeps the 500-point threshold constant (spec §F1)", () => {
    expect(LTTB_THRESHOLD).toBe(500);
  });

  it("input at or below threshold returns the same reference untouched", () => {
    const pts = [xy(0, 1), xy(1, 2), xy(2, 3)];
    expect(downsampleLTTB(pts, xOf, yOf, 5)).toBe(pts);
    expect(downsampleLTTB(pts, xOf, yOf, 3)).toBe(pts);
  });

  it("empty input → empty output", () => {
    expect(downsampleLTTB([], xOf, yOf)).toEqual([]);
  });

  it("threshold < 3 cannot bucket → input returned as-is", () => {
    const pts = Array.from({ length: 100 }, (_, i) => xy(i, i));
    expect(downsampleLTTB(pts, xOf, yOf, 2)).toBe(pts);
  });

  it("output has exactly threshold points, first/last preserved, x ascending", () => {
    const pts = Array.from({ length: 1000 }, (_, i) => xy(i, Math.sin(i / 50) + 1));
    const out = downsampleLTTB(pts, xOf, yOf, 100);
    expect(out).toHaveLength(100);
    expect(out[0]).toBe(pts[0]);
    expect(out[out.length - 1]).toBe(pts[999]);
    const xs = out.map(xOf);
    for (let i = 1; i < xs.length; i++) expect(xs[i]).toBeGreaterThan(xs[i - 1]);
  });

  it("keeps the dominant spike inside its bucket (visual fidelity)", () => {
    const pts = Array.from({ length: 1000 }, (_, i) => xy(i, i < 500 ? 0 : 1));
    // index 499 是阶跃尖峰，LTTB 应选中而非丢失
    const out = downsampleLTTB(pts, xOf, yOf, 100);
    expect(out).toContain(pts[499]);
  });

  it("bucket averaging hits the last-bucket clamp (rangeEnd capped at n)", () => {
    // n=999, threshold=100：every 非整除，末段 bucket rangeEnd 被 clamp 到 n
    const pts = Array.from({ length: 999 }, (_, i) => xy(i, (i % 7) * 0.1));
    const out = downsampleLTTB(pts, xOf, yOf, 100);
    expect(out).toHaveLength(100);
    expect(out[99]).toBe(pts[998]);
  });
});
