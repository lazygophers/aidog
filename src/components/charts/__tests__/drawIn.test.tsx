import { describe, it, expect, vi, afterEach } from "vitest";
import { useRef } from "react";
import { render, screen, waitFor } from "../../../test/render";
import { drawInProps, useDrawInPath } from "../drawIn";

/** ref 回调先于 effect 跑：在这里 stub getTotalLength。stub=undefined 时不挂（走无函数分支）。 */
function Probe({ stub }: { stub?: number }) {
  const ref = useRef<SVGPathElement>(null);
  useDrawInPath(ref, 100);
  return (
    <svg>
      <path
        ref={(el) => {
          if (el && stub !== undefined) (el as SVGPathElement).getTotalLength = () => stub;
          ref.current = el;
        }}
        data-testid="p"
      />
    </svg>
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("drawInProps", () => {
  it("returns recharts animation props with default 700ms", () => {
    expect(drawInProps()).toEqual({ isAnimationActive: true, animationBegin: 0, animationDuration: 700 });
  });

  it("honours custom duration", () => {
    expect(drawInProps(300).animationDuration).toBe(300);
  });
});

describe("useDrawInPath", () => {
  it("sets dasharray/offset to path length, then animates offset to 0", async () => {
    const { container } = render(<Probe stub={120} />);
    const path = container.querySelector("path")!;
    expect(path.style.strokeDasharray).toBe("120");
    expect(path.style.strokeDashoffset).toBe("120");
    await waitFor(() => expect(path.style.strokeDashoffset).toBe("0"));
    expect(path.style.transition).toContain("stroke-dashoffset 100ms");
  });

  it("skips when getTotalLength is unavailable (test env / exotic element)", () => {
    const { container } = render(<Probe />);
    const path = container.querySelector("path")!;
    expect(path.style.strokeDasharray).toBe("");
  });

  it("skips zero-length path", () => {
    const { container } = render(<Probe stub={0} />);
    const path = container.querySelector("path")!;
    expect(path.style.strokeDasharray).toBe("");
  });

  it("no crash when ref never attaches (null ref branch)", () => {
    function NoPath() {
      const ref = useRef<SVGPathElement>(null);
      useDrawInPath(ref);
      return <svg data-testid="empty" />;
    }
    render(<NoPath />);
    expect(screen.getByTestId("empty")).toBeTruthy();
  });
});
