// 模型链编辑器（槽位 / 时段共用）：增 / 删 / 调序 / 去重 / 空值过滤。
// 选择器锚稳定 data-testid + data-state，不锚文案 / class / DOM 层级；拖拽只断言 onDragEnd 之后状态（落点判定列人工验收）。
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
import { ModelChainEditor } from "./ModelChainEditor";

const t = ((key: string, fallback?: string) => fallback ?? key) as never;

function mount(value: string[], source: string[] = ["a", "b", "c", "d"]) {
  const onChange = vi.fn();
  const r = render(
    <ModelChainEditor testId="default" value={value} onChange={onChange} source={source} protocol={"openai" as never} t={t} />,
  );
  const q = (id: string) => r.container.querySelector(`[data-testid="${id}"]`) as HTMLElement | null;
  return { onChange, q, ...r };
}
const optionIds = () =>
  Array.from(document.body.querySelectorAll('[data-testid^="model-option-"]')).map((e) => e.getAttribute("data-testid"));

describe("ModelChainEditor", () => {
  it("chip 按链序渲染，首个 primary 其余 backup", () => {
    const { q } = mount(["a", "b", "c"]);
    expect(q("chip-a")?.getAttribute("data-state")).toBe("primary");
    expect(q("chip-b")?.getAttribute("data-state")).toBe("backup");
    expect(q("chip-c")?.getAttribute("data-state")).toBe("backup");
  });

  it("链长 1：只有一个 primary chip，无 backup", () => {
    const { container } = mount(["a"]);
    expect(container.querySelectorAll('[data-state="primary"]')).toHaveLength(1);
    expect(container.querySelectorAll('[data-state="backup"]')).toHaveLength(0);
  });

  it("onDragEnd 后写出 arrayMove 顺序", () => {
    const { onChange } = mount(["a", "b", "c"]);
    dragEnd!({ active: { id: "a" }, over: { id: "c" } });
    expect(onChange).toHaveBeenCalledWith(["b", "c", "a"]);
  });

  it("删除 chip：只去掉目标，其余保序；删光写出空数组", () => {
    const { onChange, q } = mount(["a", "b", "c"]);
    fireEvent.click(q("chip-remove-b")!);
    expect(onChange).toHaveBeenLastCalledWith(["a", "c"]);
    const m = mount(["a"]);
    fireEvent.click(m.q("chip-remove-a")!);
    expect(m.onChange).toHaveBeenCalledWith([]);
  });

  const opt = (id: string) => document.body.querySelector(`[data-testid="model-option-${id}"]`) as HTMLElement;

  it("下拉保留已选并打勾，点未选候选追加到链尾且下拉不关", () => {
    const { onChange, q } = mount(["a", "c"]);
    fireEvent.focus(q("chain-input-default")!);
    expect(optionIds()).toEqual(["a", "b", "c", "d"].map((i) => `model-option-${i}`));
    expect(opt("a").getAttribute("data-state")).toBe("checked");
    expect(opt("b").getAttribute("data-state")).toBe("unchecked");
    fireEvent.mouseDown(opt("d"));
    expect(onChange).toHaveBeenCalledWith(["a", "c", "d"]);
    expect(optionIds()).toHaveLength(4);
  });

  it("点已勾选项从链中移除，下拉不关", () => {
    const { onChange, q } = mount(["a", "c"]);
    fireEvent.focus(q("chain-input-default")!);
    fireEvent.mouseDown(opt("a"));
    expect(onChange).toHaveBeenCalledWith(["c"]);
    expect(optionIds()).toHaveLength(4);
  });

  it("回车添加手输模型：trim；空白 / 重复不追加", () => {
    const { onChange, q } = mount(["a"]);
    const input = q("chain-input-default") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "  x-custom " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenLastCalledWith(["a", "x-custom"]);
    onChange.mockClear();
    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.change(input, { target: { value: "a" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).not.toHaveBeenCalled();
  });
});
