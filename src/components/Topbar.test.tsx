// G3 骨架就近测试（票 02）：Topbar 面包屑/主题切换 + Sidebar 折叠两档/持久化。
// 选择器锚稳定 id + data-state；文案断言用 i18n key（test/render 空 resources 回退 key）。
import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render, screen } from "../test/render";

// useApp 依赖 AppContext（locale/theme 全局单例）——mock 掉换可控 themeMode。
vi.mock("../context/AppContext", () => ({
  useApp: () => ({
    locale: "zh-Hans",
    setLocale: vi.fn(),
    themeMode: (globalThis as { __testTheme?: string }).__testTheme ?? "dark",
    toggleMode: () => { (globalThis as { __toggled?: boolean }).__toggled = true; },
  }),
}));

import { Topbar } from "./Topbar";
import { Sidebar } from "./Sidebar";

describe("Topbar — G3 骨架（票 02）", () => {
  beforeEach(() => { delete (globalThis as { __toggled?: boolean }).__toggled; });

  it("面包屑 section / page + ⌘K 占位 + 通知铃", () => {
    const onNotif = vi.fn();
    render(<Topbar crumb={["nav.section.logStats", "nav.logs"]} onOpenNotifications={onNotif} />);
    expect(screen.getByText("nav.section.logStats")).toBeTruthy();
    expect(document.getElementById("g3-topbar-crumb-page")!.textContent).toBe("nav.logs");
    expect(document.getElementById("g3-topbar-search")).toBeTruthy();
    fireEvent.click(document.getElementById("g3-topbar-bell")!);
    expect(onNotif).toHaveBeenCalled();
  });

  it("主题切换按钮调 toggleMode", () => {
    render(<Topbar crumb={["", "nav.home"]} />);
    fireEvent.click(document.getElementById("g3-topbar-theme")!);
    expect((globalThis as { __toggled?: boolean }).__toggled).toBe(true);
  });

  it("notifHidden 时通知铃不渲染", () => {
    render(<Topbar crumb={["", "nav.home"]} notifHidden />);
    expect(document.getElementById("g3-topbar-bell")).toBeNull();
  });
});

describe("Sidebar — 折叠两档（票 02）", () => {
  beforeEach(() => {
    try { localStorage.removeItem("aidog.sidebar.collapsed"); } catch { /* noop */ }
  });

  const items = [{ id: "home", icon: "home", labelKey: "nav.home", section: "nav.section.overview" }];

  it("默认展开 216；点击折叠 → 56 + data-state + localStorage 记忆", () => {
    const { unmount } = render(<Sidebar navItems={items} activeId="home" onNavigate={vi.fn()} />);
    const aside = document.getElementById("g3-sidebar")!;
    expect(aside.getAttribute("data-state")).toBe("expanded");
    expect((aside as HTMLElement).style.width).toBe("216px");
    fireEvent.click(document.getElementById("g3-sidebar-collapse")!);
    const aside2 = document.getElementById("g3-sidebar")!;
    expect(aside2.getAttribute("data-state")).toBe("collapsed");
    expect((aside2 as HTMLElement).style.width).toBe("56px");
    expect(localStorage.getItem("aidog.sidebar.collapsed")).toBe("1");
    unmount();
  });

  it("localStorage=1 → 挂载即折叠态", () => {
    localStorage.setItem("aidog.sidebar.collapsed", "1");
    render(<Sidebar navItems={items} activeId="home" onNavigate={vi.fn()} />);
    expect(document.getElementById("g3-sidebar")!.getAttribute("data-state")).toBe("collapsed");
  });
});
