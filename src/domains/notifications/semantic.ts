// 语义事件目录（通知体系重设计票 01）。
// Rust 侧真值源：aidog_db/src/models/notification.rs 的
// notif_type_for_event / PROXY_SEMANTIC_EVENTS，及 aidog_notification/src/semantic.rs 的
// SEMANTIC_TYPES —— 改 Rust 必须同步本文件（跨层镜像，测试双向锚定事件清单）。
export type SemanticType =
  | "task_complete"
  | "waiting_input"
  | "error"
  | "breaker_on"
  | "breaker_off"
  | "quota_exhaust_forecast";

export const SEMANTIC_TYPES: SemanticType[] = [
  "task_complete",
  "waiting_input",
  "error",
  "breaker_on",
  "breaker_off",
  "quota_exhaust_forecast",
];

/** proxy 内核侧语义事件（熔断/配额预测发射点）。镜像 PROXY_SEMANTIC_EVENTS。 */
export const PROXY_SEMANTIC_EVENTS: SemanticType[] = [
  "breaker_on",
  "breaker_off",
  "quota_exhaust_forecast",
];

/** CC hook 事件 → 语义类型；未列出 = raw 事件名透传自成类型。镜像 notif_type_for_event。 */
export const CC_EVENT_SEMANTIC: Record<string, SemanticType> = {
  Stop: "task_complete",
  TaskCompleted: "task_complete",
  SubagentStop: "task_complete",
  PermissionRequest: "waiting_input",
  Elicitation: "waiting_input",
  Notification: "waiting_input",
  StopFailure: "error",
  PostToolUseFailure: "error",
};
