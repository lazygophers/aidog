import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { usePersistedNav } from "./lastNav";

const VALID = ["home", "platforms", "settings/tray"];
let setNav: (id: string) => void;

function Harness() {
  const [nav, set] = usePersistedNav(VALID, "home");
  setNav = set;
  return <div data-testid="nav" data-state={nav} />;
}
const state = () => screen.getByTestId("nav").getAttribute("data-state");

beforeEach(() => localStorage.clear());

describe("usePersistedNav", () => {
  it("读不到 → 回落默认首页", () => {
    render(<Harness />);
    expect(state()).toBe("home");
  });

  it("切页后重建（卸载再挂载）读回该页", () => {
    const first = render(<Harness />);
    act(() => setNav("settings/tray"));
    first.unmount();
    render(<Harness />);
    expect(state()).toBe("settings/tray");
  });

  it("旧 id 经别名表恢复到新 id", () => {
    localStorage.setItem("aidog.lastNav", "old/x");
    function H() {
      const [nav] = usePersistedNav(["home", "new-x"], "home", { "old/x": "new-x" });
      return <div data-testid="nav2" data-state={nav} />;
    }
    render(<H />);
    expect(screen.getByTestId("nav2").getAttribute("data-state")).toBe("new-x");
  });

  it("存的值不在合法清单（页被删/篡改）→ 回落默认", () => {
    localStorage.setItem("aidog.lastNav", "gone-page");
    render(<Harness />);
    expect(state()).toBe("home");
  });
});
