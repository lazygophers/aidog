// Subagents 页就近测试：渲染真组件，mock api，断言走 data-testid / data-state。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../test/render";

const api = vi.hoisted(() => ({ list: vi.fn(), save: vi.fn(), delete: vi.fn(), import: vi.fn(), preview: vi.fn() }));
vi.mock("../services/api/subagents", () => ({ subagentsApi: api }));
const skillsApi = vi.hoisted(() => ({ listInstalled: vi.fn() }));
vi.mock("../services/api/skills", () => ({ skillsApi }));
const mcpApi = vi.hoisted(() => ({ list: vi.fn() }));
vi.mock("../services/api/mcp", () => ({ mcpApi }));

import { Subagents } from "./Subagents";

const ROW = {
  name: "reviewer",
  description: "Reviews code",
  prompt: "p",
  agents: ["codex"],
  skills: ["code-review"],
  mcp_servers: ["github"],
  status: { "claude-code": "modified", codex: "on" },
  capabilities: { "claude-code": ["skills", "mcpServers"], codex: ["model"] },
};

beforeEach(() => {
  vi.clearAllMocks();
  api.list.mockResolvedValue([ROW]);
  skillsApi.listInstalled.mockResolvedValue({ stale: false, items: [{ name: "alpha" }, { name: "beta" }] });
  mcpApi.list.mockResolvedValue([{ name: "github" }, { name: "linear" }]);
  api.save.mockResolvedValue([ROW]);
  api.import.mockResolvedValue({ added: ["a"], updated: [], have: [], edited: [], conflicts: [] });
});

describe("Subagents", () => {
  it("lists entries with per-agent switches and file status", async () => {
    render(<Subagents />);
    expect(await screen.findByTestId("subagent-row-reviewer")).toBeTruthy();
    expect(screen.getByTestId("subagent-toggle-reviewer-codex").getAttribute("data-state")).toBe("checked");
    expect(screen.getByTestId("subagent-toggle-reviewer-claude-code").getAttribute("data-state")).toBe("unchecked");
    expect(screen.getByTestId("subagent-status-reviewer-claude-code").textContent).toBe("subagents.status.modified");
    expect(screen.queryByTestId("subagent-status-reviewer-codex")).toBeNull();
  });

  it("toggling an agent saves with the agent added", async () => {
    render(<Subagents />);
    fireEvent.click(await screen.findByTestId("subagent-toggle-reviewer-claude-code"));
    await waitFor(() => expect(api.save).toHaveBeenCalledTimes(1));
    const [oldName, sub] = api.save.mock.calls[0];
    expect(oldName).toBe("reviewer");
    expect(sub.agents).toEqual(["codex", "claude-code"]);
  });

  it("creates a new subagent from the dialog", async () => {
    render(<Subagents />);
    fireEvent.click(await screen.findByTestId("subagent-add"));
    fireEvent.change(screen.getByTestId("subagent-edit-name"), { target: { value: "helper" } });
    fireEvent.change(screen.getByTestId("subagent-edit-description"), { target: { value: "d" } });
    fireEvent.change(screen.getByTestId("subagent-edit-prompt"), { target: { value: "body" } });
    fireEvent.click(screen.getByTestId("subagent-edit-save"));
    await waitFor(() => expect(api.save).toHaveBeenCalledTimes(1));
    const [oldName, sub] = api.save.mock.calls[0];
    expect(oldName).toBeNull();
    expect(sub).toMatchObject({ name: "helper", description: "d", prompt: "body" });
  });

  it("shows capability badges per agent with unsupported ones absent", async () => {
    render(<Subagents />);
    await screen.findByTestId("subagent-row-reviewer");
    // claude-code 支持 skills/mcpServers，codex 只支持 model
    expect(screen.getByTestId("subagent-cap-reviewer-codex-model")).toBeTruthy();
    expect(screen.getByTestId("subagent-cap-reviewer-codex-skills").className).toContain("opacity-40");
    expect(screen.getByTestId("subagent-cap-reviewer-codex-mcpServers").className).toContain("opacity-40");
    // 绑定徽标：claude-code 行显 skills/mcpServers 数量
    expect(screen.getByTestId("subagent-binding-reviewer-claude-code-skills")).toBeTruthy();
    expect(screen.getByTestId("subagent-binding-reviewer-claude-code-mcpServers")).toBeTruthy();
  });

  it("edit dialog binds skills and mcp servers via multiselect", async () => {
    render(<Subagents />);
    fireEvent.click(await screen.findByTestId("subagent-add"));
    fireEvent.change(screen.getByTestId("subagent-edit-name"), { target: { value: "b" } });
    fireEvent.change(screen.getByTestId("subagent-edit-description"), { target: { value: "d" } });
    // 候选来自库内清单
    await waitFor(() => expect(screen.getByTestId("subagent-skill-opt-alpha")).toBeTruthy());
    fireEvent.click(screen.getByTestId("subagent-skill-opt-alpha"));
    await waitFor(() => expect(screen.getByTestId("subagent-mcp-opt-github")).toBeTruthy());
    fireEvent.click(screen.getByTestId("subagent-mcp-opt-github"));
    fireEvent.click(screen.getByTestId("subagent-edit-save"));
    await waitFor(() => expect(api.save).toHaveBeenCalledTimes(1));
    const [, sub] = api.save.mock.calls[0];
    expect(sub.skills).toEqual(["alpha"]);
    expect(sub.mcp_servers).toEqual(["github"]);
  });

  it("imports from a source and shows the report", async () => {
    render(<Subagents />);
    fireEvent.click(await screen.findByTestId("subagent-import"));
    fireEvent.change(screen.getByTestId("subagent-import-source"), { target: { value: "https://x/r.git" } });
    fireEvent.click(screen.getByTestId("subagent-import-run"));
    await waitFor(() => expect(api.import).toHaveBeenCalledWith("https://x/r.git", [], undefined));
    expect(await screen.findByTestId("subagent-import-result")).toBeTruthy();
  });

  it("scan shows a candidate checklist and imports only the checked items", async () => {
    render(<Subagents />);
    fireEvent.click(await screen.findByTestId("subagent-import"));
    fireEvent.change(screen.getByTestId("subagent-import-source"), { target: { value: "/repo" } });
    api.preview.mockResolvedValue([
      { rel: "a.md", name: "alpha", description: "A" },
      { rel: "sub/b.md", name: "beta", description: "B" },
    ]);
    fireEvent.click(screen.getByTestId("subagent-import-scan"));
    await waitFor(() => expect(api.preview).toHaveBeenCalledWith("/repo"));
    expect(await screen.findByTestId("subagent-import-cand-a.md")).toBeTruthy();
    expect(screen.getByTestId("subagent-import-cand-a.md").getAttribute("data-state")).toBe("checked");
    expect(screen.getByTestId("subagent-import-cand-sub/b.md").getAttribute("data-state")).toBe("checked");
    fireEvent.click(screen.getByTestId("subagent-import-cand-sub/b.md"));
    expect(screen.getByTestId("subagent-import-cand-sub/b.md").getAttribute("data-state")).toBe("unchecked");
    fireEvent.click(screen.getByTestId("subagent-import-run"));
    await waitFor(() => expect(api.import).toHaveBeenCalledTimes(1));
    expect(api.import.mock.calls[0]).toEqual(["/repo", [], ["a.md"]]);
  });
});
