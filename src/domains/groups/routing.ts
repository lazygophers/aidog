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
    failover: t("group.failover", "故障转移"),
    load_balance: t("group.loadBalance", "负载均衡"),
    health_aware: t("group.routingMode.health_aware", "健康感知"),
    sticky: t("group.routingMode.sticky", "会话粘性"),
    rotate: t("group.routingMode.rotate", "严格轮换"),
    usage: t("group.routingMode.usage", "用量最少"),
    pace: t("group.routingMode.pace", "余量最多"),
    smart: t("group.routingMode.smart", "智能模式"),
  };
  return map[mode] ?? mode;
}

/** 策略说明（下拉旁提示）。 */
export function routingModeDesc(t: TFunction, mode: RoutingMode): string {
  const map: Record<RoutingMode, string> = {
    failover: t("group.routingModeDesc.failover", "按优先级升序选平台，失败逐个回退。"),
    load_balance: t("group.routingModeDesc.load_balance", "在可用平台间加权随机分流。"),
    health_aware: t("group.routingModeDesc.health_aware", "加权随机分流，实现与负载均衡相同；熔断/冷却摘除对所有路由模式生效。"),
    sticky: t("group.routingModeDesc.sticky", "同会话绑定同一平台，失效/熔断后回退加权随机。"),
    rotate: t("group.routingModeDesc.rotate", "同档平台间按请求依次轮转，重启后从头开始。"),
    usage: t("group.routingModeDesc.usage", "近 24 小时成功请求量最少的平台优先。"),
    pace: t("group.routingModeDesc.pace", "重置前剩余额度最多的平台优先；无额度数据时回落用量模式。"),
    smart: t("group.routingModeDesc.smart", "最快重置的套餐优先，次键套餐余额；无重置数据时回落健康感知。"),
  };
  return map[mode] ?? "";
}
