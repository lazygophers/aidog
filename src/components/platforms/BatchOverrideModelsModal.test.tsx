// BatchOverrideModelsModal 的 jev 决策槽：只填 jev 也能确认、jev 进 onConfirm 入参、空白不算填。
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "../../test/render";
import { BatchOverrideModelsModal } from "./BatchOverrideModelsModal";
import type { Platform } from "../../services/api";

vi.mock("../../domains/platforms/defaults", () => ({
  getDefaultModels: vi.fn(async () => ({})),
  buildProtocolsFromPresets: vi.fn(async () => []),
}));

const t = ((key: string, fallback?: string) => fallback ?? key) as any;

function renderModal(onConfirm = vi.fn()) {
  const platforms = [{ id: 1, name: "p1", models: { default: "glm-5" } }] as unknown as Platform[];
  render(
    <BatchOverrideModelsModal open platforms={platforms} allPlatforms={platforms} onConfirm={onConfirm} onClose={vi.fn()} t={t} />,
  );
  // Dialog 走 Portal：手输模式 6 个槽位输入框按 MODEL_SLOTS 顺序，jev 是最后一个
  const inputs = Array.from(document.body.querySelectorAll("input")) as HTMLInputElement[];
  // 确认按钮：t stub 回 fallback「覆盖 {{count}} 个平台」（取消按钮是「取消」，右上角关闭按钮无此文本）
  const confirm = Array.from(document.body.querySelectorAll("button"))
    .find(b => b.textContent?.startsWith("覆盖"))!;
  return { inputs, confirm, onConfirm };
}

describe("BatchOverrideModelsModal jev slot", () => {
  it("only jev filled → confirm enabled and jev passed through", () => {
    const { inputs, confirm, onConfirm } = renderModal();
    expect(inputs).toHaveLength(6);
    expect(confirm.disabled).toBe(true);
    fireEvent.change(inputs[5], { target: { value: "jev-latest" } });
    expect(confirm.disabled).toBe(false);
    fireEvent.click(confirm);
    expect(onConfirm).toHaveBeenCalledWith({
      default: "", sonnet: "", opus: "", haiku: "", gpt: "", jev: "jev-latest",
    });
  });

  it("whitespace-only jev does not count as filled", () => {
    const { inputs, confirm } = renderModal();
    fireEvent.change(inputs[5], { target: { value: "   " } });
    expect(confirm.disabled).toBe(true);
  });
});
