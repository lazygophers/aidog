// Claude Code `x-claude-code-request-class` 已知取值 → 中文兜底（locale key `logs.requestClass.<值>`）。
const FALLBACK: Record<string, string> = {
  main: "主对话",
  subagent: "子代理",
  workflow: "工作流",
  compaction: "压缩",
  auxiliary: "辅助",
  side_reply: "旁路回复",
};

export const REQUEST_CLASSES = Object.keys(FALLBACK);

/** 过滤器里「未知（request_class=''）」的取值；与后端 `stats_flow_breakdown` 约定一致。 */
export const REQUEST_CLASS_UNKNOWN = "unknown";

/** 请求类型显示名：已知值走词表，未知值原样展示，空串 -> "-"。 */
export function requestClassLabel(t: (key: string, fallback: string) => string, v: string): string {
  if (!v) return "-";
  return v in FALLBACK ? t(`logs.requestClass.${v}`, FALLBACK[v]) : v;
}
