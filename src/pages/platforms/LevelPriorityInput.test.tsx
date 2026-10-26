// 就近测试（frontend-refactor-tests 规则）：选择器只锚 data-testid，不锚文案/DOM 层级。
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { LevelPriorityInput } from "./PlatformRow";

describe("LevelPriorityInput", () => {
  const base = { testId: "row-level", onChange: vi.fn() };

  it("输入超界值失焦 clamp 到 1-10", () => {
    const onChange = vi.fn();
    render(<LevelPriorityInput {...base} onChange={onChange} value={5} />);
    const el = screen.getByTestId("row-level");
    fireEvent.change(el, { target: { value: "99" } });
    fireEvent.blur(el);
    expect(onChange).toHaveBeenCalledWith(10);
    expect(el).toHaveValue("10");
  });

  it("Esc 还原为原值且不提交", () => {
    const onChange = vi.fn();
    render(<LevelPriorityInput {...base} onChange={onChange} value={3} />);
    const el = screen.getByTestId("row-level");
    fireEvent.change(el, { target: { value: "7" } });
    fireEvent.keyDown(el, { key: "Escape" });
    expect(onChange).not.toHaveBeenCalled();
  });

  it("空输入失焦回落原值", () => {
    const onChange = vi.fn();
    render(<LevelPriorityInput {...base} onChange={onChange} value={4} />);
    const el = screen.getByTestId("row-level");
    fireEvent.change(el, { target: { value: "" } });
    fireEvent.blur(el);
    expect(onChange).not.toHaveBeenCalled();
    expect(el).toHaveValue("4");
  });
});
