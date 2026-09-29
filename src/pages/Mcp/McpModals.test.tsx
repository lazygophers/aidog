// 票 09：添加弹窗推荐区 + 预填流行为测试（文本/交互断言，禁快照）。
// 测试 i18n 空 resources → t 返回 key 本身，按 key 断言（同 PlatformCard.test 惯例）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "../../test/render";
import { McpModals } from "./McpModals";
import { McpView } from "./McpView";
import { useMcpData } from "./useMcpData";
import type { McpRecommendedEntry, McpServerInfo } from "../../services/api";
// mockIPC 注入 window.__TAURI_INTERNALS__ → transport 分流走 Tauri IPC 路径被拦截
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";

const REC: McpRecommendedEntry[] = [
  {
    name: "playwright", transport: "stdio", command: "npx", args: ["@playwright/mcp@latest"],
    env: {}, url: "", headers: {}, display_name: "Playwright",
    description: { "zh-Hans": "自然语言驱动真实浏览器", "en-US": "Drive a real browser" },
    category: "browser", icon: "playwright", docs_url: "https://github.com/microsoft/playwright-mcp",
    homepage_url: "https://playwright.dev", required_env_keys: [],
  },
  {
    name: "brave-search", transport: "stdio", command: "npx", args: ["@brave/brave-search-mcp"],
    env: { BRAVE_API_KEY: "" }, url: "", headers: {}, display_name: "Brave Search",
    description: { "zh-Hans": "Brave 网页搜索" }, category: "search", icon: "brave",
    docs_url: "https://brave.com/search/api/", homepage_url: "", required_env_keys: ["BRAVE_API_KEY"],
  },
  {
    name: "deepwiki", transport: "http", command: "", args: [], env: {}, url: "https://mcp.deepwiki.com/mcp",
    headers: {}, display_name: "DeepWiki", description: { "zh-Hans": "AI 仓库 wiki 问答" },
    category: "docs", icon: "", docs_url: "https://deepwiki.com", homepage_url: "", required_env_keys: [],
  },
];

const SERVER = (name: string): McpServerInfo => ({
  id: 1, name, transport: "stdio", command: "npx", args: [], env: {}, url: "",
  headers: {}, enabledAgents: [], createdAt: 0, updatedAt: 0,
});

function Page() {
  const d = useMcpData();
  return (
    <>
      <McpView d={d} />
      <McpModals d={d} />
    </>
  );
}

async function openAdd(installed: string[] = []) {
  mockIPC((cmd: string) => {
    if (cmd === "mcp_list") return installed.map(SERVER);
    if (cmd === "mcp_recommended_list") return REC;
    return null;
  });
  render(<Page />);
  fireEvent.click(screen.getByText("mcp.add"));
  await screen.findByText("Playwright");
}

describe("McpModals 推荐区（票 09）", () => {
  beforeEach(() => {
    clearMocks();
  });

  it("默认推荐 tab：分组 + 卡片（名称/描述/命令行摘要），可切手动配置", async () => {
    await openAdd();
    // 分组名（i18n key）
    expect(screen.getByText("mcp.category.browser")).toBeTruthy();
    expect(screen.getByText("mcp.category.search")).toBeTruthy();
    // 卡片内容
    expect(screen.getByText("Playwright")).toBeTruthy();
    expect(screen.getByText("自然语言驱动真实浏览器")).toBeTruthy();
    expect(screen.getByText("npx @playwright/mcp@latest")).toBeTruthy();
    // icon 空串（deepwiki）走首字母 fallback
    expect(screen.getByText("D")).toBeTruthy();
    // 切手动 tab → 表单出现、推荐卡消失
    fireEvent.click(screen.getByRole("tab", { name: "mcp.manualTab" }));
    expect(screen.getByText("mcp.field.name")).toBeTruthy();
    expect(screen.queryByText("Playwright")).toBeNull();
    // 切回推荐
    fireEvent.click(screen.getByRole("tab", { name: "mcp.recTab" }));
    expect(screen.getByText("Playwright")).toBeTruthy();
  });

  it("已装项置灰点不动：显示已安装，点击不进表单", async () => {
    await openAdd(["playwright"]);
    const card = screen.getByText("Playwright");
    expect(screen.getByText("mcp.installed")).toBeTruthy();
    fireEvent.click(card);
    // 仍停在推荐 tab，表单未出现
    expect(screen.queryByText("mcp.field.name")).toBeNull();
    expect(screen.getByText("Playwright")).toBeTruthy();
  });

  it("点未装 stdio 卡片 → 手动表单预填七字段（env 键预填值空）+ 必填 key 橙字提示", async () => {
    await openAdd();
    fireEvent.click(screen.getByText("Brave Search"));
    // 切到手动表单，字段预填
    expect((screen.getByDisplayValue("brave-search") as HTMLInputElement).value).toBe("brave-search");
    expect((screen.getByDisplayValue("npx") as HTMLInputElement).value).toBe("npx");
    expect((screen.getByDisplayValue("@brave/brave-search-mcp") as HTMLTextAreaElement).value).toBe(
      "@brave/brave-search-mcp",
    );
    // env 键预填、值留空
    expect((screen.getByDisplayValue("BRAVE_API_KEY") as HTMLInputElement).value).toBe("BRAVE_API_KEY");
    // 必填提示 + docs_url 链接
    expect(screen.getByText("mcp.requiredKeyHint")).toBeTruthy();
    expect(screen.getByText("https://brave.com/search/api/")).toBeTruthy();
    // stdio 不出 Codex 提示
    expect(screen.queryByText("mcp.codexStdioOnlyHint")).toBeNull();
  });

  it("点未装 http 卡片 → url 预填 + Codex 仅 stdio 提示", async () => {
    await openAdd();
    fireEvent.click(screen.getByText("DeepWiki"));
    expect((screen.getByDisplayValue("https://mcp.deepwiki.com/mcp") as HTMLInputElement).value).toBe(
      "https://mcp.deepwiki.com/mcp",
    );
    expect(screen.getByText("mcp.codexStdioOnlyHint")).toBeTruthy();
  });

  it("保存走 mcp_add 路径并关闭弹窗", async () => {
    await openAdd();
    fireEvent.click(screen.getByText("DeepWiki"));
    mockIPC((cmd: string, args?: unknown) => {
      if (cmd === "mcp_add") {
        expect((args as { payload: { name: string; transport: string; url: string } }).payload).toMatchObject({
          name: "deepwiki",
          transport: "http",
          url: "https://mcp.deepwiki.com/mcp",
        });
        return SERVER("deepwiki");
      }
      if (cmd === "mcp_list") return [];
      return null;
    });
    fireEvent.click(screen.getByText("mcp.save"));
    await waitFor(() => {
      expect(screen.queryByText("mcp.save")).toBeNull();
    });
  });
});
