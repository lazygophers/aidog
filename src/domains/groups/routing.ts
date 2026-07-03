import type { TFunction } from "i18next";
import type { RoutingMode } from "../../services/api";

/** 全部调度策略（与 api.ts RoutingMode 契约对齐，禁裸 string）。 */
export const ROUTING_MODES: RoutingMode[] = ["failover", "load_balance", "health_aware", "least_latency", "sticky"];

/** 策略短名（i18n，缺键回退默认中文）。 */
export function routingModeLabel(t: TFunction, mode: RoutingMode): string {
  const map: Record<RoutingMode, string> = {
    failover: t("group.failover", "故障转移"),
    load_balance: t("group.loadBalance", "负载均衡"),
    health_aware: t("group.routingMode.health_aware", "健康感知"),
    least_latency: t("group.routingMode.least_latency", "最低延迟"),
    sticky: t("group.routingMode.sticky", "会话粘性"),
  };
  return map[mode] ?? mode;
}

/** 策略说明（下拉旁提示）。 */
export function routingModeDesc(t: TFunction, mode: RoutingMode): string {
  const map: Record<RoutingMode, string> = {
    failover: t("group.routingModeDesc.failover", "按优先级升序选平台，失败逐个回退。"),
    load_balance: t("group.routingModeDesc.load_balance", "在可用平台间加权随机分流。"),
    health_aware: t("group.routingModeDesc.health_aware", "摘除熔断平台后，在健康平台间加权随机。"),
    least_latency: t("group.routingModeDesc.least_latency", "按各平台延迟均值升序优先选最快平台。"),
    sticky: t("group.routingModeDesc.sticky", "同会话绑定同一平台，失效/熔断后回退加权随机。"),
  };
  return map[mode] ?? "";
}
