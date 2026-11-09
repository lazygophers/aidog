// 时段档空槽落点提示 + 「从模型配置复制」（票 时段档-模型配置对齐/04）：渲染真组件，选择器锚 data-testid / id + data-state。
import { render, fireEvent } from "../../test/render";
import { describe, it, expect, vi } from "vitest";
vi.mock("../../domains/platforms/modelPrices", () => ({ useModelPrices: () => new Map() }));
import { ModelsMatrixSection } from "./ModelsMatrixSection";

const empty = { embedding: [], rerank: [], default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [] };

function mountTime(over: Record<string, unknown>) {
  const setRules = vi.fn();
  const r = render(
    <ModelsMatrixSection
      columns="time"
      models={empty as never} onModelsChange={() => {}}
      activeDropdown={null} setActiveDropdown={() => {}} availableModels={[]}
      protocol={"anthropic" as never} fetchError="" fetching={false} onFillAll={() => {}}
      onFetchModels={() => {}} apiKeyMissing={false} endpointsCount={1}
      endpoints={[]}
      rules={[]} setRules={setRules} peak={[]}
      tzMode={"utc" as never} setTzMode={() => {}} t={((k: string) => k) as never}
      {...over}
    />,
  );
  return { setRules, ...r };
}
const q = (id: string) => document.body.querySelector(`[data-testid="${id}"]`) as HTMLElement | null;
const fb = (slot: string) => document.body.querySelector(`#time-slot-fallback-${slot}`);

describe("空聊天槽落点提示", () => {
  it("本档 default 有值 → 空槽落到 default 链首（data-state=default），有值的槽不显示", () => {
    mountTime({ rules: [{ windows: [], models: { default: ["m1", "m2"], opus: ["o1"] } }] });
    expect(fb("sonnet")?.getAttribute("data-state")).toBe("default");
    expect(fb("sonnet")?.getAttribute("data-model")).toBe("m1");
    expect(fb("opus")).toBeNull();
    expect(fb("default")).toBeNull();
  });
  it("本档 default 也空 → 透传请求模型名（data-state=passthrough）", () => {
    mountTime({ rules: [{ windows: [], models: { opus: ["o1"] } }] });
    expect(fb("sonnet")?.getAttribute("data-state")).toBe("passthrough");
    expect(fb("default")?.getAttribute("data-state")).toBe("passthrough");
  });
  it("embedding / rerank 不出聊天落点提示", () => {
    mountTime({ rules: [{ windows: [], models: {} }] });
    expect(fb("embedding")).toBeNull();
    expect(fb("rerank")).toBeNull();
  });
});

describe("从模型配置复制", () => {
  const models = { ...empty, default: ["d1"], opus: ["o1", "o2"], embedding: ["e1"] };
  it("本档全空 → 直接写入「模型」tab 内置槽链 + 内置槽开关，自定义槽不写，不弹确认", () => {
    const { setRules } = mountTime({
      models, disabledSlots: ["opus", "fast"],
      rules: [{ windows: [{ start_hour: 1, end_hour: 2, multiplier: 1 }], models: {}, disabled_slots: [] }],
    });
    fireEvent.click(q("time-rule-copy-models")!);
    expect(q("time-rule-copy-confirm")).toBeNull();
    expect(setRules).toHaveBeenLastCalledWith([{
      windows: [{ start_hour: 1, end_hour: 2, multiplier: 1 }],
      models: { default: ["d1"], opus: ["o1", "o2"], embedding: ["e1"] },
      disabled_slots: ["opus"],
    }]);
  });
  it("本档已有槽值 → 先弹确认，确认后覆盖整档", () => {
    const { setRules } = mountTime({ models, rules: [{ windows: [], models: { haiku: ["h"] }, disabled_slots: ["gpt"] }] });
    fireEvent.click(q("time-rule-copy-models")!);
    expect(setRules).not.toHaveBeenCalled();
    expect(q("time-rule-copy-confirm")).not.toBeNull();
    fireEvent.click(q("time-rule-copy-confirm")!);
    expect(setRules).toHaveBeenLastCalledWith([{
      windows: [], models: { default: ["d1"], opus: ["o1", "o2"], embedding: ["e1"] }, disabled_slots: [],
    }]);
  });
  it("确认框取消 → 档不变", () => {
    const { setRules } = mountTime({ models, rules: [{ windows: [], models: { haiku: ["h"] } }] });
    fireEvent.click(q("time-rule-copy-models")!);
    fireEvent.click(q("time-rule-copy-cancel")!);
    expect(setRules).not.toHaveBeenCalled();
    expect(q("time-rule-copy-confirm")).toBeNull();
  });
});
