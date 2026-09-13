import { describe, it, expect } from "vitest";
import { render, screen } from "../../../test/render";
import { PieChart } from "../PieChart";

const data = [
  { name: "claude", value: 5 },
  { name: "gpt", value: 3 },
  { name: "glm", value: 2 },
];

describe("PieChart", () => {
  it("renders one sector per positive-value slice", () => {
    const { container } = render(<PieChart data={data} animate={false} />);
    expect(container.querySelector("svg")).toBeTruthy();
    expect(container.querySelectorAll(".recharts-pie-sector").length).toBe(3);
  });

  it("renders honest empty state when no positive values", () => {
    render(<PieChart data={[{ name: "x", value: 0 }]} />);
    expect(screen.getByText("charts.noData")).toBeTruthy();
  });

  it("first slice takes amber primary, rest grayscale palette", () => {
    const { container } = render(<PieChart data={data} animate={false} />);
    const fills = Array.from(container.querySelectorAll(".recharts-pie-sector path")).map(
      (el) => el.getAttribute("fill"),
    );
    expect(fills[0]).toBe("var(--primary)");
    expect(fills[1]).toBe("var(--chart-2)");
    expect(fills[2]).toBe("var(--chart-3)");
  });
});
