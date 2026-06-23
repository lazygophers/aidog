import { describe, it, expect } from "vitest";
import { render, screen } from "../../test/render";
import { BalanceBar } from "./BalanceBar";

describe("BalanceBar", () => {
  it("renders nothing when remaining is null/undefined/NaN", () => {
    const { container: c1 } = render(<BalanceBar remaining={null} />);
    expect(c1.firstChild).toBeNull();
    const { container: c2 } = render(<BalanceBar remaining={undefined} />);
    expect(c2.firstChild).toBeNull();
    const { container: c3 } = render(<BalanceBar remaining={NaN} />);
    expect(c3.firstChild).toBeNull();
  });

  it("renders value only when no total (no progress bar)", () => {
    render(<BalanceBar remaining={12.3} />);
    expect(screen.getByText(/12\.30/)).toBeInTheDocument();
  });

  it("renders value + total + progress bar with custom currency", () => {
    render(<BalanceBar remaining={20} total={50} currency="¥" />);
    expect(screen.getByText("¥20.00")).toBeInTheDocument();
    expect(screen.getByText(/¥50\.00/)).toBeInTheDocument();
  });

  it("hides total when showTotal=false", () => {
    render(<BalanceBar remaining={20} total={50} showTotal={false} />);
    expect(screen.queryByText(/\/ \$50\.00/)).not.toBeInTheDocument();
  });

  it("respects explicit level prop", () => {
    const { container } = render(<BalanceBar remaining={5} total={100} level="success" />);
    // bar's filled segment uses the success color
    const span = screen.getByText("$5.00");
    expect(span).toHaveStyle({ color: "var(--color-success)" });
    expect(container.firstChild).not.toBeNull();
  });

  it("auto-derives level from remaining pct (danger when low)", () => {
    const span = render(<BalanceBar remaining={5} total={100} />).container;
    expect(span).not.toBeNull();
  });

  it("clamps pct and treats non-positive total as no-total", () => {
    render(<BalanceBar remaining={10} total={0} />);
    expect(screen.getByText("$10.00")).toBeInTheDocument();
  });
});
