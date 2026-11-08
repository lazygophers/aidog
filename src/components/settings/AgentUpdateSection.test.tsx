// AgentUpdateSection 行为测试：渲染真组件 + data-testid 断言（frontend-refactor-tests 规则）。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../../test/render";

const h = vi.hoisted(() => ({
  checkAgentUpdates: vi.fn(),
  applyAgentUpdate: vi.fn(),
}));
vi.mock("../../services/transport", () => ({ invoke: vi.fn(), listen: vi.fn(async () => () => {}) }));
vi.mock("../../services/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../services/api")>();
  return { ...actual, cliEnvApi: h };
});

import { AgentUpdateSection } from "./AgentUpdateSection";

const statuses = [
  {
    tool: "claude",
    installed: true,
    version: "2.1.100",
    latest_version: "2.1.200",
    has_update: true,
    source: "homebrew",
    updatable: true,
    manual_hint: null,
  },
  {
    tool: "codex",
    installed: true,
    version: "1.0.0",
    latest_version: "1.1.0",
    has_update: true,
    source: "npm-global",
    updatable: true,
    manual_hint: null,
  },
  {
    tool: "pi",
    installed: true,
    version: "0.9.0",
    latest_version: "1.0.0",
    has_update: true,
    source: "native",
    updatable: false,
    manual_hint: "manual: unsupported install source",
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  h.checkAgentUpdates.mockResolvedValue(statuses);
  h.applyAgentUpdate.mockResolvedValue({
    tool: "claude",
    source: "homebrew",
    before_version: "2.1.100",
    after_version: "2.1.200",
    success: true,
    message: "updated",
  });
});

describe("AgentUpdateSection", () => {
  it("渲染三行版本对照（当前 → 最新 + 来源）", async () => {
    render(<AgentUpdateSection />);
    expect(await screen.findByTestId("agent-update-row-claude")).toHaveAttribute("data-updatable", "true");
    expect(screen.getByTestId("agent-update-row-pi")).toHaveAttribute("data-updatable", "false");
  });

  it("手动来源不显示更新按钮，显示手动提示", async () => {
    render(<AgentUpdateSection />);
    await screen.findByTestId("agent-update-row-pi");
    expect(screen.queryByTestId("agent-update-btn-pi")).not.toBeInTheDocument();
    expect(screen.getByTestId("agent-update-hint-pi")).toBeInTheDocument();
  });

  it("更新走确认对话框，确认后按 tool 调 apply 并回显前后版本", async () => {
    render(<AgentUpdateSection />);
    await screen.findByTestId("agent-update-row-claude");
    fireEvent.click(screen.getByTestId("agent-update-btn-claude"));
    expect(screen.getByTestId("agent-update-confirm")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("agent-update-confirm"));
    await waitFor(() => expect(h.applyAgentUpdate).toHaveBeenCalledWith("claude"));
    await waitFor(() => expect(screen.getByTestId("agent-update-result-claude")).toBeInTheDocument());
    await waitFor(() => expect(h.checkAgentUpdates).toHaveBeenCalledTimes(2));
  });

  it("apply 失败回显错误", async () => {
    h.applyAgentUpdate.mockRejectedValueOnce("exit 1: network down");
    render(<AgentUpdateSection />);
    await screen.findByTestId("agent-update-row-codex");
    fireEvent.click(screen.getByTestId("agent-update-btn-codex"));
    fireEvent.click(screen.getByTestId("agent-update-confirm"));
    await waitFor(() =>
      expect(screen.getByTestId("agent-update-result-codex")).toHaveTextContent("network down"),
    );
  });
});
