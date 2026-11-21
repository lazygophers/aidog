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
        { clientCode: "claude_code", sessionId: "s1", title: "演示会话", workdir: "/tmp/demo", firstCallMs: NOW - DAY, lastCallMs: NOW, calls, cost: 1.5, unpricedCalls: 0, inputTokens: 100, outputTokens: 20 },
        { clientCode: "codex", sessionId: "s2", title: "未定价会话", workdir: "", firstCallMs: NOW - DAY, lastCallMs: NOW - 3600_000, calls: 2, cost: 0, unpricedCalls: 2, inputTokens: 10, outputTokens: 5 },
        { clientCode: "codex", sessionId: "s3", title: "免费模型会话", workdir: "", firstCallMs: NOW - DAY, lastCallMs: NOW - 7200_000, calls: 1, cost: 0, unpricedCalls: 0, inputTokens: 10, outputTokens: 5 },
      ]
    : [],
});

beforeEach(() => {
  vi.clearAllMocks();
  api.consentSet.mockResolvedValue(undefined);
  api.refresh.mockResolvedValue({ running: false, clientCode: "", startedAtMs: 0, filesTotal: 0, filesDone: 0, callsImported: 0, parseErrors: 0, finishedAtMs: 0, error: "" });
});

describe("ClientUsage", () => {
  it("四家都未启用时只显示引导卡，按钮跳设置子页", async () => {
    api.status.mockResolvedValue(statusPayload(true, { claude_code: false }));
    const onNavigate = vi.fn();
    render(<ClientUsage onNavigate={onNavigate} />);
    await waitFor(() => expect(screen.getByTestId("client-usage-guide")).toBeTruthy());
    expect(screen.queryByTestId("summary-card")).toBeNull();
    expect(screen.queryByTestId("client-switches")).toBeNull();
    expect(screen.queryByTestId("consent-card")).toBeNull();
    expect(api.summary).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("client-usage-guide-go"));
    expect(onNavigate).toHaveBeenCalledWith("settings/client_usage");
  });

  it("部分启用时顶部一行提示 + 跳转链接，页面无开关", async () => {
    api.status.mockResolvedValue(statusPayload(true, { claude_code: true }));
    const d = dataPayload(5);
    api.summary.mockResolvedValue(d.summary);
    api.trend.mockResolvedValue(d.trend);
    api.byModel.mockResolvedValue(d.byModel);
    api.sessions.mockResolvedValue(d.sessions);
    const onNavigate = vi.fn();
    render(<ClientUsage onNavigate={onNavigate} />);
    await waitFor(() => expect(screen.getByTestId("client-usage-partial-hint")).toBeTruthy());
    expect(screen.getByTestId("client-usage-partial-hint").getAttribute("data-disabled")).toBe("codex,gemini_cli,opencode");
    expect(screen.queryByTestId("client-usage-guide")).toBeNull();
    expect(screen.queryByTestId("client-switch-codex")).toBeNull();
    fireEvent.click(screen.getByTestId("client-usage-partial-go"));
    expect(onNavigate).toHaveBeenCalledWith("settings/client_usage");
  });

  it("四家全启用时不显示部分启用提示", async () => {
    api.status.mockResolvedValue(statusPayload(true, { claude_code: true, codex: true, gemini_cli: true, opencode: true }));
    const d = dataPayload(5);
    api.summary.mockResolvedValue(d.summary);
    api.trend.mockResolvedValue(d.trend);
    api.byModel.mockResolvedValue(d.byModel);
    api.sessions.mockResolvedValue(d.sessions);
    render(<ClientUsage />);
    await waitFor(() => expect(screen.getByTestId("data-loaded")).toBeTruthy());
    expect(screen.queryByTestId("client-usage-partial-hint")).toBeNull();
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
    // 会话表：s2 全未定价（unpricedCalls==calls）→ 唯一未定价标记；s3 免费模型（cost=0 已定价）→ 不带标记。
    const marks = screen.getAllByTestId("session-unpriced");
    expect(marks).toHaveLength(1);
    // 按模型表：cost=0 的模型显示未定价 + 计数。
    const mark = screen.getByTestId("unpriced-mark");
    expect(mark.textContent).toContain("2");
  });

  it("混合会话显示金额与未定价计数", async () => {
    api.status.mockResolvedValue(statusPayload(true, { claude_code: true }));
    const d = dataPayload(5);
    api.summary.mockResolvedValue(d.summary);
    api.trend.mockResolvedValue(d.trend);
    api.byModel.mockResolvedValue(d.byModel);
    // s1 改成混合：2 次调用 1 次未定价 → 金额 + ×1 小标。
    api.sessions.mockResolvedValue([
      { ...d.sessions[0], calls: 2, cost: 1.5, unpricedCalls: 1 },
    ]);
    render(<ClientUsage />);
    await waitFor(() => expect(screen.getByTestId("sessions-card")).toBeTruthy());
    const mixed = screen.getByTestId("session-mixed-cost");
    expect(mixed.textContent).toContain("1.50");
    expect(mixed.textContent).toContain("×1");
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
