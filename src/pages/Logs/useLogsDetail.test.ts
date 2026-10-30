import { describe, expect, it } from "vitest";
import { buildLogMarkdown } from "./useLogsDetail";
import type { ProxyLogDetail } from "../../services/api";

const base = {
  id: "log-1",
  group_key: "g1",
  model: "claude-sonnet-5",
  actual_model: "claude-sonnet-5",
  source_protocol: "anthropic",
  target_protocol: "openai",
  status_code: 200,
  duration_ms: 120,
  input_tokens: 10,
  output_tokens: 20,
  cache_tokens: 0,
  created_at: "2026-10-24T00:00:00Z",
  request_url: "http://x/v1",
  request_headers: "{}",
  request_body: "{}",
  user_response_headers: "{}",
  user_response_body: "",
  response_body: "",
  upstream_request_url: "",
  upstream_status_code: 200,
  upstream_request_headers: "{}",
  upstream_request_body: "",
  upstream_response_headers: "{}",
  attempts: [],
  routing: null,
} as unknown as ProxyLogDetail;

const attempt = (name: string, status: number, error = "", model?: string) => ({
  platform_id: 1, platform_name: name, status_code: status, error, duration_ms: 50, ts: 0, model,
});

describe("buildLogMarkdown", () => {
  it("含路由决策与 attempts 两段并导出两层目标模型", () => {
    const md = buildLogMarkdown({
      ...base,
      attempts: [attempt("platA", 500, "upstream error", "mapped-a"), attempt("platB", 200, "", "mapped-b")],
      routing: {
        mode: "failover",
        entries: [
          { order: 1, platform: "platB", platform_id: 2, level_priority: 5, priority: 0, reason: "selected", tags: ["mapping-promoted"], target_model: "mapped-b" },
          { order: 2, platform: "platC", platform_id: 3, level_priority: 3, priority: 2, reason: "breaker", tags: [] },
        ],
      },
    } as unknown as ProxyLogDetail);
    expect(md).toContain("## Routing Decision");
    expect(md).toContain("- Mode: failover");
    expect(md).toContain("1. platB — 选中（映射提升） · 优先级 L5/P0 · mapped-b");
    expect(md).toContain("2. platC — 熔断 · 优先级 L3/P2");
    expect(md).toContain("## Attempts");
    expect(md).toContain("1. platA — status 500 — 50 ms — mapped-a — upstream error");
    expect(md).toContain("2. platB — status 200 — 50 ms — mapped-b");
  });

  it("旧日志缺模型字段时不输出占位符", () => {
    const md = buildLogMarkdown({ ...base, attempts: [attempt("platA", 200)] } as unknown as ProxyLogDetail);
    expect(md).not.toContain("undefined");
    expect(md).not.toContain("null");
    expect(md).toContain("## Attempts");
  });

  it("只有 attempts 无快照（旧日志）：无 Routing Decision 段", () => {
    const md = buildLogMarkdown({ ...base, attempts: [attempt("platA", 200)] } as unknown as ProxyLogDetail);
    expect(md).not.toContain("Routing Decision");
    expect(md).toContain("## Attempts");
  });

  it("两者皆空：两段都省略", () => {
    const md = buildLogMarkdown(base);
    expect(md).not.toContain("Routing Decision");
    expect(md).not.toContain("## Attempts");
    expect(md).toContain("# Proxy Log log-1");
  });
});
