import { describe, it, expect } from "vitest";
import { render, screen } from "../../test/render";
import { PiUnsupportedNote } from "./PiUnsupportedNote";

// 测试 i18n 实例回退返回 key 本身（见 src/test/render.tsx），所以断言 key 不断言译文。
describe("PiUnsupportedNote", () => {
  it("渲染 pi 图标、标题 key 与调用方传入的原因 key", () => {
    const { container } = render(
      <PiUnsupportedNote reasonKey="pi.noMcp" reasonFallback="pi 没有 MCP 协议" />,
    );
    expect(container.querySelector('img[alt="pi"]')).not.toBeNull();
    expect(screen.getByText("pi.unsupportedTitle")).toBeInTheDocument();
    expect(screen.getByText(/pi\.noMcp/)).toBeInTheDocument();
  });

  it("原因 key 逐调用点独立（MCP / Hooks / cc-switch 三处不串味）", () => {
    const { rerender } = render(
      <PiUnsupportedNote reasonKey="pi.noHooks" reasonFallback="f" />,
    );
    expect(screen.getByText(/pi\.noHooks/)).toBeInTheDocument();
    rerender(<PiUnsupportedNote reasonKey="pi.noStatusline" reasonFallback="f" />);
    expect(screen.getByText(/pi\.noStatusline/)).toBeInTheDocument();
  });
});
