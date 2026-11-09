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

/** 压缩触发原因（`x-claude-code-compaction` / `-context-compacted`）显示名；未知值原样，空串 -> "-"。 */
export function compactionLabel(t: (key: string, fallback: string) => string, v: string): string {
  if (!v) return "-";
  return v in COMPACTION_FALLBACK ? t(`logs.compaction.${v}`, COMPACTION_FALLBACK[v]) : v;
}

const COMPACTION_FALLBACK: Record<string, string> = { auto: "自动", manual: "手动", reactive: "被动" };

/** `Name=ms;Name=ms`（已解码）-> 条目列表；忽略空段与缺 `=` / ms 非数字的段。 */
export function parseToolDurations(s: string): { name: string; ms: number }[] {
  const out: { name: string; ms: number }[] = [];
  for (const seg of s.split(";")) {
    const i = seg.lastIndexOf("=");
    const ms = Number(seg.slice(i + 1));
    if (i > 0 && seg.slice(i + 1).trim() !== "" && Number.isFinite(ms)) out.push({ name: seg.slice(0, i), ms });
  }
  return out;
}
