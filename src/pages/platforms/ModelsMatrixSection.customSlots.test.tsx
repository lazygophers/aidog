// 自定义槽位行：随全局定义动态渲染，值读写走 onCustomSlotChange；选择器锚 data-testid / data-state。
import { render, fireEvent } from "../../test/render";
import { describe, it, expect, vi } from "vitest";
vi.mock("../../domains/platforms/modelPrices", () => ({ useModelPrices: () => new Map() }));
import { ModelsMatrixSection } from "./ModelsMatrixSection";

const empty = { embedding: [], rerank: [], default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [] };
function mount(over: Record<string, unknown>) {
  const onCustomSlotChange = vi.fn();
  const r = render(
    <ModelsMatrixSection
      models={empty as never} onModelsChange={() => {}}
      activeDropdown={null} setActiveDropdown={() => {}} availableModels={["a", "b"]}
      protocol={"anthropic" as never} fetchError="" fetching={false} onFillAll={() => {}}
      onFetchModels={() => {}} apiKeyMissing={false} endpointsCount={1}
      endpoints={[]} rules={[]} setRules={() => {}} peak={[]}
      tzMode={"utc" as never} setTzMode={() => {}} t={((k: string) => k) as never}
      onCustomSlotChange={onCustomSlotChange}
      {...over}
    />,
  );
  return { onCustomSlotChange, ...r };
}
const defs = [{ name: "fast-x", match: "contains" }, { name: "pro", match: "exact" }];

describe("ModelsMatrixSection 自定义槽位", () => {
  it("无定义 → 不渲染自定义行", () => {
    const { container } = mount({ customSlotDefs: [], customSlotValues: {} });
    expect(container.querySelector('[data-testid^="custom-slot-"]')).toBeNull();
  });
  it("每个定义渲染一行，徽标 data-state 为匹配方式", () => {
    const { container } = mount({ customSlotDefs: defs, customSlotValues: {} });
    expect(container.querySelector('[data-testid="custom-slot-fast-x"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="custom-slot-badge-fast-x"]')!.getAttribute("data-state")).toBe("contains");
    expect(container.querySelector('[data-testid="custom-slot-badge-pro"]')!.getAttribute("data-state")).toBe("exact");
  });
  it("值取自 customSlotValues，选择追加后写出整条链", () => {
    const { container, onCustomSlotChange } = mount({ customSlotDefs: defs, customSlotValues: { "fast-x": ["a"] } });
    fireEvent.focus(container.querySelector('[data-testid="chain-input-c-fast-x"]')!);
    fireEvent.mouseDown(document.body.querySelector('[data-testid="model-option-b"]')!);
    expect(onCustomSlotChange).toHaveBeenCalledWith("fast-x", ["a", "b"]);
  });
});
