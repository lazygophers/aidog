// McpToolsSection 就近测试：渲染真组件，mock settingsApi（transport 层），
// 断言走 data-testid（frontend-refactor-tests.md）。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../../test/render";

const h = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn() }));
const m = vi.hoisted(() => ({ selfInstall: vi.fn() }));
vi.mock("../../services/transport", () => ({ invoke: vi.fn(), listen: vi.fn(async () => () => {}) }));
vi.mock("../../services/api/settings", () => ({ settingsApi: { get: h.get, set: h.set } }));
vi.mock("../../services/api/mcp", () => ({ mcpApi: { selfInstall: m.selfInstall } }));

import { McpToolsSection } from "./McpToolsSection";

const providerSelect = () => screen.getByTestId("mcp-tools-provider") as HTMLSelectElement;
const enabledBox = () => screen.getByTestId("mcp-tools-enabled") as HTMLInputElement;
const keysArea = () => screen.getByTestId("mcp-tools-keys") as HTMLTextAreaElement;
const saved = () => screen.getByTestId("mcp-tools-save");

beforeEach(() => {
  vi.clearAllMocks();
  h.get.mockResolvedValue(null);
  h.set.mockResolvedValue(undefined);
  m.selfInstall.mockReset();
});

describe("McpToolsSection", () => {
  it("未配置加载：provider 空、enabled 默认开，保存写出缺省形状", async () => {
    render(<McpToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    expect(enabledBox().checked).toBe(true);
    fireEvent.click(saved());
    await waitFor(() =>
      expect(h.set).toHaveBeenCalledWith("mcp_tools", "settings", {
        search: { provider: "", keys: [], enabled: true, searxng_base_urls: [] },
      }),
    );
  });

  it("已配置加载回填；保存保留 image/video 透传", async () => {
    h.get.mockResolvedValue({
      search: { provider: "tavily", keys: ["k1"], enabled: true, searxng_base_urls: [] },
      image: { base_url: "https://api.openai.com/v1", api_key: "img-k", model: "g" },
    });
    render(<McpToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe("tavily"));
    expect(keysArea().value).toBe("k1");
    fireEvent.click(saved());
    await waitFor(() => {
      const call = h.set.mock.calls[0];
      expect(call[0]).toBe("mcp_tools");
      expect((call[2] as any).image).toEqual({ base_url: "https://api.openai.com/v1", api_key: "img-k", model: "g" });
      expect((call[2] as any).search.provider).toBe("tavily");
    });
  });

  it("选 searxng 出多实例 URL 输入，保存按行解析", async () => {
    render(<McpToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    expect(screen.queryByTestId("mcp-tools-searxng-urls")).toBeNull();
    fireEvent.change(providerSelect(), { target: { value: "searxng" } });
    const urls = screen.getByTestId("mcp-tools-searxng-urls") as HTMLTextAreaElement;
    fireEvent.change(urls, { target: { value: " http://a \nhttp://b/\n\n" } });
    fireEvent.click(saved());
    await waitFor(() =>
      expect(h.set).toHaveBeenCalledWith("mcp_tools", "settings", {
        search: {
          provider: "searxng", keys: [], enabled: true,
          searxng_base_urls: ["http://a", "http://b/"],
        },
      }),
    );
  });

  it("key 多行解析：trim + 去空行", async () => {
    render(<McpToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    fireEvent.change(keysArea(), { target: { value: " k1 \nk2\n\nk3  " } });
    fireEvent.click(saved());
    await waitFor(() => {
      const call = h.set.mock.calls[0];
      expect((call[2] as any).search.keys).toEqual(["k1", "k2", "k3"]);
    });
  });

  it("unverified provider 带未验证标记（不猜端点，后端回显式错误）", async () => {
    render(<McpToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    // 测试环境 i18n 资源为空，回退不生效、选项文本落到原始 key——锚 key 名断言
    for (const p of ["youcom", "deepseek", "doubao", "tencent_yuanbao"]) {
      const opt = [...providerSelect().options].find((o) => o.value === p);
      expect(opt, p).toBeTruthy();
      expect(opt!.textContent).toContain("mcp.tools.unverifiedBadge");
    }
    expect([...providerSelect().options].find((o) => o.value === "tavily")!.textContent).not.toContain("mcp.tools.unverifiedBadge");
  });

  it("enabled 关闭保留在保存值里", async () => {
    h.get.mockResolvedValue({ search: { provider: "brave", keys: ["k"], enabled: false, searxng_base_urls: [] } });
    render(<McpToolsSection />);
    await waitFor(() => expect(enabledBox().checked).toBe(false));
    fireEvent.click(saved());
    await waitFor(() => {
      const call = h.set.mock.calls[0];
      expect((call[2] as any).search.enabled).toBe(false);
    });
  });

  it("加载失败展示原文", async () => {
    h.get.mockRejectedValue("db locked");
    render(<McpToolsSection />);
    expect(await screen.findByTestId("mcp-tools-load-error")).toHaveTextContent("db locked");
  });

  it("保存失败展示原文", async () => {
    h.set.mockRejectedValue("settings_set failed");
    render(<McpToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    fireEvent.click(saved());
    expect(await screen.findByTestId("mcp-tools-save-error")).toHaveTextContent("settings_set failed");
  });

  it("image 字段编辑后保存写出 image 对象；video 留空不写", async () => {
    render(<McpToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    fireEvent.change(screen.getByTestId("mcp-tools-image-base-url"), { target: { value: "https://p/v1/" } });
    fireEvent.change(screen.getByTestId("mcp-tools-image-api-key"), { target: { value: "sk" } });
    fireEvent.change(screen.getByTestId("mcp-tools-image-model"), { target: { value: "m1" } });
    fireEvent.click(saved());
    await waitFor(() => {
      const call = h.set.mock.calls[0];
      expect((call[2] as any).image).toEqual({ base_url: "https://p/v1/", api_key: "sk", model: "m1" });
      expect((call[2] as any).video).toBeUndefined();
    });
  });

  it("一键安装成功：展示结果元素（i18n 资源空时落原始 key，同 unverifiedBadge）", async () => {
    m.selfInstall.mockResolvedValue({ name: "aidog", command: "/usr/local/bin/aidog-kernel", agents: ["claude-code", "codex"] });
    render(<McpToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    fireEvent.click(screen.getByTestId("mcp-tools-install"));
    const res = await screen.findByTestId("mcp-tools-install-result");
    expect(res).toHaveTextContent("mcp.tools.installed");
    expect(m.selfInstall).toHaveBeenCalledTimes(1);
  });

  it("一键安装失败展示原文", async () => {
    m.selfInstall.mockRejectedValue("no kernel");
    render(<McpToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    fireEvent.click(screen.getByTestId("mcp-tools-install"));
    expect(await screen.findByTestId("mcp-tools-install-result")).toHaveTextContent("no kernel");
  });
});
