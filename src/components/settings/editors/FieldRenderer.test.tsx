import { describe, it, expect, vi } from "vitest";
import { render } from "../../../test/render";
import { fireEvent, screen } from "@testing-library/react";
import { FieldRenderer } from "./FieldRenderer";

const mk = (value: string, onChange = vi.fn()) =>
  render(
    <FieldRenderer
      field={{ key: "outputStyle", label: "Output Style", type: "string", options: ["default", "Mine"] }}
      value={value}
      onChange={onChange}
      t={((k: string) => k) as never}
    />,
  );

describe("FieldRenderer string+options 完整下拉", () => {
  it("已有值时打开下拉仍列出全部选项，当前值标 selected", () => {
    mk("Mine");
    fireEvent.focus(document.querySelector("#cb-outputStyle")!);
    expect(screen.getByTestId("cb-opt-outputStyle-default")).toBeTruthy();
    expect(screen.getByTestId("cb-opt-outputStyle-Mine").getAttribute("data-state")).toBe("selected");
  });

  it("输入自定义值可提交；点选项提交该项", () => {
    const onChange = vi.fn();
    mk("Mine", onChange);
    const input = document.querySelector("#cb-outputStyle")!;
    fireEvent.change(input, { target: { value: "custom-x" } });
    expect(onChange).toHaveBeenLastCalledWith("custom-x");
    fireEvent.focus(input);
    fireEvent.mouseDown(screen.getByTestId("cb-opt-outputStyle-default"));
    expect(onChange).toHaveBeenLastCalledWith("default");
  });

  it("键盘：下选 + 回车确认", () => {
    const onChange = vi.fn();
    mk("", onChange);
    const input = document.querySelector("#cb-outputStyle")!;
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenLastCalledWith("Mine");
  });
});
