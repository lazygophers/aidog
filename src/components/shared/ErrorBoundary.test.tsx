import { describe, it, expect, vi } from "vitest";
import { render, screen } from "../../test/render";
import { ErrorBoundary } from "./ErrorBoundary";

function Boom(): never {
  throw new Error("boom-render-crash");
}

// componentDidCatch 走 console.error（frontendLog 挂钩），测试里静音防噪声
vi.spyOn(console, "error").mockImplementation(() => {});

describe("ErrorBoundary", () => {
  it("子组件渲染抛错 → 渲染错误面板而非白屏，含消息全文", () => {
    render(
      <ErrorBoundary scope="page">
        <Boom />
      </ErrorBoundary>,
    );
    const detail = screen.getByTestId("error-boundary-detail");
    expect(detail.textContent).toContain("boom-render-crash");
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  it("正常子树原样渲染，不弹面板", () => {
    render(
      <ErrorBoundary scope="page">
        <div data-testid="ok-child">fine</div>
      </ErrorBoundary>,
    );
    expect(screen.getByTestId("ok-child")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("复制按钮携带错误全文（消息 + 组件栈）", () => {
    render(
      <ErrorBoundary scope="root">
        <Boom />
      </ErrorBoundary>,
    );
    const detail = screen.getByTestId("error-boundary-detail").textContent ?? "";
    expect(detail).toMatch(/boom-render-crash/);
    expect(detail).toMatch(/at \w+/); // 组件栈片段
    const btn = screen.getAllByRole("button").find((b) => b.getAttribute("title") === detail);
    expect(btn).toBeTruthy(); // CopyButton title 缺省回落到 text
  });
});
