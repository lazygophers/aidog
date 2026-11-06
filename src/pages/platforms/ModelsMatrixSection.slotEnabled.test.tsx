// 槽位启用开关：每个内置 / 自定义槽位行一个 Switch（开 = 启用，关 = 平台不接收命中该槽位的请求）；选择器锚 data-testid / data-state。
import { render, fireEvent } from "../../test/render";
import { describe, it, expect, vi } from "vitest";
vi.mock("../../domains/platforms/modelPrices", () => ({ useModelPrices: () => new Map() }));
import { ModelsMatrixSection } from "./ModelsMatrixSection";

const empty = { embedding: [], rerank: [], default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [] };
function mount(over: Record<string, unknown>) {
  const onSlotEnabledChange = vi.fn();
  const r = render(
    <ModelsMatrixSection
      models={empty as never} onModelsChange={() => {}}
      activeDropdown={null} setActiveDropdown={() => {}} availableModels={["a", "b"]}
      protocol={"anthropic" as never} fetchError="" fetching={false} onFillAll={() => {}}
      onFetchModels={() => {}} apiKeyMissing={false} endpointsCount={1}
      endpoints={[]} rules={[]} setRules={() => {}} peak={[]}
      tzMode={"utc" as never} setTzMode={() => {}} t={((k: string) => k) as never}
      onSlotEnabledChange={onSlotEnabledChange}
      {...over}
    />,
  );
  return { onSlotEnabledChange, ...r };
}
const toggle = (c: HTMLElement, slot: string) => c.querySelector(`[data-testid="slot-enabled-${slot}"]`) as HTMLElement;

describe("ModelsMatrixSection 槽位启用开关", () => {
  it("8 个内置槽位各有开关，data-state 反映是否启用（关闭的槽 unchecked）", () => {
    const { container } = mount({ disabledSlots: ["opus"] });
    for (const s of ["default", "sonnet", "opus", "haiku", "gpt", "jev", "embedding", "rerank"]) {
      expect(toggle(container, s)).not.toBeNull();
    }
    expect(toggle(container, "opus").getAttribute("data-state")).toBe("unchecked");
    expect(toggle(container, "default").getAttribute("data-state")).toBe("checked");
  });
  it("自定义槽位行也有开关", () => {
    const { container } = mount({ customSlotDefs: [{ name: "fast-x", match: "contains" }], disabledSlots: ["fast-x"] });
    expect(toggle(container, "fast-x").getAttribute("data-state")).toBe("unchecked");
  });
  it("点击切换：已启用 → 回调 (槽名, false)；已关闭 → (槽名, true)", () => {
    const { container, onSlotEnabledChange } = mount({ disabledSlots: ["opus"] });
    fireEvent.click(toggle(container, "default"));
    expect(onSlotEnabledChange).toHaveBeenLastCalledWith("default", false);
    fireEvent.click(toggle(container, "opus"));
    expect(onSlotEnabledChange).toHaveBeenLastCalledWith("opus", true);
  });
});
