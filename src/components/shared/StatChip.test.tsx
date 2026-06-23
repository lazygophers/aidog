import { describe, it, expect } from "vitest";
import { render, screen } from "../../test/render";
import { StatChip } from "./StatChip";

describe("StatChip", () => {
  it("renders value and label", () => {
    render(<StatChip value="1.2M" label="tokens" />);
    expect(screen.getByText("1.2M")).toBeInTheDocument();
    expect(screen.getByText("tokens")).toBeInTheDocument();
  });

  it("applies explicit color over level", () => {
    render(<StatChip value="x" label="y" color="red" level="danger" />);
    const val = screen.getByText("x");
    expect(val).toHaveStyle({ color: "rgb(255, 0, 0)" });
  });

  it("derives color from level when no explicit color", () => {
    render(<StatChip value="ok" label="status" level="success" />);
    expect(screen.getByText("ok")).toHaveStyle({ color: "var(--color-success)" });
  });

  it("falls back to primary text color with neither", () => {
    render(<StatChip value="z" label="lbl" />);
    expect(screen.getByText("z")).toHaveStyle({ color: "var(--text-primary)" });
  });

  it("renders the icon node when provided", () => {
    render(<StatChip value="v" label="l" icon={<i data-testid="ic" />} />);
    expect(screen.getByTestId("ic")).toBeInTheDocument();
  });
});
