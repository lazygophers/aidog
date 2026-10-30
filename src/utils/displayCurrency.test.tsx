import { describe, it, expect, vi, beforeEach } from "vitest";
import { act, render, screen } from "@testing-library/react";

type Handler = () => void;
const h = vi.hoisted(() => ({
  handlers: [] as Handler[],
  get: vi.fn(),
}));

vi.mock("../services/transport", () => ({
  invoke: vi.fn(),
  listen: vi.fn(async (_e: string, cb: Handler) => {
    h.handlers.push(cb);
    return () => {};
  }),
}));
vi.mock("../services/api/currency", () => ({
  DISPLAY_CURRENCY_EVENT: "display-currency-updated",
  displayCurrencyApi: { get: h.get },
}));

import golden from "../../src-tauri/crates/aidog_db/src/currency_golden.json";
import { formatMoney } from "./formatters";
import { setDisplayCurrency, useDisplayCurrency, USD_DISPLAY } from "./displayCurrency";
import type { DisplayCurrency } from "../services/api/types";

const cny: DisplayCurrency = { code: "CNY", symbol: "¥", decimals: 2, rate: 6.6675, table: [] };

function Probe() {
  useDisplayCurrency();
  return <span data-testid="p">{formatMoney(1)}</span>;
}

beforeEach(() => setDisplayCurrency(USD_DISPLAY));

describe("跨层 golden（与 Rust currency_golden.json 同数据）", () => {
  it.each(golden as Array<{ code: string; usd: number; rate: number; symbol: string; decimals: number; expected: string }>)(
    "$code $usd -> $expected",
    (c) => {
      setDisplayCurrency({ code: c.code, symbol: c.symbol, decimals: c.decimals, rate: c.rate, table: [] });
      expect(formatMoney(c.usd)).toBe(c.expected);
    },
  );
  it("覆盖 8 币种 × 多档", () => {
    expect(new Set(golden.map((c) => c.code)).size).toBe(8);
  });
});

describe("币种 store", () => {
  it("启动拉一次，收到 display-currency-updated 后重读并重渲染", async () => {
    h.get.mockResolvedValueOnce(USD_DISPLAY).mockResolvedValueOnce(cny);
    render(<Probe />);
    await act(async () => {});
    expect(screen.getByTestId("p").textContent).toBe("$1.00");
    expect(h.handlers).toHaveLength(1);
    await act(async () => {
      h.handlers[0]();
    });
    expect(h.get).toHaveBeenCalledTimes(2);
    expect(screen.getByTestId("p").textContent).toBe("¥6.67");
  });
});
