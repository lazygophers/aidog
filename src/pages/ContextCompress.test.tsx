// ContextCompress 页就近测试：渲染真组件，mock api，断言走 data-testid / data-state。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";

const api = vi.hoisted(() => ({ status: vi.fn(), install: vi.fn(), setRtk: vi.fn(), setSqz: vi.fn(), stats: vi.fn() }));
vi.mock("../services/api/contextCompress", () => ({ contextCompressApi: api }));

import { ContextCompress } from "./ContextCompress";

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

beforeEach(() => {
  vi.clearAllMocks();
  api.status.mockResolvedValue(STATUS);
  api.setRtk.mockResolvedValue(RTK_RESULT);
  api.setSqz.mockResolvedValue({ ...STATUS, sqz_enabled: true, sqz_missing: [] });
  api.install.mockResolvedValue(STATUS);
  api.stats.mockResolvedValue({
    rtk: { total_commands: 100, total_input: 1000, total_output: 600, total_saved: 400, avg_savings_pct: 40 },
    sqz: { totalCompressions: 50, tokensIn: 2000, tokensOut: 1400, tokensSaved: 600, avgReduction: 30 },
  });
});

async function openSqz() {
  const trigger = await screen.findByTestId("cc-tab-sqz");
  fireEvent.mouseDown(trigger, { button: 0 });
  fireEvent.click(trigger);
  return screen.findByTestId("sqz-toggle");
}

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

  it("stats panel shows rtk and sqz totals without switching tabs", async () => {
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("cc-stat-rtk-saved").textContent).toBe("400"));
    expect(screen.getByTestId("cc-stat-rtk-pct").textContent).toBe("40.0%");
    expect(screen.getByTestId("cc-stat-rtk-count").textContent).toBe("100");
    expect(screen.getByTestId("cc-stat-rtk-in").textContent).toBe("1,000");
    expect(screen.getByTestId("cc-stat-rtk-out").textContent).toBe("600");
    expect(screen.getByTestId("cc-stat-sqz-saved").textContent).toBe("600");
    expect(screen.getByTestId("cc-stat-sqz-pct").textContent).toBe("30.0%");
    expect(screen.getByTestId("cc-stat-sqz-count").textContent).toBe("50");
    expect(screen.getByTestId("cc-stat-sqz-in").textContent).toBe("2,000");
    expect(screen.getByTestId("cc-stat-sqz-out").textContent).toBe("1,400");
    expect(api.stats).toHaveBeenCalledOnce();
  });

  it("slow stats do not hold back status; stats show a loading state", async () => {
    api.stats.mockReturnValue(new Promise(() => {}));
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("rtk-version").getAttribute("data-version")).toBe("1.2.0"));
    expect(screen.getByTestId("cc-stats-rtk").getAttribute("data-state")).toBe("loading");
    expect(screen.getByTestId("cc-stats-sqz").getAttribute("data-state")).toBe("loading");
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
    api.stats.mockResolvedValue({ rtk: null, sqz: null });
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("cc-stats-rtk").getAttribute("data-state")).toBe("empty"));
    expect(screen.getByTestId("cc-stats-sqz").getAttribute("data-state")).toBe("not-installed");
    expect(screen.getByTestId("cc-stats-rtk-msg").textContent).toBe("contextCompress.statsEmpty");
    expect(screen.getByTestId("cc-stats-sqz-msg").textContent).toBe("contextCompress.statsNotInstalled");
  });

  it("shows the error text when the stats call fails, and refresh retries", async () => {
    api.stats.mockRejectedValueOnce("boom");
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("cc-stats-rtk").getAttribute("data-state")).toBe("error"));
    expect(screen.getByTestId("cc-stats-rtk-msg").textContent).toContain("boom");
    expect(screen.getByTestId("cc-stats-sqz").getAttribute("data-state")).toBe("error");
    fireEvent.click(screen.getByTestId("cc-stats-refresh"));
    await waitFor(() => expect(screen.getByTestId("cc-stat-rtk-saved").textContent).toBe("400"));
    expect(api.stats).toHaveBeenCalledTimes(2);
  });
});
