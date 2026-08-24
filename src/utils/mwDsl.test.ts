// mwDsl round-trip 与错误定位测试（票 05）。

import { describe, it, expect } from "vitest";
import { treeToDsl, parseDsl, dslErrorPos } from "./mwDsl";
import type { ConditionNode } from "../services/api";

const leaf = (target: string, pattern: string, match_type = "contains", field = ""): ConditionNode => ({
  kind: "leaf",
  target,
  field,
  match_type,
  pattern,
} as ConditionNode);

describe("mwDsl round-trip", () => {
  it("单叶子往返一致", () => {
    const t = leaf("request_body", "foo");
    expect(parseDsl(treeToDsl(t))).toEqual(t);
  });

  it("嵌套 ALL/ANY + regex/exact + field 往返一致", () => {
    const t: ConditionNode = {
      kind: "all",
      children: [
        leaf("request_body", "sk-\\w+", "regex"),
        {
          kind: "any",
          children: [
            leaf("status", "4", "regex"),
            leaf("response_headers", "x", "exact", "retry-after"),
          ],
        },
      ],
    };
    const t2 = parseDsl(treeToDsl(t));
    expect(t2).toEqual(t);
  });

  it("pattern 含引号 / 换行 / 反斜杠往返一致", () => {
    const t = leaf("request_body", String.fromCharCode(97,34,98,92,99,10,100));
    expect(parseDsl(treeToDsl(t))).toEqual(t);
  });

  it("单子组在 DSL 中折叠为叶子（语义等价）", () => {
    const one: ConditionNode = { kind: "any", children: [leaf("model", "m", "exact")] };
    // treeToDsl 折叠单子组为叶子；往返结果与其唯一子节点等价。
    expect(parseDsl(treeToDsl(one))).toEqual(one.children[0]);
  });
});

describe("mwDsl 错误定位", () => {
  it("未知 target 报位置", () => {
    try {
      parseDsl('foo contains "x"');
      expect.unreachable();
    } catch (e) {
      expect(String(e)).toContain("未知 target");
      expect(dslErrorPos(e)).toBe(0);
    }
  });

  it("未闭合字符串报位置", () => {
    try {
      parseDsl('request_body contains "x');
      expect.unreachable();
    } catch (e) {
      expect(String(e)).toContain("未闭合");
      expect(dslErrorPos(e)).toBe(22);
    }
  });

  it("缺 pattern / 缺右括号 / 空 ALL() 均拒绝", () => {
    expect(() => parseDsl("request_body contains")).toThrow();
    expect(() => parseDsl("ALL(request_body contains \"x\"")).toThrow();
    expect(() => parseDsl("ALL()")).toThrow();
  });

  it("条件后多余内容拒绝", () => {
    expect(() => parseDsl('request_body contains "x" )')).toThrow("多余内容");
  });
});
