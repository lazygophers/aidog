import { describe, expect, it } from "vitest";
import { planPresetLoad, renderPresetPlaceholders } from "./preset-select";

describe("renderPresetPlaceholders", () => {
  const cfg = { a: { base_url: "http://127.0.0.1:{{port}}/proxy" }, list: ["{{port}}", 1], n: 2 };
  it("替换嵌套字符串里的 {{port}}，换端口生效", () => {
    expect(renderPresetPlaceholders(cfg, { port: 19890 })).toEqual({
      a: { base_url: "http://127.0.0.1:19890/proxy" }, list: ["19890", 1], n: 2,
    });
  });
  it("无占位符原样返回同一引用", () => {
    const plain = { x: "y", n: [1] };
    expect(renderPresetPlaceholders(plain, { port: 1 })).toBe(plain);
  });
  it("未提供的变量保留原文", () => {
    expect(renderPresetPlaceholders({ s: "{{other}}" }, { port: 1 })).toEqual({ s: "{{other}}" });
  });
});

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
