import { useTranslation } from "react-i18next";
import type { GroupLastRequest as Last } from "../../services/api";
import { formatRelativeTime } from "../../utils/formatters";
import { blockedReasonLabel } from "../Logs/primitives";

/**
 * 分组「最后一次请求」徽标（列表卡片头 / 详情页顶栏共用）：成功|失败 + 状态码 + 相对时间，
 * 失败时附原因（blocked_reason 标签，否则 attempts/响应体摘要）。有 onOpen 时点击跳日志详情。
 */
export function GroupLastRequest({ last, onOpen }: { last: Last | undefined; onOpen?: (logId: string) => void }) {
  const { t } = useTranslation();
  const label = t("group.lastRequest.label", "最近请求");
  if (!last) {
    return (
      <span data-testid="group-last-request" data-state="none" style={{ fontSize: 14, color: "var(--text-tertiary)", whiteSpace: "nowrap" }}>
        {label} · {t("group.lastRequest.none", "暂无请求")}
      </span>
    );
  }
  const state = last.status_code === 0 ? "incomplete" : last.status_code >= 200 && last.status_code < 300 ? "success" : "failed";
  const color = state === "success" ? "var(--color-success)" : state === "failed" ? "var(--color-danger)" : "var(--text-tertiary)";
  const stateText = t(`group.lastRequest.${state}`, state);
  const reasonTitle = state === "failed" ? last.blocked_reason || last.error : "";
  const reasonText = last.blocked_reason ? blockedReasonLabel(last.blocked_reason, t) : last.error;
  const rel = formatRelativeTime(last.created_at);
  return (
    <span
      data-testid="group-last-request"
      data-state={state}
      role={onOpen ? "button" : undefined}
      title={onOpen ? t("group.lastRequest.viewLog", "查看日志详情") : undefined}
      onClick={onOpen ? (e) => { e.stopPropagation(); onOpen(last.id); } : undefined}
      style={{ display: "inline-flex", alignItems: "center", gap: 6, fontSize: 14, minWidth: 0, cursor: onOpen ? "pointer" : undefined }}
    >
      <span style={{ color: "var(--text-tertiary)", whiteSpace: "nowrap" }}>{label}</span>
      <b style={{ color, fontWeight: 600 }}>{stateText}</b>
      {last.status_code !== 0 && <span className="num" data-testid="group-last-request-code" style={{ color }}>{last.status_code}</span>}
      {rel && <span style={{ color: "var(--text-tertiary)", whiteSpace: "nowrap" }}>{rel}</span>}
      {reasonTitle && (
        <span
          data-testid="group-last-request-reason"
          title={reasonTitle}
          style={{ color: "var(--color-danger)", maxWidth: 260, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
        >
          {reasonText}
        </span>
      )}
    </span>
  );
}
