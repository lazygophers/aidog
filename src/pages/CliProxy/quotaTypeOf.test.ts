import { describe, it, expect } from "vitest";
import { quotaTypeOf } from "./quotaTypeOf";

describe("quotaTypeOf", () => {
  it("解析出 newapi", () => {
    expect(quotaTypeOf(JSON.stringify({ type: "newapi" }))).toBe("newapi");
  });
  it("undefined 回落 none", () => {
    expect(quotaTypeOf(undefined)).toBe("none");
  });
  it("空字符串回落 none", () => {
    expect(quotaTypeOf("")).toBe("none");
  });
  it("非法 JSON 回落 none（不抛异常）", () => {
    expect(quotaTypeOf("{not json")).toBe("none");
  });
  it("JSON 内无 type 字段回落 none", () => {
    expect(quotaTypeOf(JSON.stringify({ foo: "bar" }))).toBe("none");
  });
  it("type 为空字符串（假值）也回落 none", () => {
    expect(quotaTypeOf(JSON.stringify({ type: "" }))).toBe("none");
  });
});
