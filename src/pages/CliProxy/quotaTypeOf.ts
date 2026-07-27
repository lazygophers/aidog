/** 解析 quota JSON → type 值（none/newapi），异常/缺省回落 none。 */
export function quotaTypeOf(q: string | undefined): string {
  try { return (JSON.parse(q || "{}").type) || "none"; } catch { return "none"; }
}
