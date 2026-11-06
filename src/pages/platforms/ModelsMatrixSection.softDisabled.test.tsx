// 备用槽位开关：每个内置 / 自定义槽位行一个 Switch；选择器锚 data-testid / data-state。
import { render, fireEvent } from "../../test/render";
import { describe, it, expect, vi } from "vitest";
vi.mock("../../domains/platforms/modelPrices", () => ({ useModelPrices: () => new Map() }));
import { ModelsMatrixSection } from "./ModelsMatrixSection";

const empty = { embedding: [], rerank: [], default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [] };
function mount(over: Record<string, unknown>) {
  const onSoftDisabledChange = vi.fn();
  const r = render(
    <ModelsMatrixSection
      models={empty as never} onModelsChange={() => {}}
      activeDropdown={null} setActiveDropdown={() => {}} availableModels={["a", "b"]}
      protocol={"anthropic" as never} fetchError="" fetching={false} onFillAll={() => {}}
      onFetchModels={() => {}} apiKeyMissing={false} endpointsCount={1}
      endpoints={[]} rules={[]} setRules={() => {}} peak={[]}
      tzMode={"utc" as never} setTzMode={() => {}} t={((k: string) => k) as never}
      onSoftDisabledChange={onSoftDisabledChange}
      {...over}
    />,
  );
  return { onSoftDisabledChange, ...r };
}
const toggle = (c: HTMLElement, slot: string) => c.querySelector(`[data-testid="soft-toggle-${slot}"]`) as HTMLElement;

describe("ModelsMatrixSection 备用槽位开关", () => {
  it("8 个内置槽位各有开关，data-state 反映是否备用", () => {
    const { container } = mount({ softDisabledSlots: ["opus"] });
    for (const s of ["default", "sonnet", "opus", "haiku", "gpt", "jev", "embedding", "rerank"]) {
      expect(toggle(container, s)).not.toBeNull();
    }
    expect(toggle(container, "opus").getAttribute("data-state")).toBe("checked");
    expect(toggle(container, "default").getAttribute("data-state")).toBe("unchecked");
  });
  it("自定义槽位行也有开关", () => {
    const { container } = mount({ customSlotDefs: [{ name: "fast-x", match: "contains" }], softDisabledSlots: ["fast-x"] });
    expect(toggle(container, "fast-x").getAttribute("data-state")).toBe("checked");
  });
  it("点击切换：未备用 → 回调 (槽名, true)；已备用 → (槽名, false)", () => {
    const { container, onSoftDisabledChange } = mount({ softDisabledSlots: ["opus"] });
    fireEvent.click(toggle(container, "default"));
    expect(onSoftDisabledChange).toHaveBeenLastCalledWith("default", true);
    fireEvent.click(toggle(container, "opus"));
    expect(onSoftDisabledChange).toHaveBeenLastCalledWith("opus", false);
  });
});
