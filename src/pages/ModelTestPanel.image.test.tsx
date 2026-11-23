// ModelTestPanel 生图模式就近测试（生图-模型类型 票 10）：
// image 模式点 Run → 走 modelTestApi.imageTest（写 request_log kind='test' 链），非聊天 model_test。
// 选择器锚 i18n key（test/render 空 resources 回退 key）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render, screen } from "../test/render";

const calls = vi.hoisted(() => ({ test: [] as unknown[], imageTest: [] as unknown[] }));
vi.mock("../services/api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("../services/api")>();
  return {
    ...orig,
    modelTestApi: {
      test: vi.fn(async (req: unknown) => { calls.test.push(req); return okResult("chat-model"); }),
      imageTest: vi.fn(async (req: unknown) => { calls.imageTest.push(req); return okResult("gpt-image-2"); }),
    },
  };
});
import { ModelTestPanel } from "./ModelTestPanel";
import type { Platform } from "../services/api";

function okResult(model: string) {
  return {
    success: true, model, prompt_preview: "p", response_preview: "1 image(s) generated",
    duration_ms: 5, input_tokens: 1, output_tokens: 1, error: "",
  };
}

const platform = {
  id: 7, name: "OpenAI", platform_type: "openai", base_url: "https://api.openai.com/v1",
  extra: "{}",
  models: { default: ["gpt-5"], image: ["gpt-image-2", "flux-pro"] },
  available_models: ["gpt-5"],
  endpoints: [], status: "enabled", est_balance_remaining: 0, balance_level: "",
  manual_budgets: [],
} as never as Platform;

describe("ModelTestPanel image mode", () => {
  beforeEach(() => { calls.test.length = 0; calls.imageTest.length = 0; });

  it("image mode runs through imageTest with image slot models", async () => {
    render(<ModelTestPanel platform={platform} onClose={() => {}} />);
    fireEvent.click(screen.getByText("test.modeImage"));
    fireEvent.click(screen.getByText("test.run"));
    // 等异步 invoke 链收敛（两条 image 槽位模型各一次）
    await vi.waitFor(() => {
      expect(calls.imageTest).toHaveLength(2);
      expect(calls.test).toHaveLength(0);
    });
    expect((calls.imageTest[0] as { model: string }).model).toBe("gpt-image-2");
    expect(screen.getByText("test.results")).toBeTruthy();
  });

  it("quick mode still uses chat model_test", async () => {
    render(<ModelTestPanel platform={platform} onClose={() => {}} />);
    fireEvent.click(screen.getByText("test.run"));
    await vi.waitFor(() => {
      expect(calls.test).toHaveLength(1);
      expect(calls.imageTest).toHaveLength(0);
    });
  });
});
