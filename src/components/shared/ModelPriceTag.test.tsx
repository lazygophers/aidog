import { describe, it, expect } from "vitest";
import { render, screen } from "../../test/render";
import { ModelPriceTag } from "./ModelPriceTag";

describe("ModelPriceTag", () => {
  it("info 为空不渲染", () => {
    const { container } = render(<ModelPriceTag info={null} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("free → 免费文案", () => {
    render(<ModelPriceTag info={{ free: true }} />);
    expect(screen.getByText("common.priceFree")).toBeInTheDocument();
  });

  it("unitLabel 优先于 token 四价", () => {
    render(<ModelPriceTag info={{ free: false, unitLabel: "$0.04/张", input: 1 }} />);
    expect(screen.getByText("$0.04/张")).toBeInTheDocument();
  });

  it("compact：单行 in/out；无价则空", () => {
    const { container, rerender } = render(<ModelPriceTag compact info={{ free: false, input: 3e-6, output: 15e-6 }} />);
    expect(container.textContent).toBe("in $3.00 · out $15.00");
    rerender(<ModelPriceTag compact info={{ free: false, cacheRead: 1 }} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("完整态：多行，含 peak；无价则空", () => {
    const { container, rerender } = render(
      <ModelPriceTag info={{ free: false, input: 3e-6, output: 15e-6, cacheRead: 3e-7, peakInput: 6e-6, peakOutput: 30e-6 }} />,
    );
    expect(container.querySelectorAll("span > span")).toHaveLength(2);
    expect(container.textContent).toContain("common.pricePeak");
    rerender(<ModelPriceTag info={{ free: false }} />);
    expect(container).toBeEmptyDOMElement();
  });
});
