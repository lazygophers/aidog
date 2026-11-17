// Switch 就近测试：选中态滑块在 RTL 下向左移（向右会压住相邻文字）。
import { describe, expect, it } from "vitest";
import { render, screen } from "../../test/render";
import { Switch } from "./switch";

describe("Switch", () => {
  it("moves the checked thumb toward the inline end in both directions", () => {
    render(<Switch data-testid="sw" checked onCheckedChange={() => {}} />);
    const thumb = screen.getByTestId("sw").firstElementChild as HTMLElement;
    // :dir() follows the nearest dir attribute, so an ltr island (Radix Tabs) inside an rtl page stays ltr.
    expect(thumb.className).toContain("[&:dir(rtl)]:data-[state=checked]:-translate-x-4");
  });
});
