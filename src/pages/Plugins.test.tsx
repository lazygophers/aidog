import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "../test/render";

const api = vi.hoisted(() => ({
  claudeList: vi.fn(),
  claudeSet: vi.fn(),
  listen: vi.fn(),
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
vi.mock("../services/transport", () => ({ listen: api.listen }));

let onPluginsUpdated: ((event: { payload: unknown }) => void) | undefined;

import { Plugins } from "./Plugins";

const CLAUDE_LIST = [
  { name: "code-review", source: "/plugins/code-review", scope: null, projectPath: null, canOperate: true, operationError: null, pick: null, agents: ["claude-code", "codex"], on: true, skills: ["review"], subagents: ["reviewer"], hooks: [], servers: [], left: [], external: false },
  { name: "z-docs", source: "/plugins/z", scope: null, projectPath: null, canOperate: true, operationError: null, pick: null, agents: ["claude-code"], on: false, skills: [], subagents: [], hooks: [], servers: [], left: [{ code: "commands", args: {} }], external: false },
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
  onPluginsUpdated = undefined;
  api.listen.mockImplementation((_event: string, handler: (event: { payload: unknown }) => void) => {
    onPluginsUpdated = handler;
    return Promise.resolve(() => {});
  });
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

  it("refreshes the Claude list on demand", async () => {
    const external = { name: "new@m", source: "/x/new", scope: "user" as const, projectPath: null, canOperate: true, operationError: null, pick: null, agents: ["claude-code"], on: true, skills: [], subagents: [], hooks: [], servers: [], left: [], external: true };
    api.claudeList
      .mockResolvedValueOnce(CLAUDE_VIEW)
      .mockResolvedValueOnce({ plugins: [...CLAUDE_LIST, external], warnings: [] });
    render(<Plugins />);
    await screen.findByTestId("plugin-row-code-review");
    fireEvent.click(screen.getByTestId("plugins-refresh"));
    await screen.findByTestId("plugin-row-new@m-user");
    expect(api.claudeList).toHaveBeenCalledTimes(2);
  });

  it("automatically refreshes when Claude config files change", async () => {
    const external = { name: "external@m", source: "/x/external", scope: "user" as const, projectPath: null, canOperate: true, operationError: null, pick: null, agents: ["claude-code"], on: true, skills: [], subagents: [], hooks: [], servers: [], left: [], external: true };
    api.claudeList
      .mockResolvedValueOnce(CLAUDE_VIEW)
      .mockResolvedValueOnce({ plugins: [...CLAUDE_LIST, external], warnings: [] });
    render(<Plugins />);
    await screen.findByTestId("plugin-row-code-review");
    await waitFor(() => expect(onPluginsUpdated).toBeDefined());
    await act(async () => onPluginsUpdated?.({ payload: null }));
    await screen.findByTestId("plugin-row-external@m-user");
    expect(api.claudeList).toHaveBeenCalledTimes(2);
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

  it("enables and updates native Claude plugins through the Claude CLI", async () => {
    const external = { name: "ponytail@ponytail", source: "/x/p", scope: "user" as const, projectPath: null, canOperate: true, operationError: null, pick: null, agents: ["claude-code"], on: true, skills: [], subagents: [], hooks: [], servers: [], left: [], external: true };
    api.claudeList.mockResolvedValue({ plugins: [...CLAUDE_LIST, external], warnings: [] });
    api.claudeSet.mockImplementation((name: string, on: boolean) =>
      Promise.resolve({ plugins: [...CLAUDE_LIST, external].map((p) => (p.name === name ? { ...p, on } : p)), warnings: [] }));
    render(<Plugins />);
    await screen.findByTestId("plugin-row-ponytail@ponytail-user");
    expect(screen.getByTestId("plugin-origin-ponytail@ponytail-user").textContent).toContain("plugins.originClaude");
    expect(screen.getByTestId("plugin-scope-ponytail@ponytail-user").textContent).toBe("user");
    expect((screen.getByTestId("plugin-toggle-ponytail@ponytail-user") as HTMLButtonElement).disabled).toBe(false);
    expect((screen.getByTestId("plugin-remove-ponytail@ponytail-user") as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByTestId("plugin-agent-ponytail@ponytail-user-claude-code")).toBeNull();

    fireEvent.click(screen.getByTestId("plugin-toggle-ponytail@ponytail-user"));
    await waitFor(() => expect(api.claudeSet).toHaveBeenCalledWith("ponytail@ponytail", false));
    fireEvent.click(screen.getByTestId("plugin-update-ponytail@ponytail-user"));
    await waitFor(() => expect(api.claudeUpdate).toHaveBeenCalledWith("ponytail@ponytail"));
  });

  it("disables native plugin operations when the project is unavailable", async () => {
    const external = { name: "far@m", source: "/x/f", scope: "project" as const, projectPath: "/gone/repo", canOperate: false, operationError: "plugins.nativeProjectUnavailable", pick: null, agents: ["claude-code"], on: false, skills: [], subagents: [], hooks: [], servers: [], left: [], external: true };
    api.claudeList.mockResolvedValue({ plugins: [...CLAUDE_LIST, external], warnings: [] });
    render(<Plugins />);
    const row = await screen.findByTestId(`plugin-row-far@m-project-${encodeURIComponent("/gone/repo")}`);
    expect(row.textContent).toContain("plugins.nativeProjectUnavailable");
    expect((screen.getByTestId(`plugin-toggle-far@m-project-${encodeURIComponent("/gone/repo")}`) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId(`plugin-update-far@m-project-${encodeURIComponent("/gone/repo")}`) as HTMLButtonElement).disabled).toBe(true);
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
