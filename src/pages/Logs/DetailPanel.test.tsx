// 模型选择记录就近测试：重试行展示当次模型，旧日志（缺 model 字段）不渲染该元素。
// 锚 data-testid，禁文案断言（同 RoutingCanvas.test.tsx 口径）。
import { describe, it, expect, vi } from "vitest";

vi.mock("../../services/platform", () => ({
  writeText: vi.fn(async () => {}),
}));
vi.mock("../../domains/platforms/useProtocolMeta", () => ({
  useProtocolLabel: () => "anthropic",
}));

import { fireEvent } from "@testing-library/react";
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

function renderPanel(detail: ProxyLogDetail | null, setDetail: (d: null) => void = () => {}, onNavigate?: (id: string) => void) {
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
        setDetail,
        onNavigate,
      }}
    />,
  );
}

describe("DetailPanel attempt rounds", () => {
  it("routing.round_starts 把尝试按轮分组：每行带所属轮号", () => {
    const d = { ...detailFixture([attempt(1, 529), attempt(2, 529), attempt(1, 200)]), routing: { entries: [], round_starts: [2] } } as unknown as ProxyLogDetail;
    renderPanel(d);
    const rounds = screen.getAllByTestId("attempt-round").map((el) => el.getAttribute("data-round"));
    expect(rounds).toEqual(["1", "1", "2"]);
  });

  it("旧日志（无 round_starts）不渲染轮号", () => {
    renderPanel(detailFixture([attempt(1, 502), attempt(2, 200)]));
    expect(screen.queryByTestId("attempt-round")).toBeNull();
  });
});

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

describe("DetailPanel UA", () => {
  it("展示原始 UA 与客户端代码", () => {
    const d = { ...detailFixture([attempt(1, 200)]), user_agent: "claude-cli/2.0.1 (external, cli)", client: "claude_code" } as unknown as ProxyLogDetail;
    renderPanel(d);
    expect(screen.getByTestId("detail-user-agent").textContent).toBe("claude-cli/2.0.1 (external, cli)");
    expect(screen.getByTestId("detail-client").textContent).toBe("claude_code");
  });

  it("旧日志无 UA：占位 -", () => {
    renderPanel(detailFixture([attempt(1, 200)]));
    expect(screen.getByTestId("detail-user-agent").textContent).toBe("-");
  });
});

describe("DetailPanel Claude Code 网关提示头", () => {
  it("已知请求类型映射为本地化标签，子代理类型原样展示", () => {
    const d = { ...detailFixture([attempt(1, 200)]), request_class: "subagent", agent_type: "Explore" } as unknown as ProxyLogDetail;
    renderPanel(d);
    expect(screen.getByTestId("detail-request-class").textContent).toBe("子代理");
    expect(screen.getByTestId("detail-agent-type").textContent).toBe("Explore");
  });

  it("未知请求类型原样展示", () => {
    const d = { ...detailFixture([attempt(1, 200)]), request_class: "brand_new", agent_type: "" } as unknown as ProxyLogDetail;
    renderPanel(d);
    expect(screen.getByTestId("detail-request-class").textContent).toBe("brand_new");
  });

  it("旧日志无该头：占位 -", () => {
    renderPanel(detailFixture([attempt(1, 200)]));
    expect(screen.getByTestId("detail-request-class").textContent).toBe("-");
    expect(screen.getByTestId("detail-agent-type").textContent).toBe("-");
  });
});

describe("DetailPanel Claude Code 网关 id / 压缩 / 工具耗时", () => {
  it("七个字段展示：压缩原因本地化，未知原样，工具耗时展开成列表", () => {
    const d = {
      ...detailFixture([attempt(1, 200)]),
      prompt_id: "p-1", session_id: "s-1", agent_id: "ag-1", parent_agent_id: "ag-0",
      compaction: "manual", context_compacted: "weird", prev_tool_durations: "Bash=742;Read=9",
    } as unknown as ProxyLogDetail;
    renderPanel(d);
    expect(screen.getByTestId("detail-prompt-id").textContent).toBe("p-1");
    expect(screen.getByTestId("detail-session-id").textContent).toBe("s-1");
    expect(screen.getByTestId("detail-agent-id").textContent).toBe("ag-1");
    expect(screen.getByTestId("detail-parent-agent-id").textContent).toBe("ag-0");
    expect(screen.getByTestId("detail-compaction").textContent).toBe("手动");
    expect(screen.getByTestId("detail-context-compacted").textContent).toBe("weird");
    const rows = screen.getAllByTestId("detail-tool-duration");
    expect(rows.map(r => r.textContent)).toEqual(["Bash742 ms", "Read9 ms"]);
  });

  it("旧日志无该头：占位 -，无工具耗时列表", () => {
    renderPanel(detailFixture([attempt(1, 200)]));
    for (const id of ["prompt-id", "session-id", "agent-id", "parent-agent-id", "compaction", "context-compacted"]) {
      expect(screen.getByTestId(`detail-${id}`).textContent).toBe("-");
    }
    expect(screen.queryAllByTestId("detail-tool-duration")).toEqual([]);
  });
});

describe("DetailPanel 独立页", () => {
  it("渲染为整页（非弹窗），返回按钮把 detail 置空", () => {
    const setDetail = vi.fn();
    renderPanel(detailFixture([attempt(1, 200)]), setDetail);
    expect(screen.getByTestId("detail-page")).not.toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.click(screen.getByTestId("detail-back"));
    expect(setDetail).toHaveBeenCalledWith(null);
  });
});

describe("DetailPanel 隧道行", () => {
  const tunnel = () => ({
    ...detailFixture([]),
    source_protocol: "http-connect",
    target_protocol: "http-connect",
    blocked_reason: "mitm_opaque",
    request_url: "api.anthropic.com:443",
  }) as unknown as ProxyLogDetail;

  it("未解密隧道：显示说明，链接跳 settings/mitm", () => {
    const nav = vi.fn();
    renderPanel(tunnel(), () => {}, nav);
    expect(screen.getByTestId("detail-tunnel-notice")).not.toBeNull();
    fireEvent.click(screen.getByTestId("detail-tunnel-mitm-link"));
    expect(nav).toHaveBeenCalledWith("settings/mitm");
  });

  it("普通请求无隧道说明", () => {
    renderPanel(detailFixture([attempt(1, 200)]));
    expect(screen.queryByTestId("detail-tunnel-notice")).toBeNull();
  });
});
