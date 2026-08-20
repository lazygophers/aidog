import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen } from "../../test/render";
import userEvent from "@testing-library/user-event";
import { CopyButton } from "./CopyButton";

// 复制走 Tauri writeText（不是 navigator.clipboard —— WKWebView 无手势激活时会静默失败）。
const writeText = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({ writeText }));

beforeEach(() => writeText.mockClear());
// 用 fake timers 的用例若中途失败会把假时钟留给后续用例，让 userEvent 整体挂死。
afterEach(() => vi.useRealTimers());

describe("CopyButton 直接复制模式", () => {
  it("点击把 text 写进剪贴板", async () => {
    const user = userEvent.setup();
    render(<CopyButton text="hello" />);
    await user.click(screen.getByRole("button"));
    expect(writeText).toHaveBeenCalledWith("hello");
  });

  it("复制成功后显示对勾，1500ms 后复原", async () => {
    const user = userEvent.setup();
    const { container } = render(<CopyButton text="hello" />);
    const strokeOf = () => container.querySelector("svg")!.getAttribute("stroke");

    expect(strokeOf()).toBe("currentColor");
    await user.click(screen.getByRole("button"));
    await vi.waitFor(() => expect(strokeOf()).toBe("var(--color-success)"));
    await vi.waitFor(() => expect(strokeOf()).toBe("currentColor"), { timeout: 3000 });
  });

  it("title 缺省回落到 text", () => {
    const { rerender } = render(<CopyButton text="the-text" />);
    expect(screen.getByRole("button")).toHaveAttribute("title", "the-text");
    rerender(<CopyButton text="the-text" title="自定义" />);
    expect(screen.getByRole("button")).toHaveAttribute("title", "自定义");
  });

  it("label 渲染文字，icon 覆盖默认 SVG 且抑制 label", () => {
    const { container, rerender } = render(<CopyButton text="t" label="复制" />);
    expect(screen.getByText("复制")).toBeInTheDocument();

    rerender(<CopyButton text="t" label="复制" icon={<i data-testid="ico" />} />);
    expect(screen.getByTestId("ico")).toBeInTheDocument();
    expect(screen.queryByText("复制")).not.toBeInTheDocument();
    expect(container.querySelector("svg")).toBeNull();
  });

  it("点击不冒泡到父容器（卡片内的复制按钮不触发卡片点击）", async () => {
    const user = userEvent.setup();
    const onParent = vi.fn();
    render(
      <div onClick={onParent}>
        <CopyButton text="t" />
      </div>,
    );
    await user.click(screen.getByRole("button"));
    expect(onParent).not.toHaveBeenCalled();
  });
});

describe("CopyButton 菜单模式", () => {
  const menu = [
    { key: "a", label: "复制 A", text: "text-a" },
    { key: "b", label: "复制 B", text: "text-b" },
  ];

  it("点击 trigger 不直接复制（由菜单项决定复制哪一条）", async () => {
    const user = userEvent.setup();
    render(<CopyButton text="ignored" menu={menu} defaultLabel="默认" />);
    await user.click(screen.getByRole("button"));
    expect(writeText).not.toHaveBeenCalledWith("ignored");
  });

  it("hover 打开菜单并切到 hoverLabel，选中项复制自己的 text", async () => {
    const user = userEvent.setup();
    render(<CopyButton text="ignored" menu={menu} defaultLabel="默认" hoverLabel="悬浮" />);

    expect(screen.getByText("默认")).toBeInTheDocument();
    await user.hover(screen.getByRole("button"));
    expect(await screen.findByText("悬浮")).toBeInTheDocument();

    await user.click(await screen.findByText("复制 B"));
    expect(writeText).toHaveBeenCalledWith("text-b");
  });

  it("缺 hoverLabel 时 hover 仍显示 defaultLabel", async () => {
    const user = userEvent.setup();
    render(<CopyButton text="t" menu={menu} defaultLabel="默认" />);
    await user.hover(screen.getByRole("button"));
    expect(screen.getByText("默认")).toBeInTheDocument();
  });
});
