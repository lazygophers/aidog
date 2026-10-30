import { describe, it, expect, vi, beforeEach } from "vitest";

const h = vi.hoisted(() => ({ map: vi.fn() }));
vi.mock("../services/transport", () => ({ invoke: vi.fn(), listen: vi.fn(async () => () => {}) }));
vi.mock("../services/api/currency", () => ({
  DISPLAY_CURRENCY_EVENT: "display-currency-updated",
  displayCurrencyApi: { get: vi.fn(), balanceCurrencies: h.map },
}));
vi.mock("../services/api", () => ({
  groupDetailApi: {},
  groupUsageApi: { statsAll: vi.fn(async () => ({})) },
  platformApi: {},
  proxyApi: {},
  onProxyLogUpdated: vi.fn(),
}));

import { balanceToUsd, platformBalanceUsd, setBalanceCurrencies, refreshBalanceCurrencies } from "./balanceCurrency";
import { setDisplayCurrency } from "./displayCurrency";
import { formatMoney } from "./formatters";
import { computeQuotaDisplay } from "../domains/platforms/health";
import { fetchGroupStats } from "../pages/Groups/useGroupData";
import type { DisplayCurrency } from "../services/api/types";

const row = (code: string, symbol: string, decimals: number, rate: number) => ({ code, symbol, decimals, default_rate: rate, rate });
const table = [row("USD", "$", 2, 1), row("CNY", "¥", 2, 6.6675), row("JPY", "JP¥", 0, 150)];
const disp = (code: string): DisplayCurrency => {
  const r = table.find((t) => t.code === code)!;
  return { code, symbol: r.symbol, decimals: r.decimals, rate: r.rate, table };
};
const plat = (id: number, bal: number) => ({ id, est_balance_remaining: bal, est_coding_plan: "" }) as never;

beforeEach(() => {
  setDisplayCurrency(disp("USD"));
  setBalanceCurrencies({});
});

describe("balanceToUsd", () => {
  it("CNY 余额 ÷ 6.6675；USD / 未记录 / 未知币种原样", () => {
    expect(balanceToUsd(66.675, "CNY")).toBeCloseTo(10, 9);
    expect(balanceToUsd(10, "USD")).toBe(10);
    expect(balanceToUsd(10, undefined)).toBe(10);
    expect(balanceToUsd(10, "EUR")).toBe(10);
  });
  it("币种表未加载时 CNY 原样（不除 0 / NaN）", () => {
    setDisplayCurrency({ ...disp("USD"), table: [] });
    expect(balanceToUsd(10, "CNY")).toBe(10);
  });
  it("用户改 CNY 汇率：显示 CNY 时 CNY 余额等于原值", () => {
    const t7 = table.map((r) => (r.code === "CNY" ? { ...r, rate: 7 } : r));
    setDisplayCurrency({ code: "CNY", symbol: "¥", decimals: 2, rate: 7, table: t7 });
    expect(formatMoney(balanceToUsd(70, "CNY"))).toBe("¥70.00");
  });
});

describe("computeQuotaDisplay 余额币种", () => {
  it("CNY 余额 + 显示 USD → ÷6.6675", () => {
    const q = computeQuotaDisplay(plat(1, 66.675), undefined, false, "CNY");
    expect(formatMoney(q.balanceRemaining!)).toBe("$10.00");
    expect(q.currency).toBe("USD");
  });
  it("CNY 余额 + 显示 CNY → 原值", () => {
    setDisplayCurrency(disp("CNY"));
    const q = computeQuotaDisplay(plat(1, 66), undefined, false, "CNY");
    expect(formatMoney(q.balanceRemaining!)).toBe("¥66.00");
  });
  it("USD 余额 + 显示 JPY", () => {
    setDisplayCurrency(disp("JPY"));
    const q = computeQuotaDisplay(plat(1, 10), undefined, false, "USD");
    expect(formatMoney(q.balanceRemaining!)).toBe("JP¥1,500");
  });
  it("未知币种 / 未记录按 USD", () => {
    expect(computeQuotaDisplay(plat(1, 10), undefined, false, "EUR").balanceRemaining).toBe(10);
    expect(computeQuotaDisplay(plat(1, 10), undefined, false).balanceRemaining).toBe(10);
  });
  it("真查 CNY 余额与 total 一并折 USD；ACU 不换算保留单位", () => {
    const real = { balance: { remaining: 66.675, total: 133.35, used: 0, currency: "CNY" } } as never;
    const q = computeQuotaDisplay(plat(1, 0), real, true);
    expect(q.balanceRemaining).toBeCloseTo(10, 9);
    expect(q.balanceTotal).toBeCloseTo(20, 9);
    const acu = { balance: { remaining: 0, total: null, used: 42, currency: "ACU" } } as never;
    const a = computeQuotaDisplay(plat(1, 0), acu, true);
    expect(a.balanceRemaining).toBe(42);
    expect(a.currency).toBe("ACU");
  });
});

describe("分组余额汇总", () => {
  it("混币种先逐平台折 USD 再求和", async () => {
    h.map.mockResolvedValue({ "1": "CNY", "2": "USD" });
    const details = [{ group: { id: 5, group_key: "g" }, platforms: [{ platform: { id: 1 } }, { platform: { id: 2 } }] }] as never;
    const r = await fetchGroupStats(details, [plat(1, 66.675), plat(2, 5)]);
    expect(r.balanceMap[5]).toBeCloseTo(15, 9);
  });
  it("platformBalanceUsd 读 store", async () => {
    h.map.mockResolvedValue({ "9": "CNY" });
    await refreshBalanceCurrencies();
    expect(platformBalanceUsd(9, 6.6675)).toBeCloseTo(1, 9);
    expect(platformBalanceUsd(8, 6.6675)).toBe(6.6675);
  });
});
