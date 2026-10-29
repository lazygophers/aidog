// 侧栏 section 折叠：折叠后节头必须仍在，才能再点展开。
import { describe, it, expect, vi } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render, screen } from "../test/render";

vi.mock("../context/AppContext", () => ({
  useApp: () => ({ locale: "zh-Hans", setLocale: vi.fn() }),
}));

import { Sidebar } from "./Sidebar";

const items = [
  { id: "home", icon: "home", labelKey: "nav.home", section: "nav.section.overview" },
  { id: "logs", icon: "logs", labelKey: "nav.logs", section: "nav.section.logStats" },
  { id: "about", icon: "about", labelKey: "nav.about", section: "nav.section.system" },
];

describe("Sidebar — section 折叠/再展开", () => {
  it("折叠非 active 的 section 后节头仍可见，再点一次恢复展开", () => {
    render(<Sidebar navItems={items} activeId="home" onNavigate={vi.fn()} />);
    const sel = "nav-section-nav.section.system";
    expect(screen.getByTestId(sel).getAttribute("aria-expanded")).toBe("true");
    fireEvent.click(screen.getByTestId(sel));
    const header = screen.queryByTestId(sel);
    expect(header).not.toBeNull();
    expect(header!.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(header!);
    expect(screen.getByTestId(sel).getAttribute("aria-expanded")).toBe("true");
  });

  it("56px 图标态：节头隐藏，菜单项走图标态（title 带标签）；展开态反之", () => {
    try { localStorage.removeItem("aidog.sidebar.collapsed"); } catch { /* noop */ }
    render(<Sidebar navItems={items} activeId="home" onNavigate={vi.fn()} />);
    const sel = "nav-section-nav.section.system";
    expect(screen.queryByTestId(sel)).not.toBeNull();
    expect(screen.queryByTitle("nav.home")).toBeNull();
    fireEvent.click(document.getElementById("g3-sidebar-collapse")!);
    expect(document.getElementById("g3-sidebar")!.getAttribute("data-state")).toBe("collapsed");
    expect(screen.queryByTestId(sel)).toBeNull();
    expect(screen.queryByTitle("nav.home")).not.toBeNull();
    try { localStorage.removeItem("aidog.sidebar.collapsed"); } catch { /* noop */ }
  });

  it("图标态下已折叠 section 的菜单项仍渲染，切回展开态仍是折叠", () => {
    try { localStorage.removeItem("aidog.sidebar.collapsed"); } catch { /* noop */ }
    render(<Sidebar navItems={items} activeId="home" onNavigate={vi.fn()} />);
    const sel = "nav-section-nav.section.system";
    fireEvent.click(screen.getByTestId(sel));
    expect(screen.queryByTestId("nav-item-about")).toBeNull();
    fireEvent.click(document.getElementById("g3-sidebar-collapse")!);
    expect(screen.queryByTestId("nav-item-about")).not.toBeNull();
    fireEvent.click(document.getElementById("g3-sidebar-collapse")!);
    expect(screen.getByTestId(sel).getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByTestId("nav-item-about")).toBeNull();
    try { localStorage.removeItem("aidog.sidebar.collapsed"); } catch { /* noop */ }
  });
});
