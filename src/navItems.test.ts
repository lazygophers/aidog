import { describe, it, expect } from "vitest";
import { BASE_NAV, NAV_IDS, NAV_ALIASES } from "./navItems";

describe("导航：中间件移到扩展", () => {
  it("扩展分区含一级中间件项，设置子菜单不再有", () => {
    const ext = BASE_NAV.filter((n) => n.section === "nav.section.extension").map((n) => n.id);
    expect(ext).toContain("middleware");
    const settings = BASE_NAV.find((n) => n.id === "settings")!;
    expect(settings.children!.map((c) => c.id)).not.toContain("settings/middleware");
    expect(NAV_IDS).not.toContain("settings/middleware");
  });

  it("旧 id 映射到合法的新 id", () => {
    expect(NAV_IDS).toContain(NAV_ALIASES["settings/middleware"]);
  });
});
