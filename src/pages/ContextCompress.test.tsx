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

beforeEach(() => {
  vi.clearAllMocks();
  api.status.mockResolvedValue(STATUS);
  api.setRtk.mockResolvedValue(STATUS);
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
  it("shows rtk version and per-agent switch state", async () => {
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("rtk-version").getAttribute("data-version")).toBe("1.2.0"));
    expect(screen.getByTestId("rtk-toggle-claude-code").getAttribute("data-state")).toBe("checked");
    expect(screen.getByTestId("rtk-toggle-codex").getAttribute("data-state")).toBe("unchecked");
    expect(screen.queryByTestId("rtk-install")).toBeNull();
  });

  it("toggling codex calls setRtk", async () => {
    render(<ContextCompress />);
    await waitFor(() => expect(screen.getByTestId("rtk-version").getAttribute("data-version")).toBe("1.2.0"));
    fireEvent.click(screen.getByTestId("rtk-toggle-codex"));
    await waitFor(() => expect(api.setRtk).toHaveBeenCalledWith("codex", true));
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

  it("shows rtk and sqz compression statistics", async () => {
    render(<ContextCompress />);
    const rtk = await screen.findByTestId("rtk-stats");
    // 隔离 i18n 缺译文回传 key 本身（见 test/render.tsx），断言 key。
    expect(rtk.textContent).toBe("contextCompress.rtkStats");
    fireEvent.mouseDown(screen.getByTestId("cc-tab-sqz"), { button: 0 });
    fireEvent.click(screen.getByTestId("cc-tab-sqz"));
    const sqz = await screen.findByTestId("sqz-stats");
    expect(sqz.textContent).toBe("contextCompress.sqzStats");
    expect(api.stats).toHaveBeenCalledOnce();
  });
});
