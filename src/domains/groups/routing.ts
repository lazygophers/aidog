import type { TFunction } from "i18next";
import type { RoutingMode } from "../../services/api";

/** 全部调度策略（与 api.ts RoutingMode 契约对齐，禁裸 string）。 */
export const ROUTING_MODES: RoutingMode[] = [
  "failover",
  "load_balance",
  "health_aware",
  "sticky",
  "rotate",
  "usage",
  "pace",
  "smart",
];

/** 策略短名（i18n，缺键回退默认中文）。 */
export function routingModeLabel(t: TFunction, mode: RoutingMode): string {
  const map: Record<RoutingMode, string> = {
    failover: t("group.failover", "Failover"),
    load_balance: t("group.loadBalance", "Load Balance"),
    health_aware: t("group.routingMode.health_aware", "Health-aware"),
    sticky: t("group.routingMode.sticky", "Sticky session"),
    rotate: t("group.routingMode.rotate", "Strict rotate"),
    usage: t("group.routingMode.usage", "Least usage"),
    pace: t("group.routingMode.pace", "Most quota left"),
    smart: t("group.routingMode.smart", "Smart"),
  };
  return map[mode] ?? mode;
}

/** 策略说明（下拉旁提示）。 */
export function routingModeDesc(t: TFunction, mode: RoutingMode): string {
  const map: Record<RoutingMode, string> = {
    failover: t("group.routingModeDesc.failover", "Pick platforms by ascending priority, falling back one by one on failure."),
    load_balance: t("group.routingModeDesc.load_balance", "Weighted random distribution across available platforms."),
    health_aware: t("group.routingModeDesc.health_aware", "Weighted-random routing, identical to Load Balance; breaker/cooldown exclusion applies to every routing mode."),
    sticky: t("group.routingModeDesc.sticky", "Bind a session to one platform; fall back to weighted random when unavailable or tripped."),
    rotate: t("group.routingModeDesc.rotate", "Rotate across same-tier platforms request by request; the rotation restarts from the beginning after an app restart."),
    usage: t("group.routingModeDesc.usage", "Prefer platforms with the fewest successful requests in the last 24 hours."),
    pace: t("group.routingModeDesc.pace", "Prefer platforms with the most quota left before reset; falls back to least-usage mode when quota data is unavailable."),
    smart: t("group.routingModeDesc.smart", "Prefer plans resetting soonest, then highest plan balance; falls back to health-aware mode when reset data is unavailable."),
  };
  return map[mode] ?? "";
}
