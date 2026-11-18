// ContextCompress 页就近测试：渲染真组件，mock api，断言走 data-testid / data-state。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";

const api = vi.hoisted(() => ({ status: vi.fn(), install: vi.fn(), setRtk: vi.fn(), setSqz: vi.fn(), stats: vi.fn() }));
vi.mock("../services/api/contextCompress", () => ({ contextCompressApi: api }));

import { ContextCompress, DailySavedTooltip } from "./ContextCompress";

const STATUS = {
  rtk_version: "1.2.0",
  rtk_claude: true,
  rtk_codex: false,
  codex_present: true,
  sqz_version: "2.2.0",
  sqz_mcp_version: "2.2.0",
  sqz_enabled: false,
  sqz_missing: ["wrap:claude:octo"],
};

const RTK_RESULT = { status: STATUS, claude_error: null, codex_error: null };

// 当地 ISO 日（YYYY-MM-DD），与组件的区间过滤同一口径。
const isoDay = (offset: number) => {
  const d = new Date();
  d.setDate(d.getDate() - offset);
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
};

const RTK_DAILY = [
  { date: isoDay(0), commands: 10, input_tokens: 1000, output_tokens: 600, saved_tokens: 400, savings_pct: 40 },
  { date: isoDay(5), commands: 20, input_tokens: 2000, output_tokens: 1200, saved_tokens: 800, savings_pct: 40 },
  { date: isoDay(20), commands: 30, input_tokens: 3000, output_tokens: 1800, saved_tokens: 1200, savings_pct: 40 },
  { date: isoDay(40), commands: 40, input_tokens: 4000, output_tokens: 2400, saved_tokens: 1600, savings_pct: 40 },
  { date: isoDay(100), commands: 50, input_tokens: 5000, output_tokens: 3000, saved_tokens: 2000, savings_pct: 40 },
];
const SQZ_DAILY = [
  { date: isoDay(0), saved_tokens: 300 },
  { date: isoDay(50), saved_tokens: 700 },
];
// rtk summary 保持与日和聚合一致（口径：sum(daily)）
const RTK_SUMMARY = {
  total_commands: 150,
  total_input: 15000,
  total_output: 9000,
  total_saved: 6000,
  avg_savings_pct: 40,
};

beforeEach(() => {
  vi.clearAllMocks();
  api.status.mockResolvedValue(STATUS);
  api.setRtk.mockResolvedValue(RTK_RESULT);
  api.setSqz.mockResolvedValue({ ...STATUS, sqz_enabled: true, sqz_missing: [] });
  api.install.mockResolvedValue(STATUS);
  api.stats.mockResolvedValue({
    rtk: RTK_SUMMARY,
    sqz: { totalCompressions: 50, tokensIn: 2000, tokensOut: 1400, tokensSaved: 600, avgReduction: 30 },
    rtk_daily: RTK_DAILY,
    sqz_daily: SQZ_DAILY,
  });
});

async function openSqz() {
  const trigger = await screen.findByTestId("cc-tab-sqz");
  fireEvent.mouseDown(trigger, { button: 0 });
  fireEvent.click(trigger);
  return screen.findByTestId("sqz-toggle");
}

const sum = (rows: { saved_tokens: number }[]) => rows.reduce((a, r) => a + r.saved_tokens, 0);
const fmt = (n: number) => n.toLocaleString("en-US");

describe("ContextCompress", () => {
  it("shows rtk version and one combined switch", async () => {
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("rtk-version").getAttribute("data-version")).toBe("1.2.0"));
    // 单开关：任一侧开着即 checked；不再有双开关
    expect(screen.getByTestId("rtk-toggle").getAttribute("data-state")).toBe("checked");
    expect(screen.queryByTestId("rtk-toggle-claude-code")).toBeNull();
    expect(screen.queryByTestId("rtk-toggle-codex")).toBeNull();
    // 两侧状态照实分行显示
    expect(screen.getByTestId("rtk-side-claude-code").textContent).toContain("plugins.enabled");
    expect(screen.getByTestId("rtk-side-codex").textContent).toContain("plugins.disabled");
    expect(screen.queryByTestId("rtk-install")).toBeNull();
  });

  it("single toggle calls setRtk for both sides", async () => {
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("rtk-version").getAttribute("data-version")).toBe("1.2.0"));
    fireEvent.click(screen.getByTestId("rtk-toggle"));
    await waitFor(() => expect(api.setRtk).toHaveBeenCalledWith(false));
  });

  it("reports a per-side error without hiding the other side", async () => {
    api.setRtk.mockResolvedValue({
      status: STATUS,
      claude_error: null,
      codex_error: "codex init failed",
    });
    render(<ContextCompress />);
    fireEvent.click(await screen.findByTestId("rtk-toggle"));
    expect((await screen.findByTestId("rtk-error-codex")).textContent).toContain("codex init failed");
    expect(screen.queryByTestId("rtk-error-claude-code")).toBeNull();
    expect(screen.getByTestId("rtk-side-claude-code").textContent).toContain("plugins.enabled");
  });

  it("offers install when rtk is missing", async () => {
    api.status.mockResolvedValue({ ...STATUS, rtk_version: null, rtk_claude: false });
    render(<ContextCompress />);
    fireEvent.click(await screen.findByTestId("rtk-install"));
    await waitFor(() => expect(api.install).toHaveBeenCalledWith("rtk"));
  });

  it("sqz switch reads off with missing items and enables via setSqz", async () => {
    render(<ContextCompress />);
    const toggle = await openSqz();
    await waitFor(() => expect(toggle.getAttribute("data-state")).toBe("unchecked"));
    expect(screen.getByTestId("sqz-missing").textContent).toContain("wrap:claude:octo");
    fireEvent.click(toggle);
    await waitFor(() => expect(api.setSqz).toHaveBeenCalledWith(true));
    await waitFor(() => expect(screen.getByTestId("sqz-toggle").getAttribute("data-state")).toBe("checked"));
    expect(screen.queryByTestId("sqz-missing")).toBeNull();
  });

  it("offers sqz install when sqz-mcp is missing", async () => {
    api.status.mockResolvedValue({ ...STATUS, sqz_mcp_version: null });
    render(<ContextCompress />);
    fireEvent.mouseDown(await screen.findByTestId("cc-tab-sqz"), { button: 0 });
    fireEvent.click(screen.getByTestId("cc-tab-sqz"));
    fireEvent.click(await screen.findByTestId("sqz-install"));
    await waitFor(() => expect(api.install).toHaveBeenCalledWith("sqz"));
  });

  it("stats live in their own tab: rtk visible by default, sqz after switching", async () => {
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("cc-stats-rtk")).toBeTruthy());
    expect(screen.queryByTestId("cc-stats-sqz")).toBeNull();
    await openSqz();
    expect(await screen.findByTestId("cc-stats-sqz")).toBeTruthy();
    expect(screen.queryByTestId("cc-stats-rtk")).toBeNull();
  });

  it("rtk tiles default to 30-day range and match the daily sums", async () => {
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("cc-stat-rtk-saved").textContent).toBe(fmt(2400)));
    const in30 = RTK_DAILY.slice(0, 3);
    expect(screen.getByTestId("cc-stat-rtk-pct").textContent).toBe("40.0%");
    expect(screen.getByTestId("cc-stat-rtk-count").textContent).toBe("60");
    expect(screen.getByTestId("cc-stat-rtk-in").textContent).toBe(fmt(6000));
    expect(screen.getByTestId("cc-stat-rtk-out").textContent).toBe(fmt(3600));
    expect(in30.length).toBe(3);
  });

  it("rtk chart shows range days; range selector 90/all widens it", async () => {
    const { container } = render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("cc-chart-rtk")).toBeTruthy());
    const bars = () => container.querySelectorAll('[data-testid="cc-chart-rtk"] .recharts-bar-rectangle').length;
    await waitFor(() => expect(bars()).toBe(3));
    fireEvent.click(screen.getByTestId("cc-range-90"));
    await waitFor(() => expect(bars()).toBe(4));
    expect(screen.getByTestId("cc-stat-rtk-saved").textContent).toBe(fmt(4000));
    fireEvent.click(screen.getByTestId("cc-range-all"));
    await waitFor(() => expect(bars()).toBe(5));
    expect(screen.getByTestId("cc-stat-rtk-saved").textContent).toBe(fmt(6000));
  });

  it("rtk range pct is the weighted rate over the selected range", async () => {
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("cc-stat-rtk-saved")).toBeTruthy());
    fireEvent.click(screen.getByTestId("cc-range-90"));
    // 90 天：saved 4000 / input 10000 → 40.0%
    await waitFor(() => expect(screen.getByTestId("cc-stat-rtk-pct").textContent).toBe("40.0%"));
  });

  it("sqz tiles: saved is range-scoped; rate/count/in/out stay aggregate with note", async () => {
    render(<ContextCompress />);
    await openSqz();
    await waitFor(() => expect(screen.getByTestId("cc-stat-sqz-saved").textContent).toBe(fmt(300)));
    expect(screen.getByTestId("cc-stat-sqz-pct").textContent).toBe("30.0%");
    expect(screen.getByTestId("cc-stat-sqz-count").textContent).toBe("50");
    expect(screen.getByTestId("cc-stat-sqz-in").textContent).toBe(fmt(2000));
    expect(screen.getByTestId("cc-stat-sqz-out").textContent).toBe(fmt(1400));
    expect(screen.getByTestId("cc-sqz-rate-note")).toBeTruthy();
    // 切到全部：saved 变全量，累计口径的 tile 不变
    fireEvent.click(screen.getByTestId("cc-range-all"));
    await waitFor(() => expect(screen.getByTestId("cc-stat-sqz-saved").textContent).toBe(fmt(1000)));
    expect(screen.getByTestId("cc-stat-sqz-pct").textContent).toBe("30.0%");
  });

  it("sqz chart shows daily saved tokens", async () => {
    const { container } = render(<ContextCompress />);
    await openSqz();
    await waitFor(() =>
      expect(container.querySelectorAll('[data-testid="cc-chart-sqz"] .recharts-bar-rectangle').length).toBeGreaterThan(0),
    );
  });

  it("rtk daily tooltip shows total, saved and pct rows for the hovered day", () => {
    const { container } = render(
      <DailySavedTooltip
        active
        label="11-18"
        payload={[{ payload: { name: "11-18", saved: 400, input: 1000, pct: 40 } }]}
        withMeta
      />,
    );
    expect(container.querySelector('[data-testid="cc-tip"]')).toBeTruthy();
    const rows = [...container.querySelectorAll('[data-testid="cc-tip-row"]')];
    expect(rows.length).toBe(3);
    expect(rows[0].textContent).toContain("contextCompress.tipTotal");
    expect(rows[0].textContent).toContain(fmt(1000));
    expect(rows[1].textContent).toContain("contextCompress.tipSaved");
    expect(rows[1].textContent).toContain(fmt(400));
    expect(rows[2].textContent).toContain("contextCompress.tipPct");
    expect(rows[2].textContent).toBe("contextCompress.tipPct40.0%");
  });

  it("rtk daily tooltip hides the total row when the day has no input; pct falls back to em-dash", () => {
    const { container } = render(
      <DailySavedTooltip
        active
        label="11-18"
        payload={[{ payload: { name: "11-18", saved: 400, input: 0, pct: 0 } }]}
        withMeta
      />,
    );
    const rows = [...container.querySelectorAll('[data-testid="cc-tip-row"]')];
    expect(rows.length).toBe(2);
    expect(rows[0].textContent).toContain("contextCompress.tipSaved");
    expect(container.textContent).not.toContain("contextCompress.tipTotal");
    expect(rows[1].textContent).toBe("contextCompress.tipPct—");
  });

  it("sqz daily tooltip only shows the saved row (CLI has no daily input/pct)", () => {
    const { container } = render(
      <DailySavedTooltip
        active
        label="11-18"
        payload={[{ payload: { name: "11-18", saved: 300 } }]}
      />,
    );
    const rows = [...container.querySelectorAll('[data-testid="cc-tip-row"]')];
    expect(rows.length).toBe(1);
    expect(rows[0].textContent).toContain("contextCompress.tipSaved");
    expect(rows[0].textContent).toContain(fmt(300));
    expect(container.textContent).not.toContain("contextCompress.tipTotal");
    expect(container.textContent).not.toContain("contextCompress.tipPct");
  });

  it("inactive tooltip renders nothing", () => {
    const { container } = render(
      <DailySavedTooltip label="11-18" payload={[{ payload: { saved: 1 } }]} />,
    );
    expect(container.querySelector('[data-testid="cc-tip"]')).toBeNull();
  });

  it("slow stats do not hold back status; stats show a loading state", async () => {
    api.stats.mockReturnValue(new Promise(() => {}));
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("rtk-version").getAttribute("data-version")).toBe("1.2.0"));
    expect(screen.getByTestId("cc-stats-rtk").getAttribute("data-state")).toBe("loading");
  });

  it("does not claim 'not installed' while status is still loading", async () => {
    api.status.mockReturnValue(new Promise(() => {}));
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("cc-stats-rtk").getAttribute("data-state")).toBe("ok"));
    expect(screen.getByTestId("rtk-version").getAttribute("data-state")).toBe("loading");
    expect(screen.queryByTestId("rtk-install")).toBeNull();
  });

  it("explains missing stats: installed but no data vs not installed", async () => {
    api.status.mockResolvedValue({ ...STATUS, sqz_version: null, sqz_mcp_version: null });
    api.stats.mockResolvedValue({ rtk: null, sqz: null, rtk_daily: [], sqz_daily: [] });
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("cc-stats-rtk").getAttribute("data-state")).toBe("empty"));
    expect(screen.getByTestId("cc-stats-rtk-msg").textContent).toBe("contextCompress.statsEmpty");
    const trigger = screen.getByTestId("cc-tab-sqz");
    fireEvent.mouseDown(trigger, { button: 0 });
    fireEvent.click(trigger);
    await waitFor(() => expect(screen.getByTestId("cc-stats-sqz").getAttribute("data-state")).toBe("not-installed"));
    expect(screen.getByTestId("cc-stats-sqz-msg").textContent).toBe("contextCompress.statsNotInstalled");
  });

  it("shows the error text when the stats call fails, and refresh retries", async () => {
    api.stats.mockRejectedValueOnce("boom");
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("cc-stats-rtk").getAttribute("data-state")).toBe("error"));
    expect(screen.getByTestId("cc-stats-rtk-msg").textContent).toContain("boom");
    fireEvent.click(screen.getByTestId("cc-stats-refresh"));
    await waitFor(() => expect(screen.getByTestId("cc-stat-rtk-saved").textContent).toBe(fmt(2400)));
    expect(api.stats).toHaveBeenCalledTimes(2);
  });
});
