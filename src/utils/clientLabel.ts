/** 未识别 UA 的统计码前缀；与 Rust `client_vocab::UA_CODE_PREFIX` 一致（clientLabel.test.ts 跨层核对）。 */
export const UA_CODE_PREFIX = "ua:";

/** 客户端显示名：空串 -> 未知；`ua:<产品名>` -> 原文；其余走词表 `flow.client.<code>`，未知新码回落原始代码。 */
export function clientLabel(t: (key: string, fallback: string) => string, id: string): string {
  if (id.startsWith(UA_CODE_PREFIX)) return id.slice(UA_CODE_PREFIX.length);
  return id === "" ? t("flow.client.unknown", "未知") : t(`flow.client.${id}`, id);
}
