// @vitest-environment node
import { describe, it, expect } from "vitest";
import type { TFunction } from "i18next";
import { parseTestBody } from "./TestResultBody";

// stub TFunction：忽略 key，返回 fallback（第二参数）。
const t = ((_key: string, fallback?: string) => fallback ?? _key) as unknown as TFunction;

describe("parseTestBody", () => {
  it("空字符串 → raw 空", () => {
    expect(parseTestBody("", t)).toEqual({ kind: "raw", text: "" });
    expect(parseTestBody("   ", t)).toEqual({ kind: "raw", text: "" });
  });

  it("非 JSON → raw 原文", () => {
    const r = parseTestBody("HTTP 500 Internal Error", t);
    expect(r).toEqual({ kind: "raw", text: "HTTP 500 Internal Error" });
  });

  it("非对象 JSON（数字/数组）→ raw 原文", () => {
    expect(parseTestBody("123", t).kind).toBe("raw");
    expect(parseTestBody("[1,2]", t).kind).toBe("raw");
  });

  it("error 对象 → 拆 message/type/code", () => {
    const body = JSON.stringify({ error: { message: "bad key", type: "auth_error", code: "401" } });
    const r = parseTestBody(body, t);
    expect(r.kind).toBe("known");
    if (r.kind === "known") {
      expect(r.rows).toEqual([
        { label: "错误信息", value: "bad key" },
        { label: "错误类型", value: "auth_error" },
        { label: "错误码", value: "401" },
      ]);
    }
  });

  it("error 字符串 → 单行错误", () => {
    const r = parseTestBody(JSON.stringify({ error: "rate limited" }), t);
    expect(r.kind).toBe("known");
    if (r.kind === "known") {
      expect(r.rows).toContainEqual({ label: "错误", value: "rate limited" });
    }
  });

  it("usage（anthropic 风格）→ input/output tokens", () => {
    const r = parseTestBody(JSON.stringify({ usage: { input_tokens: 10, output_tokens: 5 } }), t);
    expect(r.kind).toBe("known");
    if (r.kind === "known") {
      expect(r.rows).toContainEqual({ label: "输入 tokens", value: "10" });
      expect(r.rows).toContainEqual({ label: "输出 tokens", value: "5" });
    }
  });

  it("usage（openai 风格）→ prompt/completion tokens 归一", () => {
    const r = parseTestBody(JSON.stringify({ usage: { prompt_tokens: 7, completion_tokens: 3 } }), t);
    if (r.kind === "known") {
      expect(r.rows).toContainEqual({ label: "输入 tokens", value: "7" });
      expect(r.rows).toContainEqual({ label: "输出 tokens", value: "3" });
    }
  });

  it("anthropic content → 响应内容", () => {
    const body = JSON.stringify({ content: [{ type: "text", text: "hello" }] });
    const r = parseTestBody(body, t);
    if (r.kind === "known") {
      expect(r.rows).toContainEqual({ label: "响应内容", value: "hello" });
    }
  });

  it("openai choices → 响应内容", () => {
    const body = JSON.stringify({ choices: [{ message: { content: "world" } }] });
    const r = parseTestBody(body, t);
    if (r.kind === "known") {
      expect(r.rows).toContainEqual({ label: "响应内容", value: "world" });
    }
  });

  it("已知但无可识别字段（空对象）→ raw 回退", () => {
    const r = parseTestBody(JSON.stringify({ foo: 1 }), t);
    expect(r.kind).toBe("raw");
  });

  it("error 对象存在但无 message/type/code → 整体序列化兜底（known）", () => {
    const body = JSON.stringify({ error: { detail: "x" } });
    const r = parseTestBody(body, t);
    expect(r.kind).toBe("known");
  });
});
