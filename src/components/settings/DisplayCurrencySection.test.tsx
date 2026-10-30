import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "../../test/render";

const h = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn() }));
vi.mock("../../services/transport", () => ({ invoke: vi.fn(), listen: vi.fn(async () => () => {}) }));
vi.mock("../../services/api/currency", () => ({
  DISPLAY_CURRENCY_EVENT: "display-currency-updated",
  displayCurrencyApi: { get: h.get, set: h.set },
}));

// Radix Select 在 jsdom 无法开面板：换成原生 select 壳，testid 仍落在 data-testid="currency-select"
vi.mock("@/components/ui/select", () => ({
  Select: ({ value, onValueChange, children }: { value: string; onValueChange: (v: string) => void; children: React.ReactNode }) => (
    <select data-testid="currency-select" value={value} onChange={(e) => onValueChange(e.target.value)}>{children}</select>
  ),
  SelectTrigger: () => null,
  SelectValue: () => null,
  SelectContent: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  SelectItem: ({ value, children }: { value: string; children: React.ReactNode }) => <option value={value}>{children}</option>,
}));

import { DisplayCurrencySection } from "./DisplayCurrencySection";
import { setDisplayCurrency } from "../../utils/displayCurrency";
import type { CurrencyRow, DisplayCurrency } from "../../services/api/types";

const row = (code: string, symbol: string, def: number, rate = def): CurrencyRow => ({ code, symbol, decimals: 2, default_rate: def, rate });
const table = (cnyRate = 7): CurrencyRow[] => [row("USD", "$", 1), row("CNY", "¥", 7, cnyRate), row("EUR", "€", 0.9)];
const state = (code: string, cnyRate = 7): DisplayCurrency => ({ code, symbol: "$", decimals: 2, rate: 1, table: table(cnyRate) });

const input = (code: string) => screen.getByTestId(`currency-rate-${code}`) as HTMLInputElement;
const save = () => fireEvent.click(screen.getByTestId("currency-save"));

beforeEach(() => {
  vi.clearAllMocks();
  h.get.mockResolvedValue(state("USD"));
  h.set.mockImplementation(async (code: string) => state(code));
  setDisplayCurrency(state("USD"));
});

describe("DisplayCurrencySection", () => {
  it("选币种保存：set 带 code 与空覆盖", async () => {
    render(<DisplayCurrencySection />);
    fireEvent.change(screen.getByTestId("currency-select"), { target: { value: "CNY" } });
    save();
    await waitFor(() => expect(h.set).toHaveBeenCalledWith("CNY", {}));
  });

  it("改汇率保存：仅与默认值不同的项进 rates", async () => {
    render(<DisplayCurrencySection />);
    fireEvent.change(input("CNY"), { target: { value: "6.5" } });
    save();
    await waitFor(() => expect(h.set).toHaveBeenCalledWith("USD", { CNY: 6.5 }));
  });

  it("恢复默认：输入回默认值，保存后不再带覆盖", async () => {
    setDisplayCurrency(state("USD", 6.5));
    render(<DisplayCurrencySection />);
    expect(input("CNY").value).toBe("6.5");
    fireEvent.click(screen.getByTestId("currency-restore-CNY"));
    expect(input("CNY").value).toBe("7");
    save();
    await waitFor(() => expect(h.set).toHaveBeenCalledWith("USD", {}));
  });

  it.each(["0", "-1", "abc", "", "Infinity"])("非法汇率 %j 前端拦截，不调后端", (v) => {
    render(<DisplayCurrencySection />);
    fireEvent.change(input("CNY"), { target: { value: v } });
    save();
    expect(screen.getByTestId("currency-error-CNY")).toBeInTheDocument();
    expect(h.set).not.toHaveBeenCalled();
  });

  it("后端 Err 原文展示", async () => {
    h.set.mockRejectedValue("rate for EUR must be > 0");
    render(<DisplayCurrencySection />);
    save();
    expect(await screen.findByTestId("currency-save-error")).toHaveTextContent("rate for EUR must be > 0");
  });

  it("USD 汇率不可编辑，无恢复按钮", () => {
    render(<DisplayCurrencySection />);
    expect(input("USD")).toBeDisabled();
    expect(screen.queryByTestId("currency-restore-USD")).toBeNull();
  });
});
