// @vitest-environment node
import { describe, it, expect } from "vitest";
import { stableStringify } from "./stableStringify";

// 用作配置表单的 dirty 签名：键顺序变化不能算改动，值变化必须算改动。
describe("stableStringify", () => {
  it("键重排产生相同串", () => {
    expect(stableStringify({ b: 1, a: 2 })).toBe(stableStringify({ a: 2, b: 1 }));
  });

  it("嵌套对象递归排序", () => {
    expect(stableStringify({ x: { z: 1, y: 2 } })).toBe(stableStringify({ x: { y: 2, z: 1 } }));
  });

  it("数组保持顺序（顺序是语义的一部分）", () => {
    expect(stableStringify([1, 2])).not.toBe(stableStringify([2, 1]));
    expect(stableStringify([{ b: 1, a: 2 }])).toBe(stableStringify([{ a: 2, b: 1 }]));
  });

  it("值变化产生不同串", () => {
    expect(stableStringify({ a: 1 })).not.toBe(stableStringify({ a: 2 }));
  });

  it("基本类型走 JSON.stringify 语义", () => {
    expect(stableStringify(null)).toBe("null");
    expect(stableStringify(1)).toBe("1");
    expect(stableStringify("s")).toBe('"s"');
    expect(stableStringify(true)).toBe("true");
    expect(stableStringify(undefined)).toBeUndefined();
  });

  it("键名本身被 JSON 转义（含引号/中文的键不串味）", () => {
    expect(stableStringify({ 'a"b': 1 })).toBe('{"a\\"b":1}');
    expect(stableStringify({ 中文: 1 })).toBe('{"中文":1}');
  });
});
