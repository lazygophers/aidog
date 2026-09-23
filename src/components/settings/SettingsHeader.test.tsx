// 近黑强调色那一轮的护栏：选中态不能用 --primary 当**前景**。
//
// --primary 映射 token `accent`，深色下是近黑 #101012；拿它当字色 / 竖条色，
// 选中项就是近黑压在近黑底上（用户实机报的「侧栏选中项整条看不见」就是这个）。
// 前景走 --accent（token `accent-text`），描边 / 竖条走 --accent-edge。
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "../../test/render";
import { SettingsHeader } from "./SettingsHeader";

describe("SettingsHeader 的 mode 切换按钮", () => {
  it("选中态的字色与底部竖条都不取 --primary", () => {
    render(
      <SettingsHeader
        mode="gui"
        onModeChange={vi.fn()}
        search=""
        onSearchChange={vi.fn()}
        onLoadRecommended={vi.fn()}
        onImport={vi.fn()}
        onSave={vi.fn()}
        saving={false}
        toast=""
        dirty={false}
      />,
    );
    const active = screen.getAllByRole("button").find(
      (b) => b.style.color !== "" && b.style.color.includes("--accent"),
    );
    expect(active).toBeTruthy();
    expect(active!.style.color).not.toContain("var(--primary)");
    expect(active!.style.boxShadow).toContain("var(--accent-edge)");
    expect(active!.style.boxShadow).not.toContain("var(--primary)");
  });
});
