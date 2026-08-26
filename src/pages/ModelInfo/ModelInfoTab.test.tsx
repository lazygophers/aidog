// 行为测试：双 tab 列表渲染 + 详情弹窗按平台聚合比价 + partial 失败清单。
// 断言用 i18n key（render.tsx 空 resources 回退 key），不依赖文案。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "../../test/render";
import { ModelInfoTab } from "./ModelInfoTab";
import type { ModelEntry, ModelInfoSnapshot, PriceSyncResult } from "../../services/api";

const snapshotMock = vi.fn();
const syncMock = vi.fn();
const settingsGetMock = vi.fn();

vi.mock("../../services/api", () => ({
  modelInfoApi: { snapshot: () => snapshotMock() },
  modelPriceApi: { sync: () => syncMock() },
  priceSyncApi: { get: () => settingsGetMock(), set: vi.fn().mockResolvedValue(undefined) },
}));

vi.mock("../../domains/platforms/defaults", () => ({
  getProtocolLabelMap: vi.fn().mockResolvedValue({ glm: "智谱 GLM", openrouter: "OpenRouter" }),
}));

// ProtocolLogo 内部走 get_protocol_logo_path IPC + colorMap，测试里换成哑元
vi.mock("../../domains/platforms/ProtocolLogo", () => ({
  ProtocolLogo: ({ protocol }: { protocol: string }) => <span data-testid={`logo-${protocol}`} />,
}));

function entry(over: Partial<ModelEntry> & Pick<ModelEntry, "platform_code" | "model_id">): ModelEntry {
  return {
    display_name: over.model_id,
    canonical_model: over.model_id,
    family: "",
    version: "",
    predecessor: "",
    capabilities: [],
    builtin_tools_excluded: [],
    max_input_tokens: null,
    max_output_tokens: null,
    context_window: null,
    official: false,
    price_data: "{}",
    updated_at: 0,
    ...over,
  };
}

const SNAPSHOT: ModelInfoSnapshot = {
  bundled: false,
  platforms: [],
  groups: [
    {
      canonical_model: "glm-4.6",
      display_name: "GLM-4.6",
      primary_platform: "glm",
      entries: [
        entry({
          platform_code: "glm",
          model_id: "glm-4.6",
          display_name: "GLM-4.6",
          capabilities: ["text", "tool_use"],
          context_window: 131072,
          official: true,
          family: "glm",
          version: "4.6",
          predecessor: "glm-4.5",
          price_data: JSON.stringify({
            input_cost_per_token: 1.1e-6,
            output_cost_per_token: 4.2e-6,
            peak: { input_cost_per_token: 3.3e-6, output_cost_per_token: 1.26e-5 },
          }),
        }),
        entry({
          platform_code: "openrouter",
          model_id: "zhipu/glm-4.6",
          display_name: "GLM-4.6 (OpenRouter)",
          capabilities: ["text"],
          price_data: JSON.stringify({ input_cost_per_token: 2e-6 }),
        }),
      ],
    },
  ],
};

describe("ModelInfoTab", () => {
  beforeEach(() => {
    snapshotMock.mockReset().mockResolvedValue(SNAPSHOT);
    syncMock.mockReset();
    settingsGetMock.mockReset().mockResolvedValue({
      auto_sync_enabled: false,
      sync_interval_secs: 86400,
      last_sync_at: 0,
      fallback_input_price: 3,
      fallback_output_price: 3,
    });
  });

  it("模型维度 tab: 一行一 canonical，展示名 + 代表平台 + 官方徽标", async () => {
    render(<ModelInfoTab />);
    expect(await screen.findByText("GLM-4.6")).toBeInTheDocument();
    // 代表条目 = primary_platform，平台名走 registry labelMap
    expect(screen.getByText("智谱 GLM")).toBeInTheDocument();
    expect(screen.getByText("modelInfo.official")).toBeInTheDocument();
    // 第二个平台折叠成「还有 N 个平台」提示
    expect(screen.getByText("modelInfo.morePlatforms")).toBeInTheDocument();
  });

  it("平台维度 tab: 切过去后按平台列出模型条目", async () => {
    render(<ModelInfoTab />);
    await screen.findByText("GLM-4.6");
    // Radix TabsTrigger 在 mouseDown 阶段切值（click 不触发）
    fireEvent.mouseDown(screen.getByRole("tab", { name: "modelInfo.tabPlatforms" }), { button: 0 });
    // 未选平台时给引导文案
    expect(await screen.findByText("modelInfo.selectPlatform")).toBeInTheDocument();
    fireEvent.click(screen.getByText("OpenRouter"));
    expect(await screen.findByText("GLM-4.6 (OpenRouter)")).toBeInTheDocument();
    expect(screen.getByText("zhipu/glm-4.6")).toBeInTheDocument();
  });

  it("详情弹窗: 点击行后按平台分 tab，聚合版本链 / 默认价 / 高峰价", async () => {
    render(<ModelInfoTab />);
    fireEvent.click(await screen.findByText("GLM-4.6"));
    const dialog = await screen.findByRole("dialog");
    // 两个平台条目各一个 tab
    expect(within(dialog).getByRole("tab", { name: /智谱 GLM/ })).toBeInTheDocument();
    expect(within(dialog).getByRole("tab", { name: /OpenRouter/ })).toBeInTheDocument();
    // 默认打开 primary_platform：版本链 + 高峰价
    expect(within(dialog).getByText("glm-4.5")).toBeInTheDocument();
    expect(within(dialog).getByText("modelInfo.priceDefault")).toBeInTheDocument();
    expect(within(dialog).getByText("modelInfo.pricePeak")).toBeInTheDocument();
  });

  it("同步失败清单: partial failures 列出文件与原因", async () => {
    const result: PriceSyncResult = {
      added: 1, updated: 2, unchanged: 0, failed: 1, total: 4,
      failures: [{ file: "platforms/glm/models/glm-4.6.json", error: "404" }],
    };
    syncMock.mockResolvedValue(result);
    render(<ModelInfoTab />);
    await screen.findByText("GLM-4.6");
    fireEvent.click(screen.getByText("modelInfo.syncNow"));
    await waitFor(() => expect(syncMock).toHaveBeenCalled());
    expect(await screen.findByText("platforms/glm/models/glm-4.6.json")).toBeInTheDocument();
    expect(screen.getByText(/404/)).toBeInTheDocument();
  });

  it("bundled 兜底: snapshot.bundled=true 时提示尚未同步", async () => {
    snapshotMock.mockResolvedValue({ ...SNAPSHOT, bundled: true });
    render(<ModelInfoTab />);
    expect(await screen.findByText("modelInfo.bundledNotice")).toBeInTheDocument();
  });
});
