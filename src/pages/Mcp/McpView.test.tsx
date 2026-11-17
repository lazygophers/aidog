// MCP 主列表就近测试：插件分发的 server 行内标出所属插件。mockIPC 拦截 transport 的 Tauri IPC 路径。
import { beforeEach, describe, expect, it } from "vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { render, screen, waitFor } from "../../test/render";
import { McpView } from "./McpView";
import { useMcpData } from "./useMcpData";
import type { McpServerInfo } from "../../services/api";

const SERVER = (name: string): McpServerInfo => ({
  id: 1, name, transport: "stdio", command: "node", args: [], env: {}, url: "",
  headers: {}, enabledAgents: [], createdAt: 0, updatedAt: 0,
});

function Page() {
  return <McpView d={useMcpData()} />;
}

describe("McpView plugin origin", () => {
  beforeEach(() => clearMocks());

  it("marks servers that came with a plugin", async () => {
    mockIPC((cmd: string) => {
      if (cmd === "mcp_list") return [SERVER("docs"), SERVER("mine")];
      if (cmd === "plugins_bundle_list") {
        return { plugins: [{ name: "demo", source: "/x", pick: null, agents: [], on: true, skills: [], subagents: [], hooks: [], servers: ["docs"], left: [] }], warnings: [] };
      }
      return null;
    });
    render(<Page />);
    await screen.findByText("mine");
    await waitFor(() => expect(screen.queryByTestId("mcp-plugin-docs")).not.toBeNull());
    expect(screen.queryByTestId("mcp-plugin-mine")).toBeNull();
  });
});
