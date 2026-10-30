/** 流向图客户端列显示名：空串 -> 未知；其余走词表 `flow.client.<code>`，未知新码回落原始代码。 */
export function clientLabel(t: (key: string, fallback: string) => string, id: string): string {
  return id === "" ? t("flow.client.unknown", "未知") : t(`flow.client.${id}`, id);
}
