// @vitest-environment node
import { describe, it, expect } from "vitest";
import { deepMerge } from "./deepMerge";

describe("deepMerge", () => {
  it("override wins for scalar keys", () => {
    expect(deepMerge({ a: 1, b: 2 }, { b: 3 })).toEqual({ a: 1, b: 3 });
  });
  it("preserves base-only keys", () => {
    expect(deepMerge({ a: 1 }, { b: 2 })).toEqual({ a: 1, b: 2 });
  });
  it("merges nested plain objects recursively", () => {
    const out = deepMerge(
      { env: { A: "1", B: "2" }, x: 1 },
      { env: { B: "3", C: "4" } },
    );
    expect(out).toEqual({ env: { A: "1", B: "3", C: "4" }, x: 1 });
  });
  it("replaces arrays instead of unioning them", () => {
    expect(deepMerge({ list: [1, 2, 3] }, { list: [9] })).toEqual({ list: [9] });
  });
  it("overrides object with scalar (mismatched types do not merge)", () => {
    expect(deepMerge({ a: { x: 1 } }, { a: 5 })).toEqual({ a: 5 });
  });
  it("overrides scalar with object", () => {
    expect(deepMerge({ a: 5 } as Record<string, unknown>, { a: { x: 1 } })).toEqual({
      a: { x: 1 },
    });
  });
  it("treats null / array / Date as non-plain objects (no recursion)", () => {
    const d = new Date();
    expect(deepMerge({ a: { x: 1 } }, { a: null })).toEqual({ a: null });
    expect(deepMerge({ a: { x: 1 } }, { a: d })).toEqual({ a: d });
    expect(deepMerge({ a: null } as Record<string, unknown>, { a: { y: 2 } })).toEqual({
      a: { y: 2 },
    });
  });
});
