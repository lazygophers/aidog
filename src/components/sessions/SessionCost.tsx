// 会话成本显示口径（设计稿方向 3）：全部未定价 →「未定价」；部分未定价 → 金额带 +。
import { useTranslation } from "react-i18next";
import { formatMoney, rtlIsolate } from "../../utils/formatters";

export type PricedState = "all" | "partial" | "none";

export function pricedState(calls: number, unpricedCalls: number): PricedState {
  if (unpricedCalls <= 0) return "all";
  return unpricedCalls >= calls ? "none" : "partial";
}

export function SessionCost({ cost, calls, unpricedCalls, testId }: { cost: number; calls: number; unpricedCalls: number; testId?: string }) {
  const { t } = useTranslation();
  const state = pricedState(calls, unpricedCalls);
  return (
    <span data-testid={testId} data-priced={state}>
      {state === "none" ? t("clientUsage.unpriced", "Unpriced") : rtlIsolate(`${formatMoney(cost)}${state === "partial" ? "+" : ""}`)}
    </span>
  );
}
