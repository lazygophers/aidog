// ToolsSection 就近测试：渲染真组件，mock settingsApi（transport 层），
// 断言走 data-testid（frontend-refactor-tests.md）。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../../test/render";

const h = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn() }));
const m = vi.hoisted(() => ({ selfInstall: vi.fn() }));
vi.mock("../../services/transport", () => ({ invoke: vi.fn(), listen: vi.fn(async () => () => {}) }));
vi.mock("../../services/api/settings", () => ({ settingsApi: { get: h.get, set: h.set } }));
const sx = vi.hoisted(() => ({ searxInstances: vi.fn() }));
vi.mock("../../services/api/search", () => ({ searchApi: { searxInstances: sx.searxInstances } }));
vi.mock("../../services/api/mcp", () => ({ mcpApi: { selfInstall: m.selfInstall } }));

import { SEARCH_ENGINES, ToolsSection } from "./ToolsSection";

const ALL_ON = Object.fromEntries(SEARCH_ENGINES.map((e) => [e.id, e.defaultOn]));
const engineBox = (id: string) => screen.getByTestId(`tools-engine-${id}`) as HTMLInputElement;

const providerSelect = () => screen.getByTestId("mcp-tools-provider") as HTMLSelectElement;
const enabledBox = () => screen.getByTestId("mcp-tools-enabled") as HTMLInputElement;
const keysArea = () => screen.getByTestId("mcp-tools-keys") as HTMLTextAreaElement;
const saved = () => screen.getByTestId("mcp-tools-save");

beforeEach(() => {
  vi.clearAllMocks();
  h.get.mockResolvedValue(null);
  h.set.mockResolvedValue(undefined);
  m.selfInstall.mockReset();
  sx.searxInstances.mockReset();
});

describe("ToolsSection", () => {
  it("未配置加载：provider 空、enabled 默认开，保存写出缺省形状", async () => {
    render(<ToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    expect(enabledBox().checked).toBe(true);
    fireEvent.click(saved());
    await waitFor(() =>
      expect(h.set).toHaveBeenCalledWith("mcp_tools", "settings", {
        search: { provider: "", keys: [], enabled: true, searxng_base_urls: [], engines: ALL_ON },
      }),
    );
  });

  it("已配置加载回填；保存保留 image/video 透传", async () => {
    h.get.mockResolvedValue({
      search: { provider: "tavily", keys: ["k1"], enabled: true, searxng_base_urls: [] },
      image: { base_url: "https://api.openai.com/v1", api_key: "img-k", model: "g" },
    });
    render(<ToolsSection />);
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

  it("SearXNG 实例 URL 恒显示，保存按行解析（trim + 去空行）", async () => {
    render(<ToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    const urls = screen.getByTestId("mcp-tools-searxng-urls") as HTMLTextAreaElement;
    fireEvent.change(urls, { target: { value: " http://a \nhttp://b/\n\n" } });
    fireEvent.click(saved());
    await waitFor(() =>
      expect(h.set).toHaveBeenCalledWith("mcp_tools", "settings", {
        search: {
          provider: "", keys: [], enabled: true,
          searxng_base_urls: ["http://a", "http://b/"], engines: ALL_ON,
        },
      }),
    );
  });

  it("引擎列表：默认取各引擎缺省开关；关掉 bing 后保存写出 engines.bing=false", async () => {
    render(<ToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    for (const e of SEARCH_ENGINES) expect(engineBox(e.id).checked, e.id).toBe(e.defaultOn);
    fireEvent.click(engineBox("bing"));
    expect(engineBox("bing").checked).toBe(false);
    fireEvent.click(saved());
    await waitFor(() => {
      const call = h.set.mock.calls[0];
      expect((call[2] as any).search.engines).toEqual({ ...ALL_ON, bing: false });
    });
  });

  it("旧设置（无 engines 键）加载 = 全取默认开；已存 false 回填；provider=searxng 读作未选且 URL 保留", async () => {
    h.get.mockResolvedValue({
      search: { provider: "searxng", keys: [], enabled: true, searxng_base_urls: ["http://old"] },
    });
    const { unmount } = render(<ToolsSection />);
    await waitFor(() => expect((screen.getByTestId("mcp-tools-searxng-urls") as HTMLTextAreaElement).value).toBe("http://old"));
    expect(providerSelect().value).toBe("");
    for (const e of SEARCH_ENGINES) expect(engineBox(e.id).checked, e.id).toBe(e.defaultOn);
    unmount();

    h.get.mockResolvedValue({ search: { provider: "", keys: [], enabled: true, engines: { google: false } } });
    render(<ToolsSection />);
    await waitFor(() => expect(engineBox("google").checked).toBe(false));
    expect(engineBox("ddg").checked).toBe(true);
  });

  it("key 多行解析：trim + 去空行", async () => {
    render(<ToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    fireEvent.change(keysArea(), { target: { value: " k1 \nk2\n\nk3  " } });
    fireEvent.click(saved());
    await waitFor(() => {
      const call = h.set.mock.calls[0];
      expect((call[2] as any).search.keys).toEqual(["k1", "k2", "k3"]);
    });
  });

  it("unverified provider 带未验证标记（不猜端点，后端回显式错误）", async () => {
    render(<ToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    // 测试环境 i18n 资源为空，回退不生效、选项文本落到原始 key——锚 key 名断言
    for (const p of ["youcom", "deepseek", "doubao", "tencent_yuanbao"]) {
      const opt = [...providerSelect().options].find((o) => o.value === p);
      expect(opt, p).toBeTruthy();
      expect(opt!.textContent).toContain("tools.unverifiedBadge");
    }
    expect([...providerSelect().options].find((o) => o.value === "tavily")!.textContent).not.toContain("tools.unverifiedBadge");
  });

  it("enabled 关闭保留在保存值里", async () => {
    h.get.mockResolvedValue({ search: { provider: "brave", keys: ["k"], enabled: false, searxng_base_urls: [] } });
    render(<ToolsSection />);
    await waitFor(() => expect(enabledBox().checked).toBe(false));
    fireEvent.click(saved());
    await waitFor(() => {
      const call = h.set.mock.calls[0];
      expect((call[2] as any).search.enabled).toBe(false);
    });
  });

  it("加载失败展示原文", async () => {
    h.get.mockRejectedValue("db locked");
    render(<ToolsSection />);
    expect(await screen.findByTestId("mcp-tools-load-error")).toHaveTextContent("db locked");
  });

  it("保存失败展示原文", async () => {
    h.set.mockRejectedValue("settings_set failed");
    render(<ToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    fireEvent.click(saved());
    expect(await screen.findByTestId("mcp-tools-save-error")).toHaveTextContent("settings_set failed");
  });

  it("image 字段编辑后保存写出 image 对象；video 留空不写", async () => {
    render(<ToolsSection />);
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
    m.selfInstall.mockResolvedValue({ name: "aidog", url: "http://127.0.0.1:9890/mcp", agents: ["claude-code", "codex"] });
    render(<ToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    fireEvent.click(screen.getByTestId("mcp-tools-install"));
    const res = await screen.findByTestId("mcp-tools-install-result");
    expect(res).toHaveTextContent("tools.installed");
    expect(m.selfInstall).toHaveBeenCalledTimes(1);
  });

  it("一键安装失败展示原文", async () => {
    m.selfInstall.mockRejectedValue("no kernel");
    render(<ToolsSection />);
    await waitFor(() => expect(providerSelect().value).toBe(""));
    fireEvent.click(screen.getByTestId("mcp-tools-install"));
    expect(await screen.findByTestId("mcp-tools-install-result")).toHaveTextContent("no kernel");
  });

  it("新增 7 个引擎默认关、可见；其余 7 个默认开", async () => {
    render(<ToolsSection />);
    await waitFor(() => expect(engineBox("ddg").checked).toBe(true));
    for (const id of ["ddg_lite", "yandex", "sogou", "so360", "baidu", "crossref", "pubmed"]) {
      expect(engineBox(id).checked, id).toBe(false);
    }
    for (const id of ["ddg", "bing", "google", "arxiv", "searxng", "wikipedia", "github"]) {
      expect(engineBox(id).checked, id).toBe(true);
    }
  });

  it("SearXNG 公共列表：加载 / 打开页面从不自动拉取，只有点刷新才拉一次", async () => {
    sx.searxInstances.mockResolvedValue([{ url: "https://a.example/", seconds: 0.3 }]);
    render(<ToolsSection />);
    await waitFor(() => expect(engineBox("ddg").checked).toBe(true));
    expect(sx.searxInstances).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("tools-searx-refresh"));
    await waitFor(() => expect(screen.getByTestId("tools-searx-inst-0")).toBeTruthy());
    expect(sx.searxInstances).toHaveBeenCalledTimes(1);
  });

  it("刷新后多选写入实例 URL 并随保存持久化；自填 URL 保留；取消勾选移除", async () => {
    h.get.mockResolvedValue({ search: { provider: "", keys: [], enabled: true, searxng_base_urls: ["http://mine"] } });
    sx.searxInstances.mockResolvedValue([
      { url: "https://a.example/", seconds: 0.3 },
      { url: "https://b.example/", seconds: 1.2 },
    ]);
    render(<ToolsSection />);
    await waitFor(() => expect((screen.getByTestId("mcp-tools-searxng-urls") as HTMLTextAreaElement).value).toBe("http://mine"));
    fireEvent.click(screen.getByTestId("tools-searx-refresh"));
    await waitFor(() => screen.getByTestId("tools-searx-inst-1"));
    fireEvent.click(screen.getByTestId("tools-searx-inst-0"));
    fireEvent.click(screen.getByTestId("tools-searx-inst-1"));
    expect((screen.getByTestId("mcp-tools-searxng-urls") as HTMLTextAreaElement).value).toBe("http://mine\nhttps://a.example/\nhttps://b.example/");
    fireEvent.click(screen.getByTestId("tools-searx-inst-0"));
    expect((screen.getByTestId("mcp-tools-searxng-urls") as HTMLTextAreaElement).value).toBe("http://mine\nhttps://b.example/");
    fireEvent.click(saved());
    await waitFor(() =>
      expect((h.set.mock.calls[0][2] as any).search.searxng_base_urls).toEqual(["http://mine", "https://b.example/"]),
    );
  });

  it("已保存的实例在刷新列表里显示为已勾选；刷新失败显示错误", async () => {
    h.get.mockResolvedValue({ search: { provider: "", keys: [], enabled: true, searxng_base_urls: ["https://a.example/"] } });
    sx.searxInstances.mockResolvedValueOnce([{ url: "https://a.example/", seconds: null }]);
    render(<ToolsSection />);
    await waitFor(() => expect((screen.getByTestId("mcp-tools-searxng-urls") as HTMLTextAreaElement).value).toBe("https://a.example/"));
    fireEvent.click(screen.getByTestId("tools-searx-refresh"));
    await waitFor(() => expect((screen.getByTestId("tools-searx-inst-0") as HTMLInputElement).checked).toBe(true));
    sx.searxInstances.mockRejectedValueOnce("searx.space: HTTP 500");
    fireEvent.click(screen.getByTestId("tools-searx-refresh"));
    await waitFor(() => expect(screen.getByTestId("tools-searx-error").textContent).toContain("HTTP 500"));
  });
});
