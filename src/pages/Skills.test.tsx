// Skills 页就近测试：插件分发的技能行内标出所属插件。mock api，断言走 data-testid。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "../test/render";

const skillsApi = vi.hoisted(() => ({ checkEnv: vi.fn(), listInstalled: vi.fn(), listRefresh: vi.fn() }));
vi.mock("../services/api/skills", () => ({ skillsApi }));
const pluginsApi = vi.hoisted(() => ({ claudeList: vi.fn() }));
vi.mock("../services/api/plugins", () => ({ pluginsApi }));

import { Skills } from "./Skills";

const SKILL = (name: string) => ({
  name, enabled_agents: ["claude"], scope: { kind: "global" }, installed_path: `/s/${name}`, description: null,
  source: null, source_type: null, source_url: null, skill_folder_hash: null, plugin_name: null,
  installed_at: null, updated_at: null,
});

beforeEach(() => {
  vi.clearAllMocks();
  skillsApi.checkEnv.mockResolvedValue({ npx: true, node: true });
  skillsApi.listInstalled.mockResolvedValue({ stale: false, items: [SKILL("review"), SKILL("mine")] });
  skillsApi.listRefresh.mockResolvedValue({ items: [SKILL("review"), SKILL("mine")], load_failed: false });
  pluginsApi.claudeList.mockResolvedValue({
    plugins: [{ name: "demo", source: "/x", pick: null, agents: [], on: true, skills: ["review"], subagents: [], hooks: [], servers: [], left: [] }],
    warnings: [],
  });
});

describe("Skills plugin origin", () => {
  it("marks skills that came with a plugin", async () => {
    render(<Skills />);
    await screen.findByText("mine");
    await waitFor(() => expect(screen.queryByTestId("skill-plugin-review")).not.toBeNull());
    expect(screen.queryByTestId("skill-plugin-mine")).toBeNull();
  });
});
