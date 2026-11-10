// 限频规则列表：渲染 / 增删 / 条件校验 / 模型多选 + 手输 / 拖动句柄。选择器锚稳定 data-testid。
// 拖拽只断言 onDragEnd 之后的状态（落点判定列人工验收）。
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "../../test/render";

let dragEnd: ((e: unknown) => void) | undefined;
vi.mock("@dnd-kit/core", async (orig) => {
  const actual = await orig<typeof import("@dnd-kit/core")>();
  return {
    ...actual,
    DndContext: (p: { onDragEnd: (e: unknown) => void; children: React.ReactNode }) => {
      dragEnd = p.onDragEnd;
      return <>{p.children}</>;
    },
  };
});
vi.mock("../../domains/platforms/modelPrices", () => ({ useModelPrices: () => new Map() }));
import { RateLimitRulesList, type RateLimitRuleDraft } from "./RateLimitRules";

const t = ((key: string, fallback?: string) => fallback ?? key) as never;
const R = (over: Partial<RateLimitRuleDraft> = {}): RateLimitRuleDraft => ({
  id: "r0", models: [], start: "", end: "", tz: "", days: [], concurrency: "", rpm: "", tpm: "", ...over,
});

function mount(rules: RateLimitRuleDraft[], options: string[] = ["glm-4", "glm-5"]) {
  const onChange = vi.fn();
  const r = render(<RateLimitRulesList rules={rules} onChange={onChange} modelOptions={options} protocol={"glm" as never} t={t} />);
  const q = (id: string) => r.container.querySelector(`[data-testid="${id}"]`) as HTMLElement | null;
  const input = (id: string) => q(id) as HTMLInputElement;
  return { onChange, q, input, ...r };
}

describe("RateLimitRulesList", () => {
  it("按列表顺序渲染每条规则的时段 / 上限 / 模型 chip", () => {
    const { q, input } = mount([
      R({ id: "a", models: ["glm-4", "glm-*"], rpm: "60", tpm: "1000", concurrency: "3" }),
      R({ id: "b", start: "14", end: "18", tz: "Asia/Shanghai", days: [1, 2], rpm: "5" }),
    ]);
    expect(input("rl-rule-0-rpm").value).toBe("60");
    expect(input("rl-rule-0-tpm").value).toBe("1000");
    expect(input("rl-rule-0-concurrency").value).toBe("3");
    expect(q("rl-rule-0-chip-glm-4")).not.toBeNull();
    expect(q("rl-rule-0-chip-glm-*")).not.toBeNull();
    expect(input("rl-rule-1-start").value).toBe("14");
    expect(input("rl-rule-1-end").value).toBe("18");
    expect(input("rl-rule-1-tz").value).toBe("Asia/Shanghai");
    expect(q("rl-rule-1-day-1")?.getAttribute("data-state")).toBe("on");
    expect(q("rl-rule-1-day-0")?.getAttribute("data-state")).toBe("off");
    expect(q("rl-rule-0-handle")).not.toBeNull();
  });

  it("提示「没填的项在该规则生效时不限」与重叠按顺序", () => {
    const { q } = mount([R()]);
    expect(q("rl-rules-hint")?.textContent).toContain("没填的项在该规则生效时不限");
  });

  it("没有任何条件 → 行内报错；有模型或时段即消失", () => {
    const { q } = mount([R({ id: "a", rpm: "5" }), R({ id: "b", models: ["glm-4"] }), R({ id: "c", start: "1", end: "2" }), R({ id: "d", days: [1] })]);
    expect(q("rl-rule-0-error")?.getAttribute("data-state")).toBe("noCondition");
    expect(q("rl-rule-1-error")).toBeNull();
    expect(q("rl-rule-2-error")).toBeNull();
    expect(q("rl-rule-3-error")).toBeNull();
  });

  it("通配符只允许末尾单个 * 且前缀非空", () => {
    const { q } = mount([R({ id: "a", models: ["a*b"] }), R({ id: "b", models: ["*"] }), R({ id: "c", models: ["glm-*"] })]);
    expect(q("rl-rule-0-error")?.getAttribute("data-state")).toBe("badWildcard");
    expect(q("rl-rule-1-error")?.getAttribute("data-state")).toBe("badWildcard");
    expect(q("rl-rule-2-error")).toBeNull();
  });

  it("加规则：追加一条空规则", () => {
    const { q, onChange } = mount([R({ id: "a", models: ["m"] })]);
    fireEvent.click(q("rl-rules-add")!);
    const next = onChange.mock.calls[0][0] as RateLimitRuleDraft[];
    expect(next).toHaveLength(2);
    expect(next[0].id).toBe("a");
    expect(next[1]).toMatchObject({ models: [], start: "", end: "", tz: "", days: [], concurrency: "", rpm: "", tpm: "" });
    expect(next[1].id).not.toBe("a");
  });

  it("删规则 / 改上限 / 点星期", () => {
    const { q, input, onChange } = mount([R({ id: "a", models: ["m"] }), R({ id: "b", models: ["n"] })]);
    fireEvent.change(input("rl-rule-1-rpm"), { target: { value: "9" } });
    expect((onChange.mock.calls[0][0] as RateLimitRuleDraft[])[1].rpm).toBe("9");
    fireEvent.click(q("rl-rule-0-day-3")!);
    expect((onChange.mock.calls[1][0] as RateLimitRuleDraft[])[0].days).toEqual([3]);
    fireEvent.click(q("rl-rule-0-remove")!);
    expect((onChange.mock.calls[2][0] as RateLimitRuleDraft[]).map(r => r.id)).toEqual(["b"]);
  });

  it("模型手输：回车加入（含通配），重复不加；chip ✕ 移除", () => {
    const { q, input, onChange } = mount([R({ id: "a", models: ["glm-4"] })]);
    const inp = input("rl-rule-0-model-input");
    fireEvent.change(inp, { target: { value: "deepseek-*" } });
    fireEvent.keyDown(inp, { key: "Enter" });
    expect((onChange.mock.calls[0][0] as RateLimitRuleDraft[])[0].models).toEqual(["glm-4", "deepseek-*"]);
    fireEvent.change(inp, { target: { value: "glm-4" } });
    fireEvent.keyDown(inp, { key: "Enter" });
    expect(onChange).toHaveBeenCalledTimes(1);
    fireEvent.click(q("rl-rule-0-chip-remove-glm-4")!);
    expect((onChange.mock.calls[1][0] as RateLimitRuleDraft[])[0].models).toEqual([]);
  });

  it("下拉选项来自传入的模型配置，点选即加入", () => {
    const { input, onChange } = mount([R({ id: "a" })], ["glm-4", "glm-5"]);
    fireEvent.focus(input("rl-rule-0-model-input"));
    fireEvent.mouseDown(document.body.querySelector('[data-testid="model-option-glm-5"]')!);
    expect((onChange.mock.calls[0][0] as RateLimitRuleDraft[])[0].models).toEqual(["glm-5"]);
  });

  it("拖动落点后按新顺序回调", () => {
    const { onChange } = mount([R({ id: "a", models: ["x"] }), R({ id: "b", models: ["y"] })]);
    dragEnd?.({ active: { id: "a" }, over: { id: "b" } });
    expect((onChange.mock.calls[0][0] as RateLimitRuleDraft[]).map(r => r.id)).toEqual(["b", "a"]);
  });
});
