// ModelCombobox exclude 参数：已选模型不出现在候选，不传时行为不变。选择器锚 data-testid。
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "../../test/render";
import { ModelCombobox } from "./ModelCombobox";

vi.mock("../../domains/platforms/modelPrices", () => ({
  useModelPrices: () => new Map(),
}));

const t = ((key: string, fallback?: string) => fallback ?? key) as never;

function open(exclude?: string[]) {
  const { container } = render(
    <ModelCombobox
      value="" onChange={() => {}} onSelect={() => {}} source={["a", "b", "c"]}
      exclude={exclude} protocol="openai" t={t}
      inputProps={{ "data-testid": "cb-input" }}
    />,
  );
  fireEvent.focus(container.querySelector('[data-testid="cb-input"]')!);
}
const opts = () =>
  Array.from(document.body.querySelectorAll('[data-testid^="model-option-"]')).map((e) =>
    e.getAttribute("data-testid"));

describe("ModelCombobox exclude", () => {
  it("不传 exclude：全部候选", () => {
    open();
    expect(opts()).toEqual(["model-option-a", "model-option-b", "model-option-c"]);
  });
  it("exclude 内的候选被移除（精确比较）", () => {
    open(["b", "C"]);
    expect(opts()).toEqual(["model-option-a", "model-option-c"]);
  });
  it("候选全部被排除：不显示下拉按钮，聚焦也不弹出", () => {
    open(["a", "b", "c"]);
    expect(document.body.querySelector('[data-testid="model-combobox-toggle"]')).toBeNull();
    expect(opts()).toEqual([]);
  });
  it("仍有候选：显示下拉按钮", () => {
    open(["a"]);
    expect(document.body.querySelector('[data-testid="model-combobox-toggle"]')).not.toBeNull();
  });
});
