// ToolsSection 就近测试（票 05 方向 A 总控台重写）：渲染真组件，mock transport 层 API，
// 断言走 data-testid / data-state（frontend-refactor-tests.md）。
// 拖拽排序：mock @dnd-kit/core 的 DndContext 捕获 onDragEnd，直接喂事件（落点判定列人工验收）。
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "../../test/render";

const h = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn() }));
const m = vi.hoisted(() => ({ selfInstall: vi.fn() }));
const sx = vi.hoisted(() => ({ searxInstances: vi.fn(), searxTest: vi.fn(), sources: vi.fn() }));
const dnd = vi.hoisted(() => ({ onDragEnd: null as null | ((e: unknown) => void) }));

vi.mock("../../services/transport", () => ({ invoke: vi.fn(), listen: vi.fn(async () => () => {}) }));
vi.mock("../../services/api/settings", () => ({ settingsApi: { get: h.get, set: h.set } }));
vi.mock("../../services/api/search", () => ({ searchApi: { sources: sx.sources, searxInstances: sx.searxInstances, searxTest: sx.searxTest } }));
vi.mock("../../services/api/mcp", () => ({ mcpApi: { selfInstall: m.selfInstall } }));

vi.mock("@dnd-kit/core", async () => {
  const actual = await vi.importActual<typeof import("@dnd-kit/core")>("@dnd-kit/core");
  return {
    ...actual,
    DndContext: (p: { children?: ReactNode; onDragEnd?: (e: unknown) => void }) => {
      dnd.onDragEnd = p.onDragEnd ?? null;
      return <>{p.children}</>;
    },
  };
});

import { toolStatus, ToolsSection } from "./ToolsSection";

const SETTINGS_SOURCES = ["tavily", "ddg", "google"];
const SOURCE_LIST = [
  { id: "tavily", kind: "keyed", needs_key: true, default_on: false, billing: "per_request" },
  { id: "brave", kind: "keyed", needs_key: true, default_on: false, billing: "per_request" },
  { id: "ddg", kind: "engine", needs_key: false, default_on: true, billing: "free" },
  { id: "google", kind: "engine", needs_key: false, default_on: true, billing: "free" },
  { id: "searxng", kind: "engine", needs_key: false, default_on: true, billing: "free" },
];

const railItem = (id: string) => screen.getByTestId(`tools-rail-${id}`);
const statusPill = (tool: string) => screen.getByTestId(`tools-status-${tool}`);
const statusOf = (tool: string) => statusPill(tool).getAttribute("data-state");

beforeEach(() => {
  vi.clearAllMocks();
  dnd.onDragEnd = null;
  h.get.mockResolvedValue(null);
  h.set.mockResolvedValue(undefined);
  m.selfInstall.mockReset();
  sx.searxInstances.mockReset();
  sx.searxTest.mockReset();
  sx.sources.mockResolvedValue(SOURCE_LIST);
});

describe("toolStatus 纯函数", () => {
  const keyed = new Set(["tavily"]);
  it("web_search：enabled=false → disabled（无论 sources）", () => {
    expect(toolStatus("web_search", { search: { enabled: false, sources: ["ddg"] } }, keyed)).toBe("disabled");
  });
  it("web_search：含免 key 引擎 → configured", () => {
    expect(toolStatus("web_search", { search: { enabled: true, sources: ["ddg"] } }, keyed)).toBe("configured");
  });
  it("web_search：只有 key 服务且有非空 key（含 *** ）→ configured", () => {
    expect(toolStatus("web_search", { search: { enabled: true, sources: ["tavily"], services: { tavily: { keys: ["***"] } } } }, keyed)).toBe("configured");
    expect(toolStatus("web_search", { search: { enabled: true, sources: ["tavily"], services: { tavily: { keys: ["k1"] } } } }, keyed)).toBe("configured");
  });
  it("web_search：只有 key 服务但 key 全空 → unconfigured", () => {
    expect(toolStatus("web_search", { search: { enabled: true, sources: ["tavily"], services: { tavily: { keys: ["", "  "] } } } }, keyed)).toBe("unconfigured");
    expect(toolStatus("web_search", { search: { enabled: true, sources: ["tavily"] } }, keyed)).toBe("unconfigured");
  });
  it("web_fetch：enabled=false → disabled；缺省 → configured", () => {
    expect(toolStatus("web_fetch", { fetch: { enabled: false } })).toBe("disabled");
    expect(toolStatus("web_fetch", {})).toBe("configured");
  });
  it("generate_image：缺 / 部分填写 → unconfigured；三项齐 → configured；无 disabled 态", () => {
    expect(toolStatus("generate_image", {})).toBe("unconfigured");
    expect(toolStatus("generate_image", { image: { base_url: "https://a", api_key: "", model: "" } })).toBe("unconfigured");
    expect(toolStatus("generate_image", { image: { base_url: "https://a", api_key: "k", model: "m" } })).toBe("configured");
  });
});

describe("ToolsSection 总控台", () => {
  it("rail 三态：搜索已配置（sources 有引擎）/ 抓网页已停用 / 生图未配置", async () => {
    h.get.mockResolvedValue({
      search: { enabled: true, sources: ["ddg"], mode: "parallel", services: {}, searxng_base_urls: [] },
      fetch: { enabled: false },
    });
    render(<ToolsSection />);
    await waitFor(() => expect(statusOf("web_search")).toBe("configured"));
    expect(statusOf("web_fetch")).toBe("disabled");
    expect(statusOf("generate_image")).toBe("unconfigured");
    expect(railItem("web_search").getAttribute("data-state")).toBe("configured");
  });

  it("执行路径图随模式与停用切换（草稿即时生效）；rail 状态按已存值，保存后才翻转", async () => {
    h.get.mockResolvedValue({
      search: { enabled: true, sources: ["ddg", "google"], mode: "sequential", services: {}, searxng_base_urls: [] },
    });
    render(<ToolsSection />);
    await waitFor(() => expect(screen.getByTestId("tools-flow").getAttribute("data-state")).toBe("sequential"));
    fireEvent.click(screen.getByTestId("tools-mode-parallel"));
    expect(screen.getByTestId("tools-flow").getAttribute("data-state")).toBe("parallel");
    const sw = screen.getByTestId("mcp-tools-enabled");
    expect(sw.getAttribute("data-state")).toBe("checked");
    fireEvent.click(sw);
    expect(screen.getByTestId("tools-flow").getAttribute("data-state")).toBe("off");
    // rail 状态按已存配置算：未保存前仍是 configured
    expect(statusOf("web_search")).toBe("configured");
    fireEvent.click(screen.getByTestId("mcp-tools-save"));
    await waitFor(() => expect(statusOf("web_search")).toBe("disabled"));
  });

  it("每卡独立保存：搜索卡保存走读-改-写，保留最新 fetch / image；写出的 sources 保持拖动后顺序", async () => {
    h.get.mockResolvedValue({
      search: { enabled: true, sources: ["ddg", "google"], mode: "sequential", services: {}, searxng_base_urls: [] },
      fetch: { enabled: true },
      image: { base_url: "https://p/v1", api_key: "k", model: "m" },
    });
    // 保存前他卡被另一会话更新：重新 get 返回新值，读-改-写必须带上它而不是旧快照。
    h.get.mockImplementation(async () => {
      if (h.get.mock.calls.length > 1) {
        return {
          search: { enabled: true, sources: ["ddg", "google"], mode: "sequential", services: {}, searxng_base_urls: [] },
          fetch: { enabled: false },
          image: { base_url: "https://other/v9", api_key: "k9", model: "m9" },
        };
      }
      return {
        search: { enabled: true, sources: ["ddg", "google"], mode: "sequential", services: {}, searxng_base_urls: [] },
        fetch: { enabled: true },
        image: { base_url: "https://p/v1", api_key: "k", model: "m" },
      };
    });
    render(<ToolsSection />);
    await waitFor(() => expect(statusOf("web_search")).toBe("configured"));
    // 拖动：把 google（active）拖到 ddg（over）之前
    await waitFor(() => expect(dnd.onDragEnd).toBeTruthy());
    act(() => dnd.onDragEnd!({ active: { id: "google" }, over: { id: "ddg" } }));
    fireEvent.click(screen.getByTestId("mcp-tools-save"));
    await waitFor(() => {
      const call = h.set.mock.calls[0];
      expect(call[0]).toBe("mcp_tools");
      expect(call[1]).toBe("settings");
      expect((call[2] as any).search.sources).toEqual(["google", "ddg"]);
      expect((call[2] as any).fetch).toEqual({ enabled: false });
      expect((call[2] as any).image).toEqual({ base_url: "https://other/v9", api_key: "k9", model: "m9" });
    });
  });

  it("抓网页卡只写 fetch 段：保存保留 search / image 原值", async () => {
    h.get.mockResolvedValue({
      search: { enabled: true, sources: ["ddg"], mode: "parallel", services: { tavily: { keys: ["***"] } }, searxng_base_urls: ["http://s"] },
      image: { base_url: "https://p/v1", api_key: "k", model: "m" },
    });
    render(<ToolsSection />);
    await waitFor(() => expect(statusOf("web_fetch")).toBe("configured"));
    fireEvent.click(railItem("web_fetch")); // 方向 A：右侧单卡，先在 rail 选中抓网页
    fireEvent.click(screen.getByTestId("mcp-tools-fetch-enabled"));
    fireEvent.click(screen.getByTestId("tools-fetch-save"));
    await waitFor(() => {
      const next = h.set.mock.calls[0][2] as any;
      expect(next.fetch).toEqual({ enabled: false });
      expect(next.search.services.tavily.keys).toEqual(["***"]);
      expect(next.search.searxng_base_urls).toEqual(["http://s"]);
      expect(next.image).toEqual({ base_url: "https://p/v1", api_key: "k", model: "m" });
    });
  });

  it("生图卡密码框默认隐藏、眼睛切换显隐；三字段齐保存写出 image", async () => {
    render(<ToolsSection />);
    await waitFor(() => expect(railItem("generate_image").getAttribute("data-state")).toBe("unconfigured"));
    fireEvent.click(railItem("generate_image"));
    const key = screen.getByTestId("mcp-tools-image-api-key") as HTMLInputElement;
    expect(key.type).toBe("password");
    fireEvent.click(screen.getByTestId("mcp-tools-image-api-key-eye"));
    expect((screen.getByTestId("mcp-tools-image-api-key") as HTMLInputElement).type).toBe("text");
    fireEvent.change(screen.getByTestId("mcp-tools-image-base-url"), { target: { value: "https://p/v1/" } });
    fireEvent.change(key, { target: { value: "sk" } });
    fireEvent.change(screen.getByTestId("mcp-tools-image-model"), { target: { value: "m1" } });
    expect(statusOf("generate_image")).toBe("unconfigured"); // 保存前状态按已存值算
    fireEvent.click(screen.getByTestId("tools-image-save"));
    await waitFor(() => {
      // settings 为空（全新安装）→ 只写本卡 image 段，其余段由后端缺省兜底
      expect(h.set.mock.calls[0][2]).toEqual({
        image: { base_url: "https://p/v1/", api_key: "sk", model: "m1" },
      });
    });
    // 保存成功后快照更新，rail 状态变已配置
    await waitFor(() => expect(statusOf("generate_image")).toBe("configured"));
  });

  it("key 服务行内密码框默认隐藏、可显隐、可增删；空行保存时剔除；*** 原样保留", async () => {
    h.get.mockResolvedValue({
      search: { enabled: true, sources: ["tavily"], mode: "sequential", services: { tavily: { keys: ["***"] } }, searxng_base_urls: [] },
    });
    render(<ToolsSection />);
    await waitFor(() => expect(screen.getByTestId("tools-key-input-tavily-0")).toBeTruthy());
    const input0 = () => screen.getByTestId("tools-key-input-tavily-0") as HTMLInputElement;
    expect(input0().type).toBe("password");
    expect(input0().value).toBe("***");
    fireEvent.click(screen.getByTestId("tools-key-input-tavily-0-eye"));
    expect((screen.getByTestId("tools-key-input-tavily-0") as HTMLInputElement).type).toBe("text");
    fireEvent.click(screen.getByTestId("tools-key-add-tavily"));
    fireEvent.change(screen.getByTestId("tools-key-input-tavily-1"), { target: { value: "  k2  " } });
    fireEvent.click(screen.getByTestId("mcp-tools-save"));
    await waitFor(() => {
      expect((h.set.mock.calls[0][2] as any).search.services.tavily.keys).toEqual(["***", "k2"]);
    });
  });

  it("来源池点选加入 / 移除已启用列表；「一起」含 key 服务出现计费提示，纯引擎不出现", async () => {
    render(<ToolsSection />);
    await waitFor(() => expect(screen.getByTestId("tools-src-chip-tavily")).toBeTruthy());
    fireEvent.click(screen.getByTestId("tools-src-chip-tavily"));
    fireEvent.click(screen.getByTestId("tools-mode-parallel"));
    expect(screen.getByTestId("tools-billing-warning")).toBeTruthy();
    fireEvent.click(screen.getByTestId("tools-src-remove-tavily"));
    expect(screen.queryByTestId("tools-billing-warning")).toBeNull();
    expect(screen.getByTestId("tools-src-chip-tavily").getAttribute("data-state")).toBe("off");
  });

  it("未保存离开：dirty 时经 registerNavGuard 拦截，确认后放行并还原草稿", async () => {
    const { requestNavigation } = await import("../../utils/navGuard");
    h.get.mockResolvedValue({ search: { enabled: true, sources: ["ddg"], mode: "parallel", services: {}, searxng_base_urls: [] } });
    render(<ToolsSection />);
    await waitFor(() => expect(statusOf("web_search")).toBe("configured"));
    // dirty banner 未出现
    expect(screen.queryByTestId("tools-unsaved-banner")).toBeNull();
    fireEvent.click(screen.getByTestId("tools-mode-sequential"));
    expect(screen.getByTestId("tools-unsaved-banner")).toBeTruthy();
    // 导航被守卫拦截：出现居中 Dialog
    let navigated = false;
    act(() => requestNavigation(() => { navigated = true; }));
    const dialog = await screen.findByTestId("tools-leave-dialog");
    expect(dialog).toBeTruthy();
    expect(navigated).toBe(false);
    fireEvent.click(screen.getByTestId("tools-leave-confirm"));
    expect(navigated).toBe(true);
    // 放行后草稿还原：banner 消失、模式回到已存值
    await waitFor(() => expect(screen.queryByTestId("tools-unsaved-banner")).toBeNull());
    expect(screen.getByTestId("tools-flow").getAttribute("data-state")).toBe("parallel");
  });

  it("未保存离开 Dialog：取消留在本页，导航不发生", async () => {
    h.get.mockResolvedValue({ search: { enabled: true, sources: ["ddg"], mode: "parallel", services: {}, searxng_base_urls: [] } });
    render(<ToolsSection />);
    await waitFor(() => expect(statusOf("web_search")).toBe("configured"));
    fireEvent.click(screen.getByTestId("tools-mode-sequential"));
    const { requestNavigation } = await import("../../utils/navGuard");
    let navigated = false;
    act(() => requestNavigation(() => { navigated = true; }));
    await screen.findByTestId("tools-leave-dialog");
    fireEvent.click(screen.getByTestId("tools-leave-cancel"));
    expect(navigated).toBe(false);
    expect(screen.queryByTestId("tools-leave-dialog")).toBeNull();
  });

  it("generate_video 与「未验证」标记不出现；搜索卡不含 video 字段写出", async () => {
    render(<ToolsSection />);
    await waitFor(() => expect(screen.getByTestId("tools-rail-web_search")).toBeTruthy());
    expect(screen.queryByTestId(/video/i)).toBeNull();
    expect(JSON.stringify(document.body.textContent)).not.toContain("generate_video");
    expect(JSON.stringify(document.body.textContent)).not.toContain("未验证");
    fireEvent.click(railItem("generate_image"));
    fireEvent.click(screen.getByTestId("tools-image-save"));
    await waitFor(() => {
      expect((h.set.mock.calls[0][2] as any).video).toBeUndefined();
    });
  });

  it("SearXNG 行式列表：已知 URL 合并去重渲染行，只在点击刷新时拉一次公共实例，勾选行写入并随保存持久化", async () => {
    h.get.mockResolvedValue({ search: { enabled: true, sources: ["searxng"], mode: "parallel", services: {}, searxng_base_urls: ["http://mine"] } });
    sx.searxInstances.mockResolvedValue([{ url: "https://a.example/", seconds: 0.3 }]);
    render(<ToolsSection />);
    await waitFor(() => expect(screen.getByTestId("tools-searx-row-0")).toBeTruthy());
    expect(screen.getByTestId("tools-searx-row-0").getAttribute("data-state")).toBe("on");
    expect(sx.searxInstances).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("tools-searx-refresh"));
    await waitFor(() => expect(screen.getByTestId("tools-searx-row-1")).toBeTruthy());
    expect(screen.getByTestId("tools-searx-row-1").textContent).toContain("https://a.example/");
    expect(sx.searxInstances).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("tools-searx-row-1").getAttribute("data-state")).toBe("off");
    fireEvent.click(screen.getByTestId("tools-searx-row-1"));
    expect(screen.getByTestId("tools-searx-row-1").getAttribute("data-state")).toBe("on");
    fireEvent.click(screen.getByTestId("mcp-tools-save"));
    await waitFor(() =>
      expect((h.set.mock.calls[0][2] as any).search.searxng_base_urls).toEqual(["http://mine", "https://a.example/"]),
    );
  });

  it("SearXNG 批量按钮：全选 / 反选 / 全不选 / 选择所有可用（只勾本会话测速通过的，未测不算）", async () => {
    h.get.mockResolvedValue({ search: { enabled: true, sources: ["searxng"], mode: "parallel", services: {}, searxng_base_urls: ["http://1", "http://2", "http://3"] } });
    sx.searxTest.mockImplementation(async (url: string) => {
      if (url === "http://2") throw new Error("connect timeout");
      return url === "http://1" ? 0.12 : 0.2;
    });
    render(<ToolsSection />);
    await waitFor(() => expect(screen.getByTestId("tools-searx-row-2")).toBeTruthy());
    const state = (i: number) => screen.getByTestId(`tools-searx-row-${i}`).getAttribute("data-state");
    // 初始全 on；反选 → 全 off
    fireEvent.click(screen.getByTestId("tools-searx-invert"));
    expect(state(0)).toBe("off");
    expect(state(2)).toBe("off");
    // 全选 → 全 on；全不选 → 全 off
    fireEvent.click(screen.getByTestId("tools-searx-all"));
    expect(state(1)).toBe("on");
    fireEvent.click(screen.getByTestId("tools-searx-none"));
    expect(state(1)).toBe("off");
    // 逐行测速：1 / 3 可用（写回耗时），2 失败标红不可用
    fireEvent.click(screen.getByTestId("tools-searx-test-0"));
    await waitFor(() => expect(screen.getByTestId("tools-searx-sec-0").textContent).toContain("0.12"));
    fireEvent.click(screen.getByTestId("tools-searx-test-1"));
    await waitFor(() => expect(screen.getByTestId("tools-searx-sec-1").textContent).toBe("tools.searxFail"));
    fireEvent.click(screen.getByTestId("tools-searx-test-2"));
    await waitFor(() => expect(screen.getByTestId("tools-searx-sec-2").textContent).toContain("0.20"));
    fireEvent.click(screen.getByTestId("tools-searx-available"));
    expect(state(0)).toBe("on");
    expect(state(1)).toBe("off");
    expect(state(2)).toBe("on");
    fireEvent.click(screen.getByTestId("mcp-tools-save"));
    await waitFor(() => expect((h.set.mock.calls[0][2] as any).search.searxng_base_urls).toEqual(["http://1", "http://3"]));
  });

  it("SearXNG 行内添加与删除：trim 添加即启用并去重，删行移出启用集，保存持久化", async () => {
    h.get.mockResolvedValue({ search: { enabled: true, sources: ["searxng"], mode: "parallel", services: {}, searxng_base_urls: ["http://1"] } });
    render(<ToolsSection />);
    await waitFor(() => expect(screen.getByTestId("tools-searx-row-0")).toBeTruthy());
    fireEvent.change(screen.getByTestId("tools-searx-add"), { target: { value: "  http://new  " } });
    fireEvent.click(screen.getByTestId("tools-searx-add-btn"));
    expect((screen.getByTestId("tools-searx-add") as HTMLInputElement).value).toBe("");
    expect(screen.getByTestId("tools-searx-row-1").textContent).toContain("http://new");
    expect(screen.getByTestId("tools-searx-row-1").getAttribute("data-state")).toBe("on");
    // 重复添加去重：不产生第二行
    fireEvent.change(screen.getByTestId("tools-searx-add"), { target: { value: "http://new" } });
    fireEvent.click(screen.getByTestId("tools-searx-add-btn"));
    expect(screen.queryByTestId("tools-searx-row-2")).toBeNull();
    // 删行：row-0（http://1）移除，http://new 顶到 row-0 且仍启用
    fireEvent.click(screen.getByTestId("tools-searx-del-0"));
    expect(screen.getByTestId("tools-searx-row-0").textContent).toContain("http://new");
    expect(screen.getByTestId("tools-searx-row-0").getAttribute("data-state")).toBe("on");
    expect(screen.queryByTestId("tools-searx-row-1")).toBeNull();
    fireEvent.click(screen.getByTestId("mcp-tools-save"));
    await waitFor(() => expect((h.set.mock.calls[0][2] as any).search.searxng_base_urls).toEqual(["http://new"]));
  });

  it("一键写入本机 agent：成功 / 失败展示结果", async () => {
    m.selfInstall.mockResolvedValue({ name: "aidog", url: "http://127.0.0.1:9890/mcp", agents: ["claude-code", "codex"] });
    render(<ToolsSection />);
    await waitFor(() => expect(screen.getByTestId("tools-rail-install")).toBeTruthy());
    fireEvent.click(screen.getByTestId("tools-rail-install"));
    fireEvent.click(screen.getByTestId("mcp-tools-install"));
    const res = await screen.findByTestId("mcp-tools-install-result");
    expect(res).toHaveTextContent("tools.installed");
    expect(m.selfInstall).toHaveBeenCalledTimes(1);
  });

  it("加载失败展示原文", async () => {
    h.get.mockRejectedValue("db locked");
    render(<ToolsSection />);
    expect(await screen.findByTestId("mcp-tools-load-error")).toHaveTextContent("db locked");
  });
});
