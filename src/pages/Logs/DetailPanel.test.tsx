// 模型选择记录就近测试：重试行展示当次模型，旧日志（缺 model 字段）不渲染该元素。
// 锚 data-testid，禁文案断言（同 RoutingCanvas.test.tsx 口径）。
import { describe, it, expect, vi } from "vitest";

vi.mock("../../services/platform", () => ({
  writeText: vi.fn(async () => {}),
}));
vi.mock("../../domains/platforms/useProtocolMeta", () => ({
  useProtocolLabel: () => "anthropic",
}));

import { render, screen } from "../../test/render";
import { DetailPanel } from "./DetailPanel";
import type { ProxyLogDetail } from "../../services/api";

const attempt = (id: number, status: number, model?: string) => ({
  platform_id: id, platform_name: `p${id}`, status_code: status, error: "",
  duration_ms: 10, ts: 0, ...(model !== undefined ? { model } : {}),
});

function detailFixture(attempts: unknown[]) {
  return {
    id: "log-1",
    group_key: "g1",
    model: "m-src",
    actual_model: "m-src",
    source_protocol: "anthropic",
    target_protocol: "openai",
    status_code: 200,
    duration_ms: 120,
    input_tokens: 1,
    output_tokens: 2,
    cache_tokens: 0,
    created_at: "2026-10-29T00:00:00Z",
    request_url: "",
    request_headers: "{}",
    request_body: "{}",
    user_response_headers: "{}",
    user_response_body: "",
    response_body: "ok",
    upstream_request_url: "",
    upstream_status_code: 200,
    upstream_request_headers: "{}",
    upstream_request_body: "{}",
    upstream_response_headers: "{}",
    attempts,
    routing: null,
  } as unknown as ProxyLogDetail;
}

function renderPanel(detail: ProxyLogDetail | null) {
  return render(
    <DetailPanel
      d={{
        detail,
        t: ((key: string, fallback?: string) => fallback ?? key) as never,
        copied: false,
        copiedId: false,
        setCopiedId: () => {},
        openDetail: () => {},
        copyDetail: () => {},
        platformMap: new Map(),
        groupName: (k) => k,
        setDetail: () => {},
      }}
    />,
  );
}

describe("DetailPanel attempts model", () => {
  it("新日志每行展示当次模型", () => {
    renderPanel(detailFixture([attempt(1, 502, "m-a"), attempt(2, 200, "m-b")]));
    const tags = screen.getAllByTestId("attempt-model");
    expect(tags).toHaveLength(2);
    expect(tags[0]!.textContent).toBe("m-a");
    expect(tags[1]!.textContent).toBe("m-b");
  });

  it("旧日志缺 model 字段：不渲染模型元素，不占位", () => {
    renderPanel(detailFixture([attempt(1, 200)]));
    expect(screen.queryByTestId("attempt-model")).toBeNull();
  });

  it("detail 为空不渲染", () => {
    renderPanel(null);
    expect(screen.queryByTestId("attempt-model")).toBeNull();
  });
});
