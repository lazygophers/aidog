// Sessions 页就近测试（扩展-会话页 票 02）：渲染真组件，mock api / transport，断言走 data-testid / data-state。
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";
import { act, fireEvent, render, screen, waitFor, within } from "../test/render";

const api = vi.hoisted(() => ({
  status: vi.fn(),
  sessions: vi.fn(),
  refresh: vi.fn(),
  sessionDetail: vi.fn(),
  sessionStates: vi.fn(),
  sessionResume: vi.fn(),
  sessionHide: vi.fn(),
  sessionUnhide: vi.fn(),
  hiddenSessions: vi.fn(),
  sessionCarry: vi.fn(),
  installedClients: vi.fn(),
  transcriptConsentGet: vi.fn(),
}));
vi.mock("../services/api/clientUsage", () => ({ clientUsageApi: api }));

const cctx = vi.hoisted(() => ({ contextModeSessions: vi.fn() }));
vi.mock("../services/api/contextCompress", () => ({ contextCompressApi: cctx }));

const ev = vi.hoisted(() => ({ handlers: new Map<string, () => void>() }));
vi.mock("../services/transport", () => ({
  invoke: vi.fn(),
  listen: vi.fn((name: string, h: () => void) => {
    ev.handlers.set(name, h);
    return Promise.resolve(() => ev.handlers.delete(name));
  }),
}));

const plat = vi.hoisted(() => ({ revealItemInDir: vi.fn(), homeDir: vi.fn() }));
vi.mock("../services/platform", () => plat);

import { Sessions } from "./Sessions";

// jsdom 无 PointerEvent，Radix Select 需要 capture / scrollIntoView 垫片。
beforeAll(() => {
  if (typeof (window as { PointerEvent?: unknown }).PointerEvent === "undefined") {
    (window as { PointerEvent?: unknown }).PointerEvent = MouseEvent;
  }
  Object.assign(Element.prototype, {
    hasPointerCapture: () => false,
    setPointerCapture: () => {},
    releasePointerCapture: () => {},
    scrollIntoView: () => {},
  });
});

const NOW = Date.now();
const H = 3600_000;
const IDLE = {
  running: false, clientCode: "", startedAtMs: 0, filesTotal: 0, filesDone: 0,
  callsImported: 0, parseErrors: 0, finishedAtMs: 0, error: "",
};
const ALL_ON = { claude_code: true, codex: true, gemini_cli: true, opencode: true };

const statusPayload = (enabled: Record<string, boolean>, imp = IDLE) => ({
  consent: { asked: true, enabled },
  import: imp,
  persistedParseErrors: 0,
});

const row = (clientCode: string, sessionId: string, title: string, workdir: string, ago: number, extra: Partial<Record<string, number>> = {}) => ({
  clientCode, sessionId, title, workdir,
  firstCallMs: NOW - ago - H, lastCallMs: NOW - ago,
  calls: 10, cost: 1.5, unpricedCalls: 0, inputTokens: 1000, outputTokens: 200,
  cacheReadTokens: 0, cacheWriteTokens: 0, reasoningTokens: 0, sidechainCalls: 0,
  ...extra,
});

const SESSIONS = [
  row("claude_code", "c1", "演示会话甲", "/work/alpha", H, { cacheReadTokens: 500, cacheWriteTokens: 60 }),
  row("codex", "x1", "演示会话乙", "/work/alpha", 2 * H, { unpricedCalls: 3 }),
  row("opencode", "o1", "演示会话丙", "/work/beta", 3 * H, { cost: 0, unpricedCalls: 10 }),
];

function setVisibility(v: "visible" | "hidden") {
  Object.defineProperty(document, "visibilityState", { configurable: true, get: () => v });
  Object.defineProperty(document, "hidden", { configurable: true, get: () => v === "hidden" });
  document.dispatchEvent(new Event("visibilitychange"));
}

beforeEach(() => {
  vi.clearAllMocks();
  ev.handlers.clear();
  setVisibility("visible");
  api.refresh.mockResolvedValue(IDLE);
  api.sessionStates.mockResolvedValue([]);
  cctx.contextModeSessions.mockResolvedValue([]);
  api.installedClients.mockResolvedValue([]);
  api.transcriptConsentGet.mockResolvedValue(false);
});

describe("Sessions 页（票 02）", () => {
  it("全部未启用：显示引导卡，去设置跳 settings/client_usage，不拉会话", async () => {
    api.status.mockResolvedValue(statusPayload({}));
    const onNavigate = vi.fn();
    render(<Sessions onNavigate={onNavigate} />);
    expect(await screen.findByTestId("sessions-guide")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("sessions-guide-go"));
    expect(onNavigate).toHaveBeenCalledWith("settings/client_usage");
    expect(api.sessions).not.toHaveBeenCalled();
    expect(screen.queryByTestId("sessions-toolbar")).toBeNull();
    expect(screen.queryByTestId("sessions-partial-off")).toBeNull();
  });

  it("部分启用：列表只含启用客户端", async () => {
    api.status.mockResolvedValue(statusPayload({ claude_code: true, codex: true, gemini_cli: true, opencode: false }));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions onNavigate={vi.fn()} />);
    await screen.findAllByTestId("sessions-row");
    const clients = screen.getAllByTestId("sessions-row").map((r) => r.getAttribute("data-client"));
    expect(clients).toEqual(["claude_code", "codex"]);
    expect(screen.queryByTestId("sessions-guide")).toBeNull();
  });

  it("正常列表：按工作目录分组，成本口径（全部未定价 / 部分 +）", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions onNavigate={vi.fn()} />);
    const groups = await screen.findAllByTestId("sessions-group");
    expect(groups.map((g) => g.getAttribute("data-workdir"))).toEqual(["/work/alpha", "/work/beta"]);
    expect(within(groups[0]).getAllByTestId("sessions-row")).toHaveLength(2);
    expect(screen.getByTestId("sessions-count")).toHaveAttribute("data-count", "3");
    const priced = screen.getAllByTestId("sessions-row-cost").map((c) => c.getAttribute("data-priced"));
    expect(priced).toEqual(["all", "partial", "none"]);
  });

  it("context-mode 压缩比：有数据的行显示徽标，展开详情同样带比值", async () => {
    cctx.contextModeSessions.mockResolvedValue([{ session_id: "c1", reduction_pct: 50.4, tokens_saved: 135051, total_calls: 77 }]);
    api.sessionDetail.mockResolvedValue({ modelChain: [], points: [{ callTimeMs: 1, inputTokens: 1, outputTokens: 1, cacheReadTokens: 0, cacheWriteTokens: 0 }], agents: [] });
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions onNavigate={vi.fn()} />);
    const rows = await screen.findAllByTestId("sessions-row");
    expect(rows.find((r) => r.getAttribute("data-session") === "c1")!.querySelector('[data-testid="sessions-row-ctx"]')!.getAttribute("data-pct")).toBe("50.4");
    expect(rows.find((r) => r.getAttribute("data-session") === "x1")!.querySelector('[data-testid="sessions-row-ctx"]')).toBeNull();
    fireEvent.click(rows.find((r) => r.getAttribute("data-session") === "c1")!);
    expect(await screen.findByTestId("session-detail-ctx")).toHaveAttribute("data-pct", "50.4");
  });

  it("搜索按标题与目录过滤", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions onNavigate={vi.fn()} />);
    await screen.findAllByTestId("sessions-row");
    fireEvent.change(screen.getByTestId("sessions-search"), { target: { value: "会话丙" } });
    expect(screen.getAllByTestId("sessions-row").map((r) => r.getAttribute("data-session"))).toEqual(["o1"]);
    fireEvent.change(screen.getByTestId("sessions-search"), { target: { value: "ALPHA" } });
    expect(screen.getAllByTestId("sessions-row").map((r) => r.getAttribute("data-session"))).toEqual(["c1", "x1"]);
    fireEvent.change(screen.getByTestId("sessions-search"), { target: { value: "不存在" } });
    expect(screen.queryAllByTestId("sessions-row")).toHaveLength(0);
    expect(screen.getByTestId("sessions-no-match")).toBeInTheDocument();
  });

  it("时间范围切换重拉会话", async () => {
    const user = userEvent.setup();
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions onNavigate={vi.fn()} />);
    await screen.findAllByTestId("sessions-row");
    const [from7] = api.sessions.mock.calls[0];
    await user.click(screen.getByTestId("sessions-range"));
    await user.click(screen.getByTestId("range-opt-30d"));
    await waitFor(() => expect(api.sessions).toHaveBeenCalledTimes(2));
    const [from30] = api.sessions.mock.calls[1];
    expect(from30).toBeLessThan(from7);
  });

  it("空数据：显示空态，重新扫描调 refresh", async () => {
    api.status.mockResolvedValue(statusPayload({ claude_code: true }));
    api.sessions.mockResolvedValue([]);
    render(<Sessions onNavigate={vi.fn()} />);
    expect(await screen.findByTestId("sessions-empty")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("sessions-rescan"));
    await waitFor(() => expect(api.refresh).toHaveBeenCalledTimes(1));
  });

  it("加载失败：role=alert 与重试", async () => {
    api.status.mockRejectedValueOnce(new Error("boom")).mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions onNavigate={vi.fn()} />);
    expect(await screen.findByTestId("sessions-load-error")).toHaveAttribute("role", "alert");
    fireEvent.click(screen.getByTestId("sessions-retry"));
    expect(await screen.findAllByTestId("sessions-row")).toHaveLength(3);
  });

  it("事件驱动：client-usage-updated 触发重拉；隐藏时不拉，重新可见补拉", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions onNavigate={vi.fn()} />);
    await screen.findAllByTestId("sessions-row");
    await waitFor(() => expect(ev.handlers.has("client-usage-updated")).toBe(true));
    expect(api.sessions).toHaveBeenCalledTimes(1);
    await act(async () => { ev.handlers.get("client-usage-updated")!(); });
    await waitFor(() => expect(api.sessions).toHaveBeenCalledTimes(2), { timeout: 3000 });
    act(() => setVisibility("hidden"));
    await act(async () => { ev.handlers.get("client-usage-updated")!(); });
    expect(api.sessions).toHaveBeenCalledTimes(2);
    act(() => setVisibility("visible"));
    await waitFor(() => expect(api.sessions).toHaveBeenCalledTimes(3));
  });
});

const seg = (model: string, calls: number, t: number) => ({
  model, calls, firstCallMs: t, lastCallMs: t + 1000,
  inputTokens: 10, outputTokens: 5, cacheReadTokens: 100, cacheWriteTokens: 1,
});
const pt = (t: number) => ({ callTimeMs: t, inputTokens: 10, outputTokens: 5, cacheReadTokens: 100, cacheWriteTokens: 1 });
const DETAIL = {
  modelChain: [seg("演示-sonnet", 2, NOW - 3000), seg("演示-opus", 1, NOW - 1000)],
  points: [pt(NOW - 3000), pt(NOW - 2000), pt(NOW - 1000)],
  agents: [],
};

describe("Sessions 页（票 03：缓存 token、模型链、消耗图）", () => {
  it("会话行 token 副行显示缓存读 / 写", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions onNavigate={vi.fn()} />);
    const cache = await screen.findAllByTestId("sessions-row-cache");
    expect(cache[0]).toHaveAttribute("data-read", "500");
    expect(cache[0]).toHaveAttribute("data-write", "60");
    expect(cache[1]).toHaveAttribute("data-read", "0");
  });

  it("点行内展开：只拉时间序列画消耗图；模型链点开后显示；再点收起；Enter 键可展开", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    api.sessionDetail.mockResolvedValue(DETAIL);
    render(<Sessions onNavigate={vi.fn()} />);
    const [first, second] = await screen.findAllByTestId("sessions-row");
    expect(first).toHaveAttribute("data-state", "closed");
    fireEvent.click(first);
    expect(first).toHaveAttribute("data-state", "open");
    expect(first).toHaveAttribute("aria-expanded", "true");
    const detail = await screen.findByTestId("session-detail");
    await waitFor(() => expect(detail).toHaveAttribute("data-state", "ready"));
    expect(api.sessionDetail).toHaveBeenCalledWith("claude_code", "c1", "points");
    expect(within(detail).getByTestId("session-detail-chart")).toHaveAttribute("data-points", "3");
    fireEvent.click(within(detail).getByTestId("session-detail-chain-toggle"));
    const segs = await within(detail).findAllByTestId("session-detail-seg");
    expect(segs.map((s) => [s.getAttribute("data-model"), s.getAttribute("data-calls")])).toEqual([["演示-sonnet", "2"], ["演示-opus", "1"]]);
    fireEvent.click(first);
    expect(first).toHaveAttribute("data-state", "closed");
    expect(screen.queryByTestId("session-detail")).toBeNull();
    fireEvent.keyDown(second, { key: "Enter" });
    expect(second).toHaveAttribute("data-state", "open");
    await waitFor(() => expect(api.sessionDetail).toHaveBeenLastCalledWith("codex", "x1", "points"));
  });

  it("详情空数据与失败两态", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    api.sessionDetail.mockResolvedValueOnce({ modelChain: [], points: [], agents: [] }).mockRejectedValueOnce(new Error("演示失败"));
    render(<Sessions onNavigate={vi.fn()} />);
    const [first, second] = await screen.findAllByTestId("sessions-row");
    fireEvent.click(first);
    await waitFor(() => expect(screen.getByTestId("session-detail")).toHaveAttribute("data-state", "empty"));
    fireEvent.click(first);
    fireEvent.click(second);
    const detail = await screen.findByTestId("session-detail");
    await waitFor(() => expect(detail).toHaveAttribute("data-state", "error"));
    expect(within(detail).getByRole("alert")).toBeInTheDocument();
  });
});

const ag = (kind: "main" | "subagent", agentId: string | null, agentType: string | null, calls: number) => ({
  kind, agentId, agentType, calls,
  inputTokens: 10 * calls, outputTokens: 5 * calls, cacheReadTokens: 100 * calls, cacheWriteTokens: calls,
  cost: 0.01 * calls, unpricedCalls: 0,
});

describe("Sessions 页（票 04：子代理展开、单次调用图表）", () => {
  it("展开区按主线 / 子代理分列：agentType 作名称，缺则显示 agentId 短串，无 id 显示通用子代理", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue([{ ...SESSIONS[0], sidechainCalls: 7 }, ...SESSIONS.slice(1)]);
    api.sessionDetail.mockResolvedValue({
      ...DETAIL,
      agents: [ag("main", null, null, 3), ag("subagent", "a1", "演示-Explore", 2), ag("subagent", "c3d4e5f6a7b8c9", null, 1), ag("subagent", null, null, 4)],
    });
    render(<Sessions onNavigate={vi.fn()} />);
    fireEvent.click((await screen.findAllByTestId("sessions-row"))[0]);
    const detail = await screen.findByTestId("session-detail");
    await waitFor(() => expect(detail).toHaveAttribute("data-state", "ready"));
    fireEvent.click(within(detail).getByTestId("session-detail-agents-toggle"));
    await within(detail).findAllByTestId("session-detail-agent");
    const rows = within(detail).queryAllByTestId("session-detail-agent");
    expect(rows.map((r) => [r.getAttribute("data-kind"), r.getAttribute("data-agent-id"), r.getAttribute("data-calls")])).toEqual([
      ["main", "", "3"], ["subagent", "a1", "2"], ["subagent", "c3d4e5f6a7b8c9", "1"], ["subagent", "", "4"],
    ]);
    const names = within(detail).getAllByTestId("session-detail-agent-name");
    expect(names[1]).toHaveTextContent("演示-Explore");
    expect(names[2]).toHaveTextContent("c3d4e5f6");
    expect(names[2]).not.toHaveTextContent("c3d4e5f6a7b8c9");
    expect(within(rows[1]).getByTestId("session-detail-agent-cost")).toHaveAttribute("data-priced", "all");
  });

  it("只有主线时（无子代理调用）不显示子代理区", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    api.sessionDetail.mockResolvedValue({ ...DETAIL, agents: [ag("main", null, null, 3)] });
    render(<Sessions onNavigate={vi.fn()} />);
    fireEvent.click((await screen.findAllByTestId("sessions-row"))[0]);
    const detail = await screen.findByTestId("session-detail");
    await waitFor(() => expect(detail).toHaveAttribute("data-state", "ready"));
    expect(within(detail).queryByTestId("session-detail-agents-toggle")).toBeNull();
    expect(within(detail).queryByTestId("session-detail-agents")).toBeNull();
  });

  it("只有 1 次调用时图表前后补零点，x 轴有可见宽度", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    api.sessionDetail.mockResolvedValue({ modelChain: [seg("演示-sonnet", 1, NOW - 1000)], points: [pt(NOW - 1000)], agents: [] });
    render(<Sessions onNavigate={vi.fn()} />);
    fireEvent.click((await screen.findAllByTestId("sessions-row"))[0]);
    const detail = await screen.findByTestId("session-detail");
    await waitFor(() => expect(detail).toHaveAttribute("data-state", "ready"));
    const chart = within(detail).getByTestId("session-detail-chart");
    expect(chart).toHaveAttribute("data-points", "1");
    expect(chart).toHaveAttribute("data-rows", "3");
  });
});

const MIN = 60_000;
const st = (clientCode: string, sessionId: string, state: string, source: string, lastActivityMs: number) =>
  ({ clientCode, sessionId, state, source, lastActivityMs });

describe("Sessions 页（票 05：运行状态）", () => {
  it("副行徽标显示三态：pid 文件 / 网关来源取后端结果，activity 来源按阈值派生", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    api.sessionStates.mockResolvedValue([
      st("claude_code", "c1", "running", "pid_file", NOW - H),
      st("codex", "x1", "idle", "activity", NOW - 10 * MIN),
      st("opencode", "o1", "stopped", "activity", NOW - 3 * H),
    ]);
    render(<Sessions onNavigate={vi.fn()} />);
    await waitFor(() => expect(api.sessionStates).toHaveBeenCalledWith(["c1", "x1", "o1"]));
    await waitFor(() => {
      const badges = screen.getAllByTestId("sessions-row-state");
      expect(badges.map((b) => b.getAttribute("data-state"))).toEqual(["running", "idle", "stopped"]);
    });
  });

  it("阈值到期：单次定时器只重新派生、不重拉数据；无 setInterval", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true, now: NOW });
    const interval = vi.spyOn(globalThis, "setInterval");
    try {
      api.status.mockResolvedValue(statusPayload(ALL_ON));
      api.sessions.mockResolvedValue([row("codex", "x1", "演示会话乙", "/work/alpha", MIN)]);
      api.sessionStates.mockResolvedValue([st("codex", "x1", "running", "activity", NOW - MIN)]);
      render(<Sessions onNavigate={vi.fn()} />);
      // 不用 waitFor（它自身用 setInterval 轮询，会污染断言）：直接冲刷微任务。
      await act(async () => { await vi.advanceTimersByTimeAsync(0); });
      expect(screen.getByTestId("sessions-row-state")).toHaveAttribute("data-state", "running");
      const fetches = api.sessions.mock.calls.length + api.sessionStates.mock.calls.length;
      await act(async () => { vi.advanceTimersByTime(MIN + 1000); });
      expect(screen.getByTestId("sessions-row-state")).toHaveAttribute("data-state", "idle");
      await act(async () => { vi.advanceTimersByTime(28 * MIN); });
      expect(screen.getByTestId("sessions-row-state")).toHaveAttribute("data-state", "stopped");
      expect(api.sessions.mock.calls.length + api.sessionStates.mock.calls.length).toBe(fetches);
      expect(interval).not.toHaveBeenCalled();
    } finally {
      interval.mockRestore();
      vi.useRealTimers();
    }
  });

  it("client-usage-updated 事件重拉运行状态", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    api.sessionStates.mockResolvedValueOnce([st("claude_code", "c1", "idle", "pid_file", NOW - H)])
      .mockResolvedValue([st("claude_code", "c1", "running", "gateway", NOW - H)]);
    render(<Sessions onNavigate={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByTestId("sessions-row-state")[0]).toHaveAttribute("data-state", "idle"));
    await waitFor(() => expect(ev.handlers.has("client-usage-updated")).toBe(true));
    await act(async () => { ev.handlers.get("client-usage-updated")!(); });
    await waitFor(() => expect(screen.getAllByTestId("sessions-row-state")[0]).toHaveAttribute("data-state", "running"), { timeout: 3000 });
  });
});

describe("继续会话（票 07）", () => {
  const RESUME_ROWS = [
    row("claude_code", "c1", "演示会话甲", "/work/alpha", H),
    row("gemini_cli", "g1", "演示无目录", "", 2 * H),
    row("gemini_cli", "g2", "演示有目录", "/work/beta", 3 * H),
  ];
  const bySid = (sid: string) => screen.getAllByTestId("sessions-row").find((r) => r.getAttribute("data-session") === sid)!;

  beforeEach(() => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(RESUME_ROWS);
    api.sessionDetail.mockReturnValue(new Promise(() => {}));
  });

  it("Gemini 无目录不渲染「继续」；点继续传 clientCode / sessionId / workdir，且不展开行", async () => {
    api.sessionResume.mockResolvedValue({ launched: true, command: "claude --resume 'c1'" });
    render(<Sessions />);
    await screen.findAllByTestId("sessions-row");
    expect(within(bySid("g1")).queryByTestId("sessions-resume")).toBeNull();
    expect(within(bySid("g2")).getByTestId("sessions-resume")).toBeInTheDocument();
    const btn = within(bySid("c1")).getByTestId("sessions-resume");
    fireEvent.click(btn);
    await waitFor(() => expect(api.sessionResume).toHaveBeenCalledWith("claude_code", "c1", "/work/alpha"));
    fireEvent.keyDown(btn, { key: "Enter" });
    expect(bySid("c1")).toHaveAttribute("data-state", "closed");
    expect(screen.queryByTestId("sessions-resume-notice")).toBeNull();
  });

  it("失败显示错误；本平台不能开终端时复制命令并提示", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    api.sessionResume.mockRejectedValueOnce("opencode session not found: o9");
    render(<Sessions />);
    await screen.findAllByTestId("sessions-row");
    fireEvent.click(within(bySid("c1")).getByTestId("sessions-resume"));
    await waitFor(() => expect(api.sessionResume).toHaveBeenCalledWith("claude_code", "c1", "/work/alpha"));
    const err = await screen.findByTestId("sessions-resume-notice");
    expect(err).toHaveAttribute("data-kind", "error");
    expect(err).toHaveAttribute("data-detail", "opencode session not found: o9");
    api.sessionResume.mockResolvedValueOnce({ launched: false, command: "gemini --resume 'g2'" });
    fireEvent.click(within(bySid("g2")).getByTestId("sessions-resume"));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith("gemini --resume 'g2'"));
    await waitFor(() => expect(screen.getByTestId("sessions-resume-notice")).toHaveAttribute("data-kind", "copied"));
  });
});

describe("打开工作目录（票 08）", () => {
  const OPEN_ROWS = [
    row("claude_code", "c1", "演示会话甲", "/work/alpha", H),
    row("gemini_cli", "g1", "演示无目录", "", 2 * H),
    row("codex", "t1", "演示家目录", "~/demo/proj", 3 * H),
    row("opencode", "d1", "演示怪路径", "-rf", 4 * H),
  ];
  const bySid = (sid: string) => screen.getAllByTestId("sessions-row").find((r) => r.getAttribute("data-session") === sid)!;
  const groupOf = (wd: string) => screen.getAllByTestId("sessions-group").find((g) => g.getAttribute("data-workdir") === wd)!;

  beforeEach(() => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(OPEN_ROWS);
    api.sessionDetail.mockReturnValue(new Promise(() => {}));
    plat.revealItemInDir.mockResolvedValue(undefined);
    plat.homeDir.mockResolvedValue("/Users/demo/");
  });

  it("行内与分组头：有目录调用一次 revealItemInDir 且不展开行；无目录不渲染", async () => {
    render(<Sessions />);
    await screen.findAllByTestId("sessions-row");
    expect(within(bySid("g1")).queryByTestId("sessions-open-dir")).toBeNull();
    expect(within(groupOf("")).queryByTestId("sessions-group-open-dir")).toBeNull();
    const btn = within(bySid("c1")).getByTestId("sessions-open-dir");
    fireEvent.click(btn);
    await waitFor(() => expect(plat.revealItemInDir).toHaveBeenCalledTimes(1));
    expect(plat.revealItemInDir).toHaveBeenCalledWith("/work/alpha");
    fireEvent.keyDown(btn, { key: "Enter" });
    expect(bySid("c1")).toHaveAttribute("data-state", "closed");
    fireEvent.click(within(groupOf("/work/alpha")).getByTestId("sessions-group-open-dir"));
    await waitFor(() => expect(plat.revealItemInDir).toHaveBeenCalledTimes(2));
    expect(plat.revealItemInDir).toHaveBeenLastCalledWith("/work/alpha");
  });

  it("~ 开头先展开为绝对路径", async () => {
    render(<Sessions />);
    await screen.findAllByTestId("sessions-row");
    fireEvent.click(within(bySid("t1")).getByTestId("sessions-open-dir"));
    await waitFor(() => expect(plat.revealItemInDir).toHaveBeenCalledWith("/Users/demo/demo/proj"));
  });

  it("打开失败显示错误；以 - 开头的路径拒绝且不调用", async () => {
    plat.revealItemInDir.mockRejectedValueOnce("path does not exist");
    render(<Sessions />);
    await screen.findAllByTestId("sessions-row");
    fireEvent.click(within(bySid("c1")).getByTestId("sessions-open-dir"));
    await waitFor(() => expect(screen.queryByTestId("sessions-resume-notice")?.getAttribute("data-kind")).toBe("open-error"));
    expect(screen.getByTestId("sessions-resume-notice")).toHaveAttribute("data-detail", "path does not exist");
    plat.revealItemInDir.mockClear();
    fireEvent.click(within(bySid("d1")).getByTestId("sessions-open-dir"));
    await waitFor(() => expect(screen.getByTestId("sessions-resume-notice")).toHaveAttribute("data-detail", "invalid path: -rf"));
    expect(plat.revealItemInDir).not.toHaveBeenCalled();
  });
});

describe("排序、正在执行置顶、tab 与官方图标（票 11）", () => {
  const SEC = 1000;
  const ids = () => screen.getAllByTestId("sessions-row").map((r) => r.getAttribute("data-session"));
  const groupOf = (wd: string) => screen.getAllByTestId("sessions-group").find((g) => g.getAttribute("data-workdir") === wd)!;
  // 乱序输入；全部已结束（无状态，按最近调用派生）。
  const SORT_ROWS = [
    row("claude_code", "a-old", "演示甲旧", "/work/alpha", 5 * H),
    row("codex", "b-new", "演示乙新", "/work/beta", H),
    row("gemini_cli", "g-1", "演示丙", "/work/gamma", 3 * H),
    row("opencode", "a-new", "演示甲新", "/work/alpha", 2 * H),
  ];
  // 置顶判定样本：x-run 执行中（activity）、c-pid Claude pid 文件 idle、g-idle 非 Claude idle、其余已结束。
  const PIN_ROWS = [
    row("claude_code", "c-old", "演示旧", "/work/alpha", 5 * H),
    row("codex", "x-new", "演示新", "/work/beta", H),
    row("claude_code", "c-pid", "演示窗口", "/work/alpha", 4 * H),
    row("codex", "x-run", "演示执行", "/work/beta", 10 * SEC),
    row("gemini_cli", "g-idle", "演示空闲", "/work/gamma", 10 * MIN),
    row("opencode", "o-mid", "演示中", "/work/alpha", 2 * H),
  ];
  const PIN_STATES = [
    st("claude_code", "c-pid", "idle", "pid_file", NOW - 20 * MIN),
    st("codex", "x-run", "running", "activity", NOW - 10 * SEC),
    st("gemini_cli", "g-idle", "idle", "activity", NOW - 10 * MIN),
  ];

  it("目录组按组内最近会话倒序；组内按时间倒序", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SORT_ROWS);
    render(<Sessions onNavigate={vi.fn()} />);
    const groups = await screen.findAllByTestId("sessions-group");
    expect(groups.map((g) => g.getAttribute("data-workdir"))).toEqual(["/work/beta", "/work/alpha", "/work/gamma"]);
    expect(within(groups[1]).getAllByTestId("sessions-row").map((r) => r.getAttribute("data-session"))).toEqual(["a-new", "a-old"]);
  });

  it("没有正在执行的会话时不显示置顶区", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SORT_ROWS);
    render(<Sessions onNavigate={vi.fn()} />);
    await screen.findAllByTestId("sessions-group");
    expect(screen.queryByTestId("sessions-pinned")).toBeNull();
  });

  it("置顶区：执行中 + Claude pid 文件（idle 也算）平铺按活动时间倒序；非 Claude 空闲不入", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(PIN_ROWS);
    api.sessionStates.mockResolvedValue(PIN_STATES);
    render(<Sessions onNavigate={vi.fn()} />);
    const pinned = await screen.findByTestId("sessions-pinned");
    expect(within(pinned).getAllByTestId("sessions-row").map((r) => r.getAttribute("data-session"))).toEqual(["x-run", "c-pid"]);
    expect(within(pinned).queryAllByTestId("sessions-group")).toHaveLength(0);
    expect(within(groupOf("/work/gamma")).getAllByTestId("sessions-row").map((r) => r.getAttribute("data-session"))).toEqual(["g-idle"]);
  });

  it("置顶会话不在目录组重复显示，目录组会话数与成本仍计入", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(PIN_ROWS);
    api.sessionStates.mockResolvedValue(PIN_STATES);
    render(<Sessions onNavigate={vi.fn()} />);
    await screen.findByTestId("sessions-pinned");
    const alpha = groupOf("/work/alpha");
    expect(within(alpha).getAllByTestId("sessions-row").map((r) => r.getAttribute("data-session"))).toEqual(["o-mid", "c-old"]);
    expect(alpha).toHaveAttribute("data-count", "3");
    expect(alpha).toHaveAttribute("data-cost", "4.5");
    expect(groupOf("/work/beta")).toHaveAttribute("data-count", "2");
    expect(ids().filter((s) => s === "c-pid")).toHaveLength(1);
  });

  it("tab：默认全部；选 Codex 只剩 Codex（置顶区与目录组都过滤）；未启用客户端 tab 禁用；无四格", async () => {
    api.status.mockResolvedValue(statusPayload({ claude_code: true, codex: true, gemini_cli: true, opencode: false }));
    api.sessions.mockResolvedValue(PIN_ROWS);
    api.sessionStates.mockResolvedValue(PIN_STATES);
    render(<Sessions onNavigate={vi.fn()} />);
    await screen.findByTestId("sessions-pinned");
    expect(screen.queryByTestId("sessions-tiles")).toBeNull();
    expect(screen.getByTestId("sessions-tab-all")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByTestId("sessions-tab-opencode")).toBeDisabled();
    fireEvent.click(screen.getByTestId("sessions-tab-codex"));
    expect(screen.getByTestId("sessions-tab-codex")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByTestId("sessions-tab-all")).toHaveAttribute("aria-selected", "false");
    expect(within(screen.getByTestId("sessions-pinned")).getAllByTestId("sessions-row").map((r) => r.getAttribute("data-session"))).toEqual(["x-run"]);
    expect(ids()).toEqual(["x-run", "x-new"]);
    fireEvent.click(screen.getByTestId("sessions-tab-all"));
    expect(ids()).toHaveLength(5);
  });

  it("部分未启用：列表上方提示未启用的客户端，去设置跳 settings/client_usage；全部启用时不提示", async () => {
    api.status.mockResolvedValue(statusPayload({ claude_code: true, codex: true, gemini_cli: false, opencode: false }));
    api.sessions.mockResolvedValue(SESSIONS);
    const onNavigate = vi.fn();
    const { unmount } = render(<Sessions onNavigate={onNavigate} />);
    const hint = await screen.findByTestId("sessions-partial-off");
    expect(hint).toHaveAttribute("data-clients", "gemini_cli,opencode");
    fireEvent.click(within(hint).getByTestId("sessions-partial-off-go"));
    expect(onNavigate).toHaveBeenCalledWith("settings/client_usage");
    unmount();
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    render(<Sessions onNavigate={vi.fn()} />);
    await screen.findAllByTestId("sessions-row");
    expect(screen.queryByTestId("sessions-partial-off")).toBeNull();
  });

  it("会话行与 tab 用客户端官方图标（打包本地 SVG）", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions onNavigate={vi.fn()} />);
    const rows = await screen.findAllByTestId("sessions-row");
    for (const r of rows) {
      const icon = within(r).getByTestId("client-icon");
      expect(icon).toHaveAttribute("data-client", r.getAttribute("data-client"));
      expect(icon.getAttribute("src")).toBeTruthy();
    }
    for (const c of ["claude_code", "codex", "gemini_cli", "opencode"]) {
      expect(within(screen.getByTestId(`sessions-tab-${c}`)).getByTestId("client-icon")).toHaveAttribute("data-client", c);
    }
  });
});

describe("Sessions 页（票 09：从列表隐藏与回收站）", () => {
  it("行内隐藏：调 sessionHide、行消失、计数减少、不展开行；撤销调 unhide 并重拉", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    api.sessionHide.mockResolvedValue(undefined);
    api.sessionUnhide.mockResolvedValue(undefined);
    render(<Sessions />);
    await waitFor(() => expect(screen.getAllByTestId("sessions-row")).toHaveLength(3));
    expect(screen.getByTestId("sessions-count")).toHaveAttribute("data-count", "3");
    const target = screen.getAllByTestId("sessions-row").find((r) => r.getAttribute("data-session") === "x1")!;
    fireEvent.click(within(target).getByTestId("sessions-hide"));
    await waitFor(() => expect(api.sessionHide).toHaveBeenCalledWith("codex", "x1"));
    await waitFor(() => expect(screen.getAllByTestId("sessions-row")).toHaveLength(2));
    expect(screen.getAllByTestId("sessions-row").map((r) => r.getAttribute("data-session"))).not.toContain("x1");
    expect(screen.getByTestId("sessions-count")).toHaveAttribute("data-count", "2");
    expect(screen.getAllByTestId("sessions-row").every((r) => r.getAttribute("data-state") === "closed")).toBe(true);
    const calls = api.sessions.mock.calls.length;
    fireEvent.click(screen.getByTestId("sessions-hide-undo"));
    await waitFor(() => expect(api.sessionUnhide).toHaveBeenCalledWith("codex", "x1"));
    await waitFor(() => expect(api.sessions.mock.calls.length).toBeGreaterThan(calls));
  });

  it("回收站：Dialog 列出隐藏会话，恢复后调 unhide、从回收站移除并重拉列表", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS.slice(0, 2));
    api.hiddenSessions.mockResolvedValue([{ ...SESSIONS[2], hiddenAt: NOW - H }]);
    api.sessionUnhide.mockResolvedValue(undefined);
    render(<Sessions />);
    await waitFor(() => expect(screen.getAllByTestId("sessions-row")).toHaveLength(2));
    fireEvent.click(screen.getByTestId("sessions-trash"));
    const dialog = await screen.findByTestId("sessions-trash-dialog");
    await waitFor(() => expect(within(dialog).queryAllByTestId("sessions-trash-row").map((r) => r.getAttribute("data-session"))).toEqual(["o1"]));
    const items = within(dialog).getAllByTestId("sessions-trash-row");
    api.sessions.mockResolvedValue(SESSIONS);
    fireEvent.click(within(items[0]).getByTestId("sessions-trash-restore"));
    await waitFor(() => expect(api.sessionUnhide).toHaveBeenCalledWith("opencode", "o1"));
    await waitFor(() => expect(within(dialog).queryAllByTestId("sessions-trash-row")).toHaveLength(0));
    expect(within(dialog).getByTestId("sessions-trash-empty")).toBeInTheDocument();
    await waitFor(() => expect(screen.getAllByTestId("sessions-row")).toHaveLength(3));
  });
});

describe("真机反馈修复与懒加载（票 12）", () => {
  const bySid = (sid: string) => screen.getAllByTestId("sessions-row").find((r) => r.getAttribute("data-session") === sid)!;
  const partCalls = (part: string) => api.sessionDetail.mock.calls.filter((c) => c[2] === part);

  it("继续：请求返回前按钮禁用，连点只调一次", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    let done: (v: unknown) => void = () => {};
    api.sessionResume.mockReturnValue(new Promise((r) => { done = r; }));
    render(<Sessions />);
    await screen.findAllByTestId("sessions-row");
    const btn = within(bySid("c1")).getByTestId("sessions-resume");
    fireEvent.click(btn);
    fireEvent.click(btn);
    fireEvent.click(btn);
    expect(api.sessionResume).toHaveBeenCalledTimes(1);
    expect(btn).toBeDisabled();
    await act(async () => { done({ launched: true, command: "x" }); });
    expect(within(bySid("c1")).getByTestId("sessions-resume")).not.toBeDisabled();
  });

  it("行内操作失败的提示挂在视口固定浮层（portal 到 body）", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    plat.revealItemInDir.mockRejectedValueOnce("path does not exist");
    render(<Sessions />);
    await screen.findAllByTestId("sessions-row");
    fireEvent.click(within(bySid("c1")).getByTestId("sessions-open-dir"));
    const notice = await screen.findByTestId("sessions-resume-notice");
    expect(notice).toHaveAttribute("data-kind", "open-error");
    const layer = notice.closest("[data-testid='sessions-notice-layer']");
    expect(layer).not.toBeNull();
    expect(layer).toHaveClass("fixed");
    expect(layer!.parentElement).toBe(document.body);
    expect(screen.getByTestId("sessions-page").contains(notice)).toBe(false);
    fireEvent.click(screen.getByTestId("sessions-notice-close"));
    expect(screen.queryByTestId("sessions-resume-notice")).toBeNull();
  });

  it("事件防抖合并；可见性恢复只在漏过事件时补拉；点打开目录不重拉", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true, now: NOW });
    try {
      api.status.mockResolvedValue(statusPayload(ALL_ON));
      api.sessions.mockResolvedValue(SESSIONS);
      plat.revealItemInDir.mockResolvedValue(undefined);
      render(<Sessions />);
      await act(async () => { await vi.advanceTimersByTimeAsync(0); });
      expect(api.sessions).toHaveBeenCalledTimes(1);
      const fire = () => ev.handlers.get("client-usage-updated")!();
      await act(async () => { fire(); await vi.advanceTimersByTimeAsync(300); fire(); await vi.advanceTimersByTimeAsync(300); fire(); });
      expect(api.sessions).toHaveBeenCalledTimes(1);
      await act(async () => { await vi.advanceTimersByTimeAsync(1600); });
      expect(api.sessions).toHaveBeenCalledTimes(2);
      // 没漏事件：切走再回来不补拉。
      act(() => setVisibility("hidden"));
      act(() => setVisibility("visible"));
      await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
      expect(api.sessions).toHaveBeenCalledTimes(2);
      fireEvent.click(within(bySid("c1")).getByTestId("sessions-open-dir"));
      await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
      expect(plat.revealItemInDir).toHaveBeenCalledTimes(1);
      expect(api.sessions).toHaveBeenCalledTimes(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("模型链 / 子代理默认折叠不请求；点击请求一次；再收再开不重复请求", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue([{ ...SESSIONS[0], sidechainCalls: 2 }, ...SESSIONS.slice(1)]);
    api.sessionDetail.mockResolvedValue({ ...DETAIL, agents: [ag("main", null, null, 3), ag("subagent", "a1", "演示-Explore", 2)] });
    render(<Sessions />);
    fireEvent.click((await screen.findAllByTestId("sessions-row"))[0]);
    const detail = await screen.findByTestId("session-detail");
    await waitFor(() => expect(detail).toHaveAttribute("data-state", "ready"));
    expect(partCalls("chain")).toHaveLength(0);
    expect(partCalls("agents")).toHaveLength(0);
    expect(within(detail).queryByTestId("session-detail-chain")).toBeNull();
    expect(within(detail).queryByTestId("session-detail-agents")).toBeNull();
    const chain = within(detail).getByTestId("session-detail-chain-toggle");
    expect(chain).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(chain);
    expect(await within(detail).findByTestId("session-detail-chain")).toBeInTheDocument();
    expect(partCalls("chain")).toHaveLength(1);
    fireEvent.click(chain);
    expect(within(detail).queryByTestId("session-detail-chain")).toBeNull();
    fireEvent.click(chain);
    expect(within(detail).getByTestId("session-detail-chain")).toBeInTheDocument();
    expect(partCalls("chain")).toHaveLength(1);
    const agents = within(detail).getByTestId("session-detail-agents-toggle");
    fireEvent.click(agents);
    expect(await within(detail).findByTestId("session-detail-agents")).toBeInTheDocument();
    fireEvent.click(agents);
    fireEvent.click(agents);
    expect(partCalls("agents")).toHaveLength(1);
    expect(partCalls("points")).toHaveLength(1);
  });

  it("模型链读取失败：显示失败态", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    api.sessionDetail.mockImplementation((_c: string, _s: string, part: string) =>
      part === "chain" ? Promise.reject(new Error("演示失败")) : Promise.resolve(DETAIL));
    render(<Sessions />);
    fireEvent.click((await screen.findAllByTestId("sessions-row"))[0]);
    const detail = await screen.findByTestId("session-detail");
    await waitFor(() => expect(detail).toHaveAttribute("data-state", "ready"));
    fireEvent.click(within(detail).getByTestId("session-detail-chain-toggle"));
    await waitFor(() => expect(within(detail).getByTestId("session-detail-chain-block")).toHaveAttribute("data-state", "error"));
  });

  it("置顶区行有 token 走势图，目录组行没有；不请求置顶区以外的会话", async () => {
    const PIN = [row("codex", "x-run", "演示执行", "/work/beta", 10_000), row("claude_code", "c-old", "演示旧", "/work/alpha", 5 * H)];
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(PIN);
    api.sessionStates.mockResolvedValue([st("codex", "x-run", "running", "activity", NOW - 10_000)]);
    api.sessionDetail.mockResolvedValue(DETAIL);
    render(<Sessions />);
    const pinned = await screen.findByTestId("sessions-pinned");
    const spark = await within(pinned).findByTestId("sessions-row-spark");
    await waitFor(() => expect(spark).toHaveAttribute("data-bars", "3"));
    expect(spark).toHaveAttribute("data-total", "348");
    expect(spark).toHaveAttribute("aria-label", "sessions.sparkLabel");
    const group = screen.getAllByTestId("sessions-group")[0];
    expect(within(group).queryByTestId("sessions-row-spark")).toBeNull();
    expect(api.sessionDetail.mock.calls.map((c) => c[1])).toEqual(["x-run"]);
  });
});

describe("换客户端继续（票 10）", () => {
  const CARRY_ROWS = [
    row("claude_code", "c1", "演示会话甲", "/work/alpha", H),
    row("gemini_cli", "g1", "演示无目录", "", 2 * H),
  ];
  const bySid = (sid: string) => screen.getAllByTestId("sessions-row").find((r) => r.getAttribute("data-session") === sid)!;
  const openMenu = async (sid: string) => {
    await userEvent.click(within(bySid(sid)).getByTestId("sessions-carry"));
    return screen.findByTestId("sessions-carry-menu");
  };

  beforeEach(() => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(CARRY_ROWS);
    api.sessionDetail.mockReturnValue(new Promise(() => {}));
  });

  it("菜单只列本机已安装的其它客户端；无目录会话不提供", async () => {
    api.installedClients.mockResolvedValue(["claude_code", "codex", "gemini_cli"]);
    render(<Sessions />);
    await screen.findAllByTestId("sessions-row");
    await waitFor(() => expect(within(bySid("c1")).getByTestId("sessions-carry")).toBeInTheDocument());
    expect(within(bySid("g1")).queryByTestId("sessions-carry")).toBeNull();
    const menu = await openMenu("c1");
    const items = within(menu).getAllByTestId("sessions-carry-item").map((i) => i.getAttribute("data-target"));
    expect(items).toEqual(["codex", "gemini_cli"]);
    expect(bySid("c1")).toHaveAttribute("data-state", "closed");
  });

  it("没装其它客户端：不渲染菜单入口", async () => {
    api.installedClients.mockResolvedValue(["claude_code"]);
    render(<Sessions />);
    await screen.findAllByTestId("sessions-row");
    await waitFor(() => expect(api.installedClients).toHaveBeenCalled());
    expect(within(bySid("c1")).queryByTestId("sessions-carry")).toBeNull();
  });

  it("原文开关关：Codex 项显示引导，点它跳设置且不调转写；交接项照常可用", async () => {
    api.installedClients.mockResolvedValue(["codex", "gemini_cli"]);
    api.sessionCarry.mockResolvedValue({ launched: true, command: "x" });
    const onNavigate = vi.fn();
    render(<Sessions onNavigate={onNavigate} />);
    await screen.findAllByTestId("sessions-row");
    await waitFor(() => expect(within(bySid("c1")).getByTestId("sessions-carry")).toBeInTheDocument());
    let menu = await openMenu("c1");
    const codex = within(menu).getAllByTestId("sessions-carry-item").find((i) => i.getAttribute("data-target") === "codex")!;
    expect(codex).toHaveAttribute("data-mode", "need-consent");
    expect(within(codex).getByTestId("sessions-carry-need-consent")).toBeInTheDocument();
    await userEvent.click(codex);
    expect(onNavigate).toHaveBeenCalledWith("settings/client_usage");
    expect(api.sessionCarry).not.toHaveBeenCalled();
    menu = await openMenu("c1");
    const gem = within(menu).getAllByTestId("sessions-carry-item").find((i) => i.getAttribute("data-target") === "gemini_cli")!;
    expect(gem).toHaveAttribute("data-mode", "handoff");
    await userEvent.click(gem);
    await waitFor(() => expect(api.sessionCarry).toHaveBeenCalledWith("claude_code", "c1", "/work/alpha", "gemini_cli"));
  });

  it("原文开关开：Codex 项为转写，点选调 session_carry；不能开终端时复制命令", async () => {
    api.installedClients.mockResolvedValue(["codex"]);
    api.transcriptConsentGet.mockResolvedValue(true);
    api.sessionCarry.mockResolvedValue({ launched: false, command: "codex resume 'n1'" });
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    render(<Sessions />);
    await screen.findAllByTestId("sessions-row");
    await waitFor(() => expect(within(bySid("c1")).getByTestId("sessions-carry")).toBeInTheDocument());
    const menu = await openMenu("c1");
    const codex = within(menu).getAllByTestId("sessions-carry-item")[0];
    await waitFor(() => expect(codex).toHaveAttribute("data-mode", "transcribe"));
    await userEvent.click(codex);
    await waitFor(() => expect(api.sessionCarry).toHaveBeenCalledWith("claude_code", "c1", "/work/alpha", "codex"));
    expect(await screen.findByTestId("sessions-resume-notice")).toHaveAttribute("data-kind", "copied");
    expect(writeText).toHaveBeenCalledWith("codex resume 'n1'");
  });
});

describe("遗留六项（票 13）", () => {
  const PIN = [row("codex", "x-run", "演示执行", "/work/beta", 10_000), row("claude_code", "c-old", "演示旧", "/work/alpha", 5 * H)];
  const bySid = (sid: string) => screen.getAllByTestId("sessions-row").find((r) => r.getAttribute("data-session") === sid)!;

  it("置顶区行与目录组行同一列模板：目录组行留出走势图空位，token 列对齐", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(PIN);
    api.sessionStates.mockResolvedValue([st("codex", "x-run", "running", "activity", NOW - 10_000)]);
    api.sessionDetail.mockReturnValue(new Promise(() => {}));
    render(<Sessions />);
    await screen.findByTestId("sessions-pinned");
    const tpl = (el: HTMLElement) => el.className.split(" ").find((c) => c.startsWith("grid-cols-"));
    expect(tpl(bySid("c-old"))).toBe(tpl(bySid("x-run")));
    expect(within(bySid("c-old")).getByTestId("sessions-row-spark-slot")).toBeInTheDocument();
    expect(within(bySid("c-old")).queryByTestId("sessions-row-spark")).toBeNull();
  });

  it("RTL：会话行第二行每段各自隔离方向（bdi），数字不与相邻段混排", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions />);
    await waitFor(() => expect(screen.getAllByTestId("sessions-row")).toHaveLength(3));
    const meta = within(bySid("c1")).getByTestId("sessions-row-meta");
    const segs = [...meta.querySelectorAll("bdi")].map((b) => b.textContent);
    expect(segs).toEqual(expect.arrayContaining(["sessions.calls", "sessions.duration"]));
    expect(segs.length).toBeGreaterThanOrEqual(3);
  });

  it("RTL：回收站的标题与目录路径用 bdi 隔离，路径固定 ltr", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS.slice(0, 2));
    api.hiddenSessions.mockResolvedValue([{ ...SESSIONS[2], hiddenAt: NOW - H }]);
    render(<Sessions />);
    await waitFor(() => expect(screen.getAllByTestId("sessions-row")).toHaveLength(2));
    fireEvent.click(screen.getByTestId("sessions-trash"));
    const dialog = await screen.findByTestId("sessions-trash-dialog");
    const item = (await within(dialog).findAllByTestId("sessions-trash-row"))[0];
    const path = within(item).getByTestId("sessions-trash-path");
    expect(path.tagName).toBe("BDI");
    expect(path).toHaveAttribute("dir", "ltr");
    expect(path).toHaveTextContent("/work/beta");
    expect(within(item).getByTestId("sessions-trash-title").tagName).toBe("BDI");
  });
});
