// 显示币种 store（票 02）：模块级单例，主窗口与 popover 各自一份，靠同一事件同步。
// 首个订阅者出现时拉一次 display_currency_get 并挂单底层 listen（对齐 pricing.ts registry-updated idiom）。
import { useSyncExternalStore } from "react";
import { listen } from "../services/transport";
import { displayCurrencyApi, DISPLAY_CURRENCY_EVENT } from "../services/api/currency";
import type { DisplayCurrency } from "../services/api/types";

export const USD_DISPLAY: DisplayCurrency = { code: "USD", symbol: "$", decimals: 2, rate: 1, table: [] };

let current: DisplayCurrency = USD_DISPLAY;
const subscribers = new Set<() => void>();
let started = false;

export function getDisplayCurrency(): DisplayCurrency {
  return current;
}

/** 直接写入（设置页 set 成功后用返回值即时落地；测试也用它注入）。 */
export function setDisplayCurrency(next: DisplayCurrency): void {
  current = next;
  subscribers.forEach((cb) => cb());
}

async function refresh(): Promise<void> {
  try {
    setDisplayCurrency(await displayCurrencyApi.get());
  } catch (e) {
    console.error("display_currency_get failed", e);
  }
}

function start(): void {
  if (started) return;
  started = true;
  void refresh();
  // listen 在无 Tauri / 无 EventSource 的环境会同步抛，包进 promise 链统一吞成日志
  Promise.resolve()
    .then(() => listen(DISPLAY_CURRENCY_EVENT, () => void refresh()))
    .catch((e) => console.error(e));
}

export function subscribeDisplayCurrency(cb: () => void): () => void {
  subscribers.add(cb);
  start();
  return () => {
    subscribers.delete(cb);
  };
}

/** 组件内调用以订阅币种变化；formatMoney 自身读同一 store，组件只需让自己重渲染。 */
export function useDisplayCurrency(): DisplayCurrency {
  return useSyncExternalStore(subscribeDisplayCurrency, getDisplayCurrency);
}
