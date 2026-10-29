// 行为测试（票 12）：滑杆/输入触发重算、JSON 解析回填、对比卡最低高亮。
// 选择器锚稳定 data-testid + data-state，禁锚 DOM 层级 / class / getByText 文案。
// invoke 全 mock（test-no-real-calls.md），无真实外呼。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "../../test/render";
import { CostSimulator } from "./CostSimulator";
import type { ModelEntryGroup } from "../../services/api";

const estimateMock = vi.fn();
const platformListMock = vi.fn();

vi.mock("../../services/api", () => ({
  costSimulatorApi: { estimate: (...a: unknown[]) => estimateMock(...a) },
  platformApi: { list: () => platformListMock() },
}));

// 模拟器输入侧基名（sim-model 等）不用 i18n， groups 展示名直接渲染原文。

function group(over: Partial<ModelEntryGroup> = {}): ModelEntryGroup {
  return {
    canonical_model: "glm-4.6",
    display_name: "GLM-4.6",
    primary_platform: "glm",
    entries: [
      {
        platform_code: "glm", model_id: "glm-4.6", canonical_model: "glm-4.6",
        display_name: "GLM-4.6", family: "glm", version: "4.6", predecessor: "glm-4.5",
        capabilities: [], builtin_tools_excluded: [], max_input_tokens: null,
        max_output_tokens: null, context_window: null, official: true,
        price_data: JSON.stringify({
          price: { input: 1e-6, output: 4e-6, cache_read: 1e-7, cache_write: 1e-6 },
        }),
        updated_at: 0,
      },
      {
        platform_code: "openrouter", model_id: "zhipu/glm-4.6", canonical_model: "glm-4.6",
        display_name: "GLM-4.6 (OR)", family: "glm", version: "4.6", predecessor: "",
        capabilities: [], builtin_tools_excluded: [], max_input_tokens: null,
        max_output_tokens: null, context_window: null, official: false,
        price_data: JSON.stringify({ price: { input: 2e-6, output: 4e-6 } }),
        updated_at: 0,
      },
    ],
    ...over,
  };
}

const dto = (est_cost: number, over: Record<string, unknown> = {}) =>
  ({ est_cost, price_source: "registry", is_peak: false, multiplier: 1, ...over });

beforeEach(() => {
  estimateMock.mockReset().mockImplementation(({ platformId }: { platformId: number }) =>
    Promise.resolve(platformId === 1 ? dto(0.02) : dto(0.005)));
  platformListMock.mockReset().mockResolvedValue([
    { id: 1, platform_type: "glm", name: "GLM" },
    { id: 2, platform_type: "openrouter", name: "OpenRouter" },
  ]);
});

describe("CostSimulator", () => {
  it("输入框改动触发 simulator_est_cost 重算（带新 token 数）", async () => {
    render(<CostSimulator groups={[group()]} labelMap={{}} />);
    await waitFor(() => expect(estimateMock).toHaveBeenCalled());
    const calls0 = estimateMock.mock.calls.length;
    fireEvent.change(screen.getByTestId("sim-tok-output"), { target: { value: "8000" } });
    await waitFor(() => expect(estimateMock.mock.calls.length).toBeGreaterThan(calls0));
    const last = estimateMock.mock.calls.at(-1)![0] as { outputTokens: number };
    expect(last.outputTokens).toBe(8000);
  });

  it("滑杆改动触发重算并同步数字输入框", async () => {
    render(<CostSimulator groups={[group()]} labelMap={{}} />);
    await waitFor(() => expect(estimateMock).toHaveBeenCalled());
    const calls0 = estimateMock.mock.calls.length;
    fireEvent.change(screen.getByTestId("sim-range-input"), { target: { value: "60000" } });
    await waitFor(() => expect(estimateMock.mock.calls.length).toBeGreaterThan(calls0));
    expect((screen.getByTestId("sim-tok-input") as HTMLInputElement).value).toBe("60000");
    const last = estimateMock.mock.calls.at(-1)![0] as { inputTokens: number };
    expect(last.inputTokens).toBe(60000);
  });

  it("粘贴请求 JSON 解析回填 input tokens（字符数 ÷4），model 命中切换分组", async () => {
    const other = group({
      canonical_model: "claude-sonnet-4-5", display_name: "Sonnet", primary_platform: "anthropic",
      entries: [{
        platform_code: "anthropic", model_id: "claude-sonnet-4-5", canonical_model: "claude-sonnet-4-5",
        display_name: "", family: "", version: "", predecessor: "", capabilities: [],
        builtin_tools_excluded: [], max_input_tokens: null, max_output_tokens: null,
        context_window: null, official: true, price_data: "{}", updated_at: 0,
      }],
    });
    platformListMock.mockResolvedValue([
      { id: 1, platform_type: "glm", name: "GLM" },
      { id: 3, platform_type: "anthropic", name: "Anthropic" },
    ]);
    render(<CostSimulator groups={[group(), other]} labelMap={{}} />);
    await waitFor(() => expect(estimateMock).toHaveBeenCalled());
    // 16 字符 ÷4 = 4 tokens
    fireEvent.change(screen.getByTestId("sim-paste"), {
      target: { value: '{"model":"claude-sonnet-4-5","messages":[{"role":"user","content":"0123456789abcdef"}]}' },
    });
    fireEvent.click(screen.getByTestId("sim-parse"));
    await waitFor(() =>
      expect((screen.getByTestId("sim-tok-input") as HTMLInputElement).value).toBe("4"));
    // model 命中第二个分组 → estimate 以其条目 model_id 调用
    await waitFor(() =>
      expect(estimateMock.mock.calls.some(c => (c[0] as { modelName: string }).modelName === "claude-sonnet-4-5")).toBe(true));
  });

  it("对比卡最低价高亮（data-state=lowest），unsupported 不参与最低判定", async () => {
    // openrouter(2) = 0.005 最低；glm(1) 0.02；再挂一个 unsupported 平台 0 价也不得高亮
    estimateMock.mockImplementation(({ platformId }: { platformId: number }) =>
      Promise.resolve(
        platformId === 1 ? dto(0.02)
          : platformId === 2 ? dto(0.005)
          : dto(0, { price_source: "unsupported" })));
    platformListMock.mockResolvedValue([
      { id: 1, platform_type: "glm", name: "GLM" },
      { id: 2, platform_type: "openrouter", name: "OpenRouter" },
      { id: 3, platform_type: "litellm", name: "LiteLLM" },
    ]);
    const g = group();
    g.entries.push({
      platform_code: "litellm", model_id: "glm-4.6", canonical_model: "glm-4.6",
      display_name: "", family: "", version: "", predecessor: "", capabilities: [],
      builtin_tools_excluded: [], max_input_tokens: null, max_output_tokens: null,
      context_window: null, official: false, price_data: "{}", updated_at: 0,
    });
    render(<CostSimulator groups={[g]} labelMap={{}} />);
    await waitFor(() =>
      expect(screen.getByTestId("sim-card-openrouter")).toHaveAttribute("data-state", "lowest"));
    expect(screen.getByTestId("sim-card-glm")).toHaveAttribute("data-state", "normal");
    expect(screen.getByTestId("sim-card-litellm")).toHaveAttribute("data-state", "unsupported");
  });

  it("高峰徽标与计价明细展示 command 返回的 is_peak / multiplier / price_source", async () => {
    estimateMock.mockImplementation(({ platformId }: { platformId: number }) =>
      Promise.resolve(platformId === 1
        ? dto(0.03, { is_peak: true, multiplier: 2 })
        : dto(0.005)));
    render(<CostSimulator groups={[group()]} labelMap={{}} />);
    const peak = await screen.findByTestId("sim-peak");
    await waitFor(() => expect(peak).toHaveAttribute("data-state", "peak"));
    expect(screen.getByTestId("sim-source")).toHaveTextContent("registry");
    expect(screen.getByTestId("sim-row-input")).toBeInTheDocument();
  });
});
