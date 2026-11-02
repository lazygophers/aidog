/** 客户端显示名：空串 -> 未知；`ua:<产品名>` -> 原文；其余走词表 `flow.client.<code>`，未知新码回落原始代码。 */
export function clientLabel(t: (key: string, fallback: string) => string, id: string): string {
  if (id.startsWith("ua:")) return id.slice(3);
  return id === "" ? t("flow.client.unknown", "未知") : t(`flow.client.${id}`, id);
}
