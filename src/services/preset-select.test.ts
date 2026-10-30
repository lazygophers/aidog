import { describe, expect, it } from "vitest";
import { planPresetLoad } from "./preset-select";

const p = (id: string) => ({ id, name: id, config: { id } });

describe("planPresetLoad", () => {
  it("0 份：不弹、无默认", () => {
    expect(planPresetLoad([])).toEqual({ needPicker: false, defaultPreset: undefined });
  });
  it("1 份：不弹，默认即该份", () => {
    const r = planPresetLoad([p("oss")]);
    expect(r.needPicker).toBe(false);
    expect(r.defaultPreset?.id).toBe("oss");
  });
  it("2 份：弹，默认取 oss 而非第一份", () => {
    const r = planPresetLoad([p("x"), p("oss")]);
    expect(r.needPicker).toBe(true);
    expect(r.defaultPreset?.id).toBe("oss");
  });
  it("3 份无 oss：弹，回落第一份", () => {
    const r = planPresetLoad([p("a"), p("b"), p("c")]);
    expect(r.needPicker).toBe(true);
    expect(r.defaultPreset?.id).toBe("a");
  });
});
