// 时段档各自的启用开关（票 时段档-模型配置对齐/03）：渲染真组件，选择器锚 data-testid / data-state，不用文案。
import { render, fireEvent } from "../../test/render";
import { describe, it, expect, vi } from "vitest";
vi.mock("../../domains/platforms/modelPrices", () => ({ useModelPrices: () => new Map() }));
import { ModelsMatrixSection } from "./ModelsMatrixSection";

const base = { embedding: [], rerank: [], default: [], sonnet: [], opus: [], haiku: [], gpt: [], jev: [] };
const peakWindow = [{ start_hour: 9, end_hour: 12, multiplier: 2 }];

function mountTime(over: Record<string, unknown>) {
  const setRules = vi.fn();
  const r = render(
    <ModelsMatrixSection
      columns="time"
      models={base as never} onModelsChange={() => {}}
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
const sw = (slot: string) => document.body.querySelector(`[data-testid="time-slot-enabled-${slot}"]`) as HTMLElement;

describe("时段档行的启用开关", () => {
  it("8 个内置槽各有开关，data-state 反映本档 disabled_slots", () => {
    mountTime({ rules: [{ windows: [], models: {}, disabled_slots: ["opus"] }] });
    for (const s of ["default", "sonnet", "opus", "haiku", "gpt", "jev", "embedding", "rerank"]) {
      expect(sw(s)).not.toBeNull();
    }
    expect(sw("opus").getAttribute("data-state")).toBe("unchecked");
    expect(sw("default").getAttribute("data-state")).toBe("checked");
  });
  it("点击开关 → setRules 写回本档 disabled_slots（关 = 追加，开 = 移除）", () => {
    const { setRules } = mountTime({ rules: [{ windows: [], models: {}, disabled_slots: ["opus"] }] });
    fireEvent.click(sw("default"));
    expect(setRules).toHaveBeenLastCalledWith([{ windows: [], models: {}, disabled_slots: ["opus", "default"] }]);
    fireEvent.click(sw("opus"));
    expect(setRules).toHaveBeenLastCalledWith([{ windows: [], models: {}, disabled_slots: [] }]);
  });
  it("本档开关与平台级开关互不影响：平台级关 opus 不改变本档显示", () => {
    mountTime({ disabledSlots: ["opus"], rules: [{ windows: [], models: {}, disabled_slots: [] }] });
    expect(sw("opus").getAttribute("data-state")).toBe("checked");
  });
});

describe("时段档整档开关（enabled）", () => {
  const q = (id: string) => document.body.querySelector(`[data-testid="${id}"]`) as HTMLElement;
  it("关掉写出 enabled:false 且档变淡，开着不写该键", () => {
    const { setRules } = mountTime({ rules: [
      { windows: [], models: {} },
      { windows: [], models: {}, enabled: false },
    ] });
    expect(q("tw-0").getAttribute("data-state")).toBe("on");
    expect(q("tw-1").getAttribute("data-state")).toBe("off");
    fireEvent.click(q("tw-0-enabled"));
    expect(JSON.parse(JSON.stringify(setRules.mock.calls.at(-1)![0]))[0]).toEqual({ windows: [], models: {}, enabled: false });
    fireEvent.click(q("tw-1-enabled"));
    expect("enabled" in JSON.parse(JSON.stringify(setRules.mock.calls.at(-1)![0]))[1]).toBe(false);
  });
  it("点开关不冒泡选中该档：当前选中档不变", () => {
    mountTime({ rules: [{ windows: [], models: {} }, { windows: [], models: {} }] });
    expect(q("tw-0").getAttribute("aria-current")).toBe("true");
    fireEvent.click(q("tw-1-enabled"));
    expect(q("tw-0").getAttribute("aria-current")).toBe("true");
    expect(q("tw-1").getAttribute("aria-current")).toBeNull();
  });
});

describe("新建档开关按「模型」tab 当前内置槽开关预填", () => {
  it("「添加时段档」：预填平台级内置槽开关，排除自定义槽名", () => {
    const { container, setRules } = mountTime({ disabledSlots: ["opus", "fast", "embedding"] });
    fireEvent.click(container.querySelector('[data-testid="time-rule-add"]') as HTMLElement);
    expect(setRules).toHaveBeenLastCalledWith([{ windows: [], models: {}, disabled_slots: ["opus", "embedding"] }]);
  });
  it("「从高峰时段导入」：同样预填", () => {
    const { container, setRules } = mountTime({ peak: peakWindow, disabledSlots: ["gpt", "fast"] });
    fireEvent.click(container.querySelector('[data-testid="time-rule-import-peak"]') as HTMLElement);
    fireEvent.click(document.body.querySelector('[data-testid="time-rule-import-confirm"]') as HTMLElement);
    const rules = setRules.mock.calls.at(-1)![0];
    expect(rules).toHaveLength(1);
    expect(rules[0].disabled_slots).toEqual(["gpt"]);
    expect(rules[0].models).toEqual({});
  });
});
