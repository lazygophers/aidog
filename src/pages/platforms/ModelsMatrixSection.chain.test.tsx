// 矩阵接入模型链编辑器：默认列写 onModelsChange(slot, 数组)，时段档写 setRules(models 槽位为数组)。
// 选择器锚稳定 data-testid，不锚文案 / DOM 层级。
import { render, fireEvent } from "../../test/render";
import { describe, it, expect, vi } from "vitest";
vi.mock("../../domains/platforms/modelPrices", () => ({ useModelPrices: () => new Map() }));
import { ModelsMatrixSection } from "./ModelsMatrixSection";

const empty = { embedding: [], rerank: [], default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [] };

function mount(over: Record<string, unknown>) {
  const onModelsChange = vi.fn();
  const setRules = vi.fn();
  const r = render(
    <ModelsMatrixSection
      models={{ ...empty, default: ["a"] } as never} onModelsChange={onModelsChange}
      activeDropdown={null} setActiveDropdown={() => {}} availableModels={["a", "b", "c"]}
      protocol={"anthropic" as never} fetchError="" fetching={false} onFillAll={() => {}}
      onFetchModels={() => {}} apiKeyMissing={false} endpointsCount={1}
      endpoints={[]} rules={[]} setRules={setRules} peak={[]}
      tzMode={"utc" as never} setTzMode={() => {}} t={((k: string) => k) as never}
      {...over}
    />,
  );
  return { onModelsChange, setRules, ...r };
}
const pick = (m: string) => fireEvent.mouseDown(document.body.querySelector(`[data-testid="model-option-${m}"]`)!);

describe("ModelsMatrixSection 模型链", () => {
  it("默认列：下拉已选打勾，点选追加写出整条链", () => {
    const { container, onModelsChange } = mount({});
    fireEvent.focus(container.querySelector('[data-testid="chain-input-d-default"]')!);
    expect(document.body.querySelector('[data-testid="model-option-a"]')?.getAttribute("data-state")).toBe("checked");
    pick("b");
    expect(onModelsChange).toHaveBeenCalledWith("default", ["a", "b"]);
  });

  it("时段档：编辑写出数组，已选在该槽下拉打勾；删光槽位键被去掉", () => {
    const rules = [{ windows: [], models: { default: ["a"], opus: ["c"] } }];
    const { container, setRules } = mount({ columns: "time", rules });
    fireEvent.focus(container.querySelector('[data-testid="chain-input-r0-default"]')!);
    expect(document.body.querySelector('[data-testid="model-option-a"]')?.getAttribute("data-state")).toBe("checked");
    pick("b");
    expect(setRules).toHaveBeenLastCalledWith([{ windows: [], models: { default: ["a", "b"], opus: ["c"] } }]);
    fireEvent.click(container.querySelector('[data-testid="chip-remove-c"]')!);
    expect(setRules).toHaveBeenLastCalledWith([{ windows: [], models: { default: ["a"] } }]);
  });
});
