import { describe, it, expect, afterEach } from "vitest";
import { render, screen } from "../../test/render";
import { ModelPriceTag } from "./ModelPriceTag";
import { setDisplayCurrency, USD_DISPLAY } from "../../utils/displayCurrency";
import { toModelPriceInfo } from "../../domains/platforms/modelPrices";

const CNY = { code: "CNY", symbol: "¥", decimals: 2, rate: 6.6675, table: [] };
const JPY = { code: "JPY", symbol: "JP¥", decimals: 0, rate: 150, table: [] };
afterEach(() => setDisplayCurrency(USD_DISPLAY));

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

  it("显示币种 CNY / JPY：USD 条目按汇率换算并带符号", () => {
    const info = { free: false, input: 3e-6, output: 15e-6 };
    setDisplayCurrency(CNY);
    const { container, rerender } = render(<ModelPriceTag compact info={info} />);
    expect(container.textContent).toBe("in ¥20.00 · out ¥100.01");
    setDisplayCurrency(JPY);
    rerender(<ModelPriceTag compact info={info} />);
    expect(container.textContent).toBe("in JP¥450 · out JP¥2,250");
  });

  it("CNY 计价条目：先 ÷6.6675 成 USD 再按显示币种换算，不双重换算", () => {
    // 原价 ¥20/M、¥100/M ≈ $3.0/$15.0
    const info = toModelPriceInfo({ currency: "CNY", input: 20e-6, output: 100e-6 })!;
    const { container, rerender } = render(<ModelPriceTag compact info={info} />);
    expect(container.textContent).toBe("in $3.00 · out $15.00");
    setDisplayCurrency(CNY);
    rerender(<ModelPriceTag compact info={info} />);
    expect(container.textContent).toBe("in ¥20.00 · out ¥100.00");
    setDisplayCurrency(JPY);
    rerender(<ModelPriceTag compact info={info} />);
    expect(container.textContent).toBe("in JP¥450 · out JP¥2,250");
  });
});
