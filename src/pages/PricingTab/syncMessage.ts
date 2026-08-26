import type { PriceSyncResult } from "../../services/api";

type Translate = (key: string, fallback: string) => string;

/**
 * registry 同步结果的提示文案：计数摘要 + partial 失败文件清单。
 *
 * 同步是 best-effort——失败的文件压根不写库，那些平台在界面上仍是上一轮的名字与 logo。
 * 所以失败必须逐条列出文件名，用户才知道哪几个平台的数据是旧的，而不是只看到一个「N 失败」。
 */
export function syncResultMessage(result: PriceSyncResult, t: Translate): string {
  const summary = t("pricing.syncResult", "同步完成: +{added} 新增, ~{updated} 更新, {failed} 失败 (共 {total} 模型)")
    .replace("{added}", String(result.added))
    .replace("{updated}", String(result.updated))
    .replace("{failed}", String(result.failed))
    .replace("{total}", String(result.total));
  if (result.failures.length === 0) return summary;
  const files = result.failures.map(f => f.file).join(", ");
  return `${summary} — ${t("pricing.syncFailures", "以下文件未更新，保留原有数据: {files}").replace("{files}", files)}`;
}
