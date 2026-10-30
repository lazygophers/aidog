// currency.ts — 显示币种命令封装（票 01）。事件名与 Rust `DISPLAY_CURRENCY_EVENT` 一致。

import { invoke } from "../transport";
import type { DisplayCurrency } from "./types";

export const DISPLAY_CURRENCY_EVENT = "display-currency-updated";

export const displayCurrencyApi = {
  get: () => invoke<DisplayCurrency>("display_currency_get"),
  /** 平台 id → 余额币种码（仅真查过的平台有条目）。 */
  balanceCurrencies: () => invoke<Record<string, string>>("balance_currency_map"),
  /** rates 整体替换覆盖项；缺项 = 恢复默认。 */
  set: (code: string, rates: Record<string, number>) =>
    invoke<DisplayCurrency>("display_currency_set", { code, rates }),
};
