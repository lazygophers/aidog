import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";

const api = vi.hoisted(() => ({
  claudeList: vi.fn(),
  claudeSet: vi.fn(),
  claudeProbe: vi.fn(),
  claudeInstall: vi.fn(),
  claudeSetAgents: vi.fn(),
  claudeRemove: vi.fn(),
  claudeUpdate: vi.fn(),
  codexList: vi.fn(),
  codexAdd: vi.fn(),
  codexRemove: vi.fn(),
  codexMarketplaceAdd: vi.fn(),
  codexMarketplaceRemove: vi.fn(),
}));
vi.mock("../services/api/plugins", () => ({ pluginsApi: api }));

import { Plugins } from "./Plugins";

const CLAUDE_LIST = [
  { name: "code-review", source: "/plugins/code-review", pick: null, agents: ["claude-code", "codex"], on: true, skills: ["review"], subagents: ["reviewer"], hooks: [], servers: [], left: [] },
  { name: "z-docs", source: "/plugins/z", pick: null, agents: ["claude-code"], on: false, skills: [], subagents: [], hooks: [], servers: [], left: [{ code: "commands", args: {} }] },
];
const CLAUDE_VIEW = { plugins: CLAUDE_LIST, warnings: [] };
const CODEX_VIEW = {
  installed: [{ pluginId: "hello@testmkt", name: "hello", marketplaceName: "testmkt", version: "0.1.0", installed: true, enabled: true }],
  marketplaces: [{ name: "testmkt", source: "/mkt", root: "/mkt" }],
};

beforeEach(() => {
  vi.clearAllMocks();
  api.claudeList.mockResolvedValue(CLAUDE_VIEW);
  api.claudeSet.mockImplementation((name: string, on: boolean) =>
    Promise.resolve({ plugins: CLAUDE_LIST.map((p) => (p.name === name ? { ...p, on } : p)), warnings: [] }));
  api.claudeProbe.mockResolvedValue([{ name: "demo", description: "demo", version: "1.0", rel: "", installed: false }]);
  api.claudeInstall.mockResolvedValue(CLAUDE_VIEW);
  api.claudeSetAgents.mockResolvedValue(CLAUDE_VIEW);
  api.claudeRemove.mockResolvedValue(CLAUDE_VIEW);
  api.claudeUpdate.mockResolvedValue(CLAUDE_VIEW);
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
  it("lists Claude bundles and toggles distribution", async () => {
    render(<Plugins />);
    await screen.findByTestId("plugin-row-code-review");
    expect(screen.getByTestId("plugin-toggle-code-review").getAttribute("data-state")).toBe("checked");
    expect(screen.getByTestId("plugin-toggle-z-docs").getAttribute("data-state")).not.toBe("checked");

    fireEvent.click(screen.getByTestId("plugin-toggle-z-docs"));
    await waitFor(() => expect(api.claudeSet).toHaveBeenCalledWith("z-docs", true));
  });

  it("translates backend warnings by code", async () => {
    api.claudeSet.mockResolvedValue({ plugins: CLAUDE_LIST, warnings: [{ code: "skill_modified", args: { name: "review", agent: "codex" } }] });
    render(<Plugins />);
    fireEvent.click(await screen.findByTestId("plugin-toggle-z-docs"));
    await waitFor(() => expect(screen.queryByTestId("plugins-warnings")?.textContent).toContain("plugins.note.skill_modified"));
  });

  it("probes and installs selected Claude bundle", async () => {
    render(<Plugins />);
    expect(await screen.findByLabelText("plugins.claudeSource")).toBeTruthy();
    fireEvent.change(screen.getByTestId("claude-plugin-source"), { target: { value: "/plugins/demo" } });
    fireEvent.click(screen.getByTestId("claude-plugin-probe"));
    await waitFor(() => expect(api.claudeProbe).toHaveBeenCalledWith("/plugins/demo"));
    expect(await screen.findByTestId("claude-plugin-candidate-demo")).toBeTruthy();
    fireEvent.click(screen.getByTestId("claude-plugin-install"));
    await waitFor(() => expect(api.claudeInstall).toHaveBeenCalledWith("/plugins/demo", ["claude-code", "codex"], ["demo"]));
  });

  it("shows left-out parts and removes or updates a bundle", async () => {
    render(<Plugins />);
    await screen.findByTestId("plugin-row-z-docs");
    expect(screen.getByTestId("plugin-left-z-docs").textContent).toContain("plugins.note.commands");
    fireEvent.click(screen.getByTestId("plugin-update-z-docs"));
    await waitFor(() => expect(api.claudeUpdate).toHaveBeenCalledWith("z-docs"));
    fireEvent.click(screen.getByTestId("plugin-remove-z-docs"));
    await waitFor(() => expect(api.claudeRemove).toHaveBeenCalledWith("z-docs"));
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
