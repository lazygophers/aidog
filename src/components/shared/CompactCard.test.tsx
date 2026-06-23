import { describe, it, expect, vi } from "vitest";
import { render, screen } from "../../test/render";
import userEvent from "@testing-library/user-event";
import { CompactCard } from "./CompactCard";

describe("CompactCard", () => {
  it("renders header", () => {
    render(<CompactCard header={<span>Hdr</span>} />);
    expect(screen.getByText("Hdr")).toBeInTheDocument();
  });

  it("no toggle button when no children", () => {
    render(<CompactCard header={<span>H</span>} />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("uncontrolled: toggles children visibility on click", async () => {
    const user = userEvent.setup();
    render(
      <CompactCard header={<span>H</span>} toggleLabel="toggle">
        <span>Detail</span>
      </CompactCard>,
    );
    expect(screen.queryByText("Detail")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button"));
    expect(screen.getByText("Detail")).toBeInTheDocument();
    await user.click(screen.getByRole("button"));
    expect(screen.queryByText("Detail")).not.toBeInTheDocument();
  });

  it("uncontrolled honours defaultExpanded", () => {
    render(
      <CompactCard header={<span>H</span>} defaultExpanded>
        <span>Detail</span>
      </CompactCard>,
    );
    expect(screen.getByText("Detail")).toBeInTheDocument();
  });

  it("controlled: uses expanded prop and calls onToggle", async () => {
    const user = userEvent.setup();
    const onToggle = vi.fn();
    const { rerender } = render(
      <CompactCard header={<span>H</span>} expanded={false} onToggle={onToggle}>
        <span>Detail</span>
      </CompactCard>,
    );
    expect(screen.queryByText("Detail")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button"));
    expect(onToggle).toHaveBeenCalledWith(true);
    rerender(
      <CompactCard header={<span>H</span>} expanded={true} onToggle={onToggle}>
        <span>Detail</span>
      </CompactCard>,
    );
    expect(screen.getByText("Detail")).toBeInTheDocument();
  });

  it("applies extra style prop", () => {
    const { container } = render(
      <CompactCard header={<span>H</span>} style={{ opacity: 0.5 }} />,
    );
    expect(container.firstChild).toHaveStyle({ opacity: "0.5" });
  });
});
