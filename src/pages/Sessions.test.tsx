// Sessions 页就近测试（扩展-会话页 票 02）：渲染真组件，mock api / transport，断言走 data-testid / data-state。
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import userEvent from "@testing-library/user-event";
import { act, fireEvent, render, screen, waitFor, within } from "../test/render";

const api = vi.hoisted(() => ({
  status: vi.fn(),
  sessions: vi.fn(),
  refresh: vi.fn(),
}));
vi.mock("../services/api/clientUsage", () => ({ clientUsageApi: api }));

const ev = vi.hoisted(() => ({ handlers: new Map<string, () => void>() }));
vi.mock("../services/transport", () => ({
  invoke: vi.fn(),
  listen: vi.fn((name: string, h: () => void) => {
    ev.handlers.set(name, h);
    return Promise.resolve(() => ev.handlers.delete(name));
  }),
}));

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
  ...extra,
});

const SESSIONS = [
  row("claude_code", "c1", "演示会话甲", "/work/alpha", H),
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
});

describe("Sessions 页（票 02）", () => {
  it("全部未启用：四格皆 off、显示引导卡，去设置跳 settings/client_usage，不拉会话", async () => {
    api.status.mockResolvedValue(statusPayload({}));
    const onNavigate = vi.fn();
    render(<Sessions onNavigate={onNavigate} />);
    expect(await screen.findByTestId("sessions-guide")).toBeInTheDocument();
    for (const c of ["claude_code", "codex", "gemini_cli", "opencode"]) {
      expect(screen.getByTestId(`sessions-tile-${c}`)).toHaveAttribute("data-state", "off");
    }
    fireEvent.click(screen.getByTestId("sessions-guide-go"));
    expect(onNavigate).toHaveBeenCalledWith("settings/client_usage");
    fireEvent.click(screen.getByTestId("sessions-tile-go-codex"));
    expect(onNavigate).toHaveBeenCalledTimes(2);
    expect(api.sessions).not.toHaveBeenCalled();
    expect(screen.queryByTestId("sessions-toolbar")).toBeNull();
  });

  it("部分启用：启用格 on、未启用格有去设置按钮、导入中的格显示进度；列表只含启用客户端", async () => {
    api.status.mockResolvedValue(statusPayload(
      { claude_code: true, codex: true, gemini_cli: true, opencode: false },
      { ...IDLE, running: true, clientCode: "gemini_cli", filesTotal: 10, filesDone: 4 },
    ));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions onNavigate={vi.fn()} />);
    await screen.findAllByTestId("sessions-row");
    expect(screen.getByTestId("sessions-tile-claude_code")).toHaveAttribute("data-state", "on");
    expect(screen.getByTestId("sessions-tile-opencode")).toHaveAttribute("data-state", "off");
    expect(screen.getByTestId("sessions-tile-go-opencode")).toBeInTheDocument();
    expect(screen.queryByTestId("sessions-tile-go-codex")).toBeNull();
    expect(screen.getByTestId("sessions-tile-import-gemini_cli")).toHaveAttribute("data-progress", "40");
    const clients = screen.getAllByTestId("sessions-row").map((r) => r.getAttribute("data-client"));
    expect(clients).toEqual(["claude_code", "codex"]);
    expect(screen.queryByTestId("sessions-guide")).toBeNull();
  });

  it("正常列表：按工作目录分组，四格计数由列表派生，成本口径（全部未定价 / 部分 +）", async () => {
    api.status.mockResolvedValue(statusPayload(ALL_ON));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions onNavigate={vi.fn()} />);
    const groups = await screen.findAllByTestId("sessions-group");
    expect(groups.map((g) => g.getAttribute("data-workdir"))).toEqual(["/work/alpha", "/work/beta"]);
    expect(within(groups[0]).getAllByTestId("sessions-row")).toHaveLength(2);
    expect(screen.getByTestId("sessions-tile-count-codex")).toHaveAttribute("data-count", "1");
    expect(screen.getByTestId("sessions-tile-count-gemini_cli")).toHaveAttribute("data-count", "0");
    expect(screen.getByTestId("sessions-count")).toHaveAttribute("data-count", "3");
    const priced = screen.getAllByTestId("sessions-row-cost").map((c) => c.getAttribute("data-priced"));
    expect(priced).toEqual(["all", "partial", "none"]);
  });

  it("点已启用格筛选、再点取消；清除筛选；未启用格不可选", async () => {
    api.status.mockResolvedValue(statusPayload({ claude_code: true, codex: true, gemini_cli: true, opencode: false }));
    api.sessions.mockResolvedValue(SESSIONS);
    render(<Sessions onNavigate={vi.fn()} />);
    await screen.findAllByTestId("sessions-row");
    const tile = screen.getByTestId("sessions-tile-codex");
    fireEvent.click(tile);
    expect(tile).toHaveAttribute("data-selected", "true");
    expect(screen.getAllByTestId("sessions-row").map((r) => r.getAttribute("data-client"))).toEqual(["codex"]);
    fireEvent.click(tile);
    expect(tile).toHaveAttribute("data-selected", "false");
    expect(screen.getAllByTestId("sessions-row")).toHaveLength(2);
    fireEvent.click(tile);
    fireEvent.click(screen.getByTestId("sessions-clear-filter"));
    expect(tile).toHaveAttribute("data-selected", "false");
    fireEvent.click(screen.getByTestId("sessions-tile-opencode"));
    expect(screen.getByTestId("sessions-tile-opencode")).toHaveAttribute("data-selected", "false");
    expect(screen.getAllByTestId("sessions-row")).toHaveLength(2);
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
    await waitFor(() => expect(api.sessions).toHaveBeenCalledTimes(2));
    act(() => setVisibility("hidden"));
    await act(async () => { ev.handlers.get("client-usage-updated")!(); });
    expect(api.sessions).toHaveBeenCalledTimes(2);
    act(() => setVisibility("visible"));
    await waitFor(() => expect(api.sessions).toHaveBeenCalledTimes(3));
  });
});
