import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";

const api = vi.hoisted(() => ({
  claudeList: vi.fn(),
  claudeSet: vi.fn(),
  codexList: vi.fn(),
  codexAdd: vi.fn(),
  codexRemove: vi.fn(),
  codexMarketplaceAdd: vi.fn(),
  codexMarketplaceRemove: vi.fn(),
}));
vi.mock("../services/api/plugins", () => ({ pluginsApi: api }));

import { Plugins } from "./Plugins";

const CLAUDE_LIST = [
  { id: "code-review@official", version: "1.0.1", installPath: "/cache/cr", lastUpdated: "2026-10-28T00:38:38.373Z", enabled: true, configured: true },
  { id: "z-docs@official", version: "", installPath: "/cache/z", lastUpdated: "", enabled: false, configured: true },
];
const CODEX_VIEW = {
  installed: [{ pluginId: "hello@testmkt", name: "hello", marketplaceName: "testmkt", version: "0.1.0", installed: true, enabled: true }],
  marketplaces: [{ name: "testmkt", source: "/mkt", root: "/mkt" }],
};

beforeEach(() => {
  vi.clearAllMocks();
  api.claudeList.mockResolvedValue(CLAUDE_LIST);
  api.claudeSet.mockImplementation((id: string, enabled: boolean) =>
    Promise.resolve(CLAUDE_LIST.map((p) => (p.id === id ? { ...p, enabled } : p))));
  api.codexList.mockResolvedValue(CODEX_VIEW);
  api.codexAdd.mockResolvedValue(CODEX_VIEW);
  api.codexRemove.mockResolvedValue(CODEX_VIEW);
  api.codexMarketplaceAdd.mockResolvedValue(CODEX_VIEW);
  api.codexMarketplaceRemove.mockResolvedValue(CODEX_VIEW);
});

const switchTab = async (testid: string) => {
  const trigger = await screen.findByTestId(testid);
  fireEvent.mouseDown(trigger, { button: 0 });
  fireEvent.click(trigger);
};

describe("Plugins", () => {
  it("lists Claude Code plugins and toggles enabledPlugins", async () => {
    render(<Plugins />);
    await screen.findByTestId("plugin-row-code-review@official");
    expect(screen.getByTestId("plugin-toggle-code-review@official").getAttribute("data-state")).toBe("checked");
    expect(screen.getByTestId("plugin-toggle-z-docs@official").getAttribute("data-state")).not.toBe("checked");

    fireEvent.click(screen.getByTestId("plugin-toggle-z-docs@official"));
    await waitFor(() => expect(api.claudeSet).toHaveBeenCalledWith("z-docs@official", true));
  });

  it("adds and removes Codex plugins", async () => {
    render(<Plugins />);
    await switchTab("plugins-tab-codex");
    await screen.findByTestId("codex-plugin-row-hello@testmkt");

    fireEvent.click(screen.getByTestId("codex-plugin-remove-hello@testmkt"));
    await waitFor(() => expect(api.codexRemove).toHaveBeenCalledWith("hello@testmkt"));

    fireEvent.change(screen.getByTestId("codex-plugin-add-input"), { target: { value: "new@mk" } });
    fireEvent.click(screen.getByTestId("codex-plugin-add"));
    await waitFor(() => expect(api.codexAdd).toHaveBeenCalledWith("new@mk"));
    expect((screen.getByTestId("codex-plugin-add-input") as HTMLInputElement).value).toBe("");
  });

  it("adds and removes Codex marketplaces", async () => {
    render(<Plugins />);
    await switchTab("plugins-tab-codex");
    await screen.findByTestId("codex-mkt-row-testmkt");

    fireEvent.change(screen.getByTestId("codex-mkt-add-input"), { target: { value: "/tmp/mkt2" } });
    fireEvent.click(screen.getByTestId("codex-mkt-add"));
    await waitFor(() => expect(api.codexMarketplaceAdd).toHaveBeenCalledWith("/tmp/mkt2"));

    fireEvent.click(screen.getByTestId("codex-mkt-remove-testmkt"));
    await waitFor(() => expect(api.codexMarketplaceRemove).toHaveBeenCalledWith("testmkt"));
  });
});
