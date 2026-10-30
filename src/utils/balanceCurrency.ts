// 平台余额币种 store（票 08）：est_balance_remaining / 真查余额都是平台**原币种**，
// 显示前先折 USD（CNY ÷ 币种表生效 CNY 汇率，其余视 USD，镜像 Rust `DisplayCurrency::balance_to_usd`），
// 再交给 formatMoney 按显示币种换算。币种表来自显示币种 store，不另存汇率。
// shortcut: 仅识别 CNY（与后端余额扣减 balance_fx 一致），其它币种按 USD 处理；新增币种时两端同改。
import { useSyncExternalStore } from "react";
import { listen } from "../services/transport";
import { displayCurrencyApi } from "../services/api/currency";
import { getDisplayCurrency } from "./displayCurrency";

let current: Record<string, string> = {};
const subscribers = new Set<() => void>();
let started = false;
let lastFetch = 0;
const THROTTLE_MS = 10_000;

export function getBalanceCurrencies(): Record<string, string> {
  return current;
}

/** 直接写入（测试注入用）。 */
export function setBalanceCurrencies(next: Record<string, string>): void {
  current = next;
  subscribers.forEach((cb) => cb());
}

/** 拉一次平台 id → 余额币种；失败保留旧值。 */
export async function refreshBalanceCurrencies(): Promise<void> {
  lastFetch = Date.now();
  try {
    setBalanceCurrencies((await displayCurrencyApi.balanceCurrencies()) ?? {});
  } catch (e) {
    console.error("balance_currency_map failed", e);
  }
}

function start(): void {
  if (started) return;
  started = true;
  void refreshBalanceCurrencies();
  // 真查成功后端先记币种再 emit proxy-log-updated；节流避免逐请求都拉（映射极小，10s 内复用）。
  Promise.resolve()
    .then(() =>
      listen("proxy-log-updated", () => {
        if (Date.now() - lastFetch > THROTTLE_MS) void refreshBalanceCurrencies();
      }),
    )
    .catch((e) => console.error(e));
}

function subscribe(cb: () => void): () => void {
  subscribers.add(cb);
  start();
  return () => {
    subscribers.delete(cb);
  };
}

/** 组件内订阅：币种映射变化时重渲染；返回当前映射（平台 id 字符串键）。 */
export function useBalanceCurrencies(): Record<string, string> {
  return useSyncExternalStore(subscribe, getBalanceCurrencies);
}

/** 原币种余额 → USD。ccy 空 / 非 CNY 视 USD；币种表未加载（无 CNY 行）时原样返回。 */
export function balanceToUsd(amount: number, ccy: string | null | undefined): number {
  if (ccy?.trim().toUpperCase() !== "CNY") return amount;
  const rate = getDisplayCurrency().table.find((r) => r.code === "CNY")?.rate;
  return rate && rate > 0 ? amount / rate : amount;
}

/** 平台余额（原币种，按 store 记录的币种）→ USD。 */
export function platformBalanceUsd(platformId: number, amount: number): number {
  return balanceToUsd(amount, current[String(platformId)]);
}
