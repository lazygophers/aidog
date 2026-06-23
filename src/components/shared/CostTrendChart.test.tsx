import { describe, it, expect } from "vitest";
import { render } from "../../test/render";
import { fireEvent } from "@testing-library/react";
import { CostTrendChart } from "./CostTrendChart";
import type { StatsBucket } from "../../services/api";

function bucket(time_bucket: string, total_cost: number): StatsBucket {
  return { time_bucket, total_cost } as StatsBucket;
}

describe("CostTrendChart", () => {
  it("renders nothing for empty buckets", () => {
    const { container } = render(<CostTrendChart buckets={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders last bucket value and an svg path for multiple buckets", () => {
    const buckets = [
      bucket("2026-06-20 10:00", 0.5),
      bucket("2026-06-20 11:00", 1.2),
      bucket("2026-06-20 12:00", 0.8),
      bucket("2026-06-20 13:00", 2.0),
    ];
    const { container } = render(<CostTrendChart buckets={buckets} />);
    // 末点金额展示（$2.00），value 与 bucket span 同 div，取 textContent。
    expect(container.querySelector(".popover-trend-value")?.textContent).toContain("$2.00");
    expect(container.querySelector("svg")).toBeTruthy();
    expect(container.querySelectorAll("path").length).toBeGreaterThan(0);
  });

  it("renders single-bucket chart (degenerate xAt path)", () => {
    const { container } = render(
      <CostTrendChart buckets={[bucket("2026-06-20 10:00", 0.3)]} />,
    );
    expect(container.querySelector(".popover-trend-value")?.textContent).toContain("$0.3");
    expect(container.querySelector("svg")).toBeTruthy();
  });

  it("updates shown value on hover and resets on mouse leave", () => {
    const buckets = [
      bucket("2026-06-20 10:00", 0.5),
      bucket("2026-06-20 11:00", 1.2),
      bucket("2026-06-20 12:00", 0.8),
    ];
    const { container } = render(<CostTrendChart buckets={buckets} />);
    const rects = container.querySelectorAll("rect");
    expect(rects.length).toBe(3);
    const valueEl = container.querySelector(".popover-trend-value")!;
    fireEvent.mouseEnter(rects[0]);
    expect(valueEl.textContent).toContain("$0.5");
    const root = container.querySelector(".popover-trend-chart")!;
    fireEvent.mouseLeave(root);
    // 回到末点
    expect(valueEl.textContent).toContain("$0.8");
  });
});
