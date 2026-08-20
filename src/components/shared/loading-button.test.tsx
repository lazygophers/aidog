import { describe, it, expect, vi } from "vitest";
import { createRef } from "react";
import { render, screen } from "../../test/render";
import userEvent from "@testing-library/user-event";
import { LoadingButton } from "./loading-button";

describe("LoadingButton", () => {
  it("loading 时禁用并插入 spinner，children 仍在", () => {
    const { container } = render(<LoadingButton loading>保存</LoadingButton>);
    expect(screen.getByRole("button")).toBeDisabled();
    expect(screen.getByText("保存")).toBeInTheDocument();
    expect(container.querySelector("svg")).not.toBeNull();
  });

  it("非 loading 时可点，无 spinner", async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    const { container } = render(<LoadingButton onClick={onClick}>保存</LoadingButton>);
    expect(container.querySelector("svg")).toBeNull();
    await user.click(screen.getByRole("button"));
    expect(onClick).toHaveBeenCalledOnce();
  });

  it("disabled 与 loading 各自都能禁用", () => {
    const { rerender } = render(<LoadingButton disabled>x</LoadingButton>);
    expect(screen.getByRole("button")).toBeDisabled();
    rerender(<LoadingButton>x</LoadingButton>);
    expect(screen.getByRole("button")).toBeEnabled();
  });

  it("点击时生成 ripple 波纹", async () => {
    const user = userEvent.setup();
    render(<LoadingButton>x</LoadingButton>);
    const btn = screen.getByRole("button");
    btn.getBoundingClientRect = () => ({ width: 60, height: 30, left: 0, top: 0 }) as DOMRect;
    await user.click(btn);
    expect(btn.querySelector(".ripple-wave")).not.toBeNull();
  });

  it("透传 className 与 ref", () => {
    const ref = createRef<HTMLButtonElement>();
    render(<LoadingButton ref={ref} className="extra">x</LoadingButton>);
    expect(ref.current).toBe(screen.getByRole("button"));
    expect(screen.getByRole("button")).toHaveClass("extra", "ripple");
  });

  it("无 onClick 时点击不抛", async () => {
    const user = userEvent.setup();
    render(<LoadingButton>x</LoadingButton>);
    await expect(user.click(screen.getByRole("button"))).resolves.toBeUndefined();
  });
});
