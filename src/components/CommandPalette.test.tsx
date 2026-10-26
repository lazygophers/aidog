// CommandPalette 就近测试：过滤 / 键盘选择 / 跳转 / 空态。
// 选择器锚 testid + data-state；文案断言 i18n key（test/render 回退 key）。
import { describe, it, expect, vi } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render, screen } from "../test/render";
import type { NavItem } from "./Sidebar";
import { CommandPalette } from "./CommandPalette";

const navItems = [
  { id: "home", icon: "home", labelKey: "nav.home", section: "s1" },
  { id: "platforms", icon: "platforms", labelKey: "nav.platforms", section: "s2" },
  { id: "settings", icon: "settings", labelKey: "nav.settings", section: "s3", children: [
    { id: "settings/tray", labelKey: "appSettings.trayTab", group: "g" },
  ] },
] as never;

const baseProps = {
  open: true,
  onOpenChange: vi.fn(),
  navItems,
  activeId: "home",
  onNavigate: vi.fn(),
};

describe("CommandPalette — ⌘K", () => {
  it("列出全部可跳项（含 settings 子页），输入过滤", () => {
    render(<CommandPalette {...baseProps} />);
    expect(document.querySelector('[data-nav-id="home"]')).toBeTruthy();
    expect(document.querySelector('[data-nav-id="settings/tray"]')).toBeTruthy();
    fireEvent.change(document.querySelector('[data-testid="command-palette-input"]')!, { target: { value: "nav.plat" } });
    expect(document.querySelector('[data-nav-id="home"]')).toBeNull();
    expect(document.querySelector('[data-nav-id="platforms"]')).toBeTruthy();
  });

  it("Enter 跳转首个命中并关闭；↑↓ 移动选中", () => {
    const onNavigate = vi.fn();
    render(<CommandPalette {...baseProps} onNavigate={onNavigate} />);
    const input = document.querySelector('[data-testid="command-palette-input"]')!;
    fireEvent.keyDown(input, { key: "ArrowDown" });
    // index=1 → platforms
    expect(document.querySelector('[data-nav-id="platforms"]')!.dataset.state).toBe("selected");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onNavigate).toHaveBeenCalledWith("platforms");
    expect(baseProps.onOpenChange).toHaveBeenCalledWith(false);
  });

  it("空态", () => {
    render(<CommandPalette {...baseProps} />);
    fireEvent.change(document.querySelector('[data-testid="command-palette-input"]')!, { target: { value: "zzz-no-hit" } });
    expect(screen.getByTestId("command-palette-empty").textContent).toBe("topbar.paletteNoResults");
  });
});
