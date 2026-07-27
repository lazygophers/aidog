import { describe, it, expect } from "vitest";
import { applySelectedPaths } from "./applySelectedPaths";

describe("applySelectedPaths", () => {
  it("写入选中的顶层 path，未选中的兄弟 key 保留原值", () => {
    const config = { a: 1, b: 2 };
    const source = { a: 100, b: 200 };
    const next = applySelectedPaths(config, source, new Set(["a"]));
    expect(next).toEqual({ a: 100, b: 2 });
  });

  it("嵌套 path 只覆盖选中叶子，同级未选中键不受影响", () => {
    const config = { permissions: { allow: ["x"], deny: ["y"] } };
    const source = { permissions: { allow: ["new"], deny: ["y"] } };
    const next = applySelectedPaths(config, source, new Set(["permissions.allow"]));
    expect(next).toEqual({ permissions: { allow: ["new"], deny: ["y"] } });
  });

  it("path 在 source 中不存在（removed diff）→ 从结果删除该 key", () => {
    const config = { hooks: { PreToolUse: "x" }, keep: 1 };
    const source = { keep: 1 };
    const next = applySelectedPaths(config, source, new Set(["hooks.PreToolUse"]));
    expect(next).toEqual({ hooks: {}, keep: 1 });
  });

  it("path 中间层 config 缺失时按需创建中间对象", () => {
    const config = {};
    const source = { a: { b: { c: 1 } } };
    const next = applySelectedPaths(config, source, new Set(["a.b.c"]));
    expect(next).toEqual({ a: { b: { c: 1 } } });
  });

  it("不修改传入的 config（返回新对象）", () => {
    const config = { a: 1 };
    const source = { a: 2 };
    const next = applySelectedPaths(config, source, new Set(["a"]));
    expect(config).toEqual({ a: 1 });
    expect(next).not.toBe(config);
  });
});
