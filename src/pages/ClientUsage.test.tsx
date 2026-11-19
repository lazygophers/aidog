// ClientUsage 页就近测试：渲染真组件，mock api，断言走 data-testid / data-state。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";

const api = vi.hoisted(() => ({
  consentGet: vi.fn(),
  consentSet: vi.fn(),
  status: vi.fn(),
  summary: vi.fn(),
  trend: vi.fn(),
  byModel: vi.fn(),
  sessions: vi.fn(),
  refresh: vi.fn(),
}));
vi.mock("../services/api/clientUsage", () => ({ clientUsageApi: api }));

import { ClientUsage } from "./ClientUsage";

const DAY = 86400000;
const NOW = Date.now();

const consent = (asked: boolean, enabled: Record<string, boolean> = {}) => ({
  asked,
  enabled,
});

const statusPayload = (asked: boolean, enabled: Record<string, boolean> = {}) => ({
  consent: consent(asked, enabled),
  import: {
    running: false, clientCode: "", startedAtMs: 0, filesTotal: 0, filesDone: 0,
    callsImported: 0, parseErrors: 0, finishedAtMs: 0, error: "",
  },
  persistedParseErrors: 0,
});

const dataPayload = (calls: number) => ({
  summary: {
    calls,
    unpricedCalls: calls > 0 ? 1 : 0,
    inputTokens: calls * 100,
    outputTokens: calls * 20,
    cacheReadTokens: 0,
    cacheWriteTokens: 0,
    reasoningTokens: 0,
    cost: calls > 0 ? 1.5 : 0,
  },
  trend: calls > 0
    ? [{ dayMs: NOW - DAY, calls, inputTokens: 100, outputTokens: 20, cost: 1.5, unpricedCalls: 0 }]
    : [],
  byModel: calls > 0
    ? [
        { model: "claude-opus-4-6", calls, inputTokens: 100, outputTokens: 20, cost: 1.5, unpricedCalls: 0 },
        { model: "unknown-model-x", calls: 2, inputTokens: 10, outputTokens: 5, cost: 0, unpricedCalls: 2 },
      ]
    : [],
  sessions: calls > 0
    ? [
        { clientCode: "claude_code", sessionId: "s1", title: "演示会话", workdir: "/tmp/demo", firstCallMs: NOW - DAY, lastCallMs: NOW, calls, cost: 1.5, inputTokens: 100, outputTokens: 20 },
        { clientCode: "codex", sessionId: "s2", title: "未定价会话", workdir: "", firstCallMs: NOW - DAY, lastCallMs: NOW - 3600_000, calls: 2, cost: 0, inputTokens: 10, outputTokens: 5 },
      ]
    : [],
});

beforeEach(() => {
  vi.clearAllMocks();
  api.consentSet.mockResolvedValue(undefined);
  api.refresh.mockResolvedValue({ running: false, clientCode: "", startedAtMs: 0, filesTotal: 0, filesDone: 0, callsImported: 0, parseErrors: 0, finishedAtMs: 0, error: "" });
});

describe("ClientUsage", () => {
  it("首次进入（asked=false）显示同意卡，四个开关默认关", async () => {
    api.status.mockResolvedValue(statusPayload(false));
    render(<ClientUsage />);
    await waitFor(() => expect(screen.getByTestId("consent-card")).toBeTruthy());
    for (const code of ["claude_code", "codex", "gemini_cli", "opencode"]) {
      const sw = screen.getByTestId(`consent-${code}`) as HTMLElement & { dataset: Record<string, string> };
      expect(sw).toBeTruthy();
      expect(sw.getAttribute("data-state")).toBe("unchecked");
    }
    expect(api.consentSet).not.toHaveBeenCalled();
  });

  it("主视图客户端开关切换即调 consent_set", async () => {
    api.status.mockResolvedValue(statusPayload(true, { claude_code: true }));
    const d = dataPayload(5);
    api.summary.mockResolvedValue(d.summary);
    api.trend.mockResolvedValue(d.trend);
    api.byModel.mockResolvedValue(d.byModel);
    api.sessions.mockResolvedValue(d.sessions);
    render(<ClientUsage />);
    await waitFor(() => expect(screen.getByTestId("client-switches")).toBeTruthy());
    const sw = screen.getByTestId("client-switch-codex");
    fireEvent.click(sw);
    await waitFor(() => {
      expect(api.consentSet).toHaveBeenCalledWith(
        expect.objectContaining({ asked: true, enabled: expect.objectContaining({ codex: true, claude_code: true }) }),
      );
    });
    // 关掉 claude_code → consent_set 带 enabled.claude_code=false。
    fireEvent.click(screen.getByTestId("client-switch-claude_code"));
    await waitFor(() => {
      expect(api.consentSet).toHaveBeenLastCalledWith(
        expect.objectContaining({ enabled: expect.objectContaining({ claude_code: false }) }),
      );
    });
  });

  it("查不到官方价的花费列显示未定价标记", async () => {
    api.status.mockResolvedValue(statusPayload(true, { claude_code: true }));
    const d = dataPayload(5);
    api.summary.mockResolvedValue(d.summary);
    api.trend.mockResolvedValue(d.trend);
    api.byModel.mockResolvedValue(d.byModel);
    api.sessions.mockResolvedValue(d.sessions);
    render(<ClientUsage />);
    await waitFor(() => expect(screen.getByTestId("sessions-card")).toBeTruthy());
    // 会话表：cost=0 的会话显示未定价。
    expect(screen.getByTestId("session-unpriced")).toBeTruthy();
    // 按模型表：cost=0 的模型显示未定价 + 计数。
    const mark = screen.getByTestId("unpriced-mark");
    expect(mark.textContent).toContain("2");
  });

  it("所选时间范围无数据时显示空态", async () => {
    api.status.mockResolvedValue(statusPayload(true, { claude_code: true }));
    const d = dataPayload(0);
    api.summary.mockResolvedValue(d.summary);
    api.trend.mockResolvedValue(d.trend);
    api.byModel.mockResolvedValue(d.byModel);
    api.sessions.mockResolvedValue(d.sessions);
    render(<ClientUsage />);
    // 等「数据已加载」标记：避免 waitFor 命中 summary=null 的首帧瞬时空态。
    await waitFor(() => expect(screen.getByTestId("data-loaded")).toBeTruthy());
    expect(screen.getByTestId("summary-empty")).toBeTruthy();
    expect(screen.queryByTestId("sessions-card")).toBeNull();
    expect(screen.queryByTestId("by-model-card")).toBeNull();
  });
});
