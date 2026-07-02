import { createPortal } from "react-dom";
import type { CSSProperties } from "react";
import type { TFunction } from "i18next";
import type { GroupDetail } from "../../services/api";
import { IconClose } from "../../components/icons";

/** 分组一键测试并发上限：同时最多 N 个平台在测，完一个补下一个。 */
export const BATCH_TEST_CONCURRENCY = 3;

/** Row model for the sortable group list (GroupDetail has no top-level stable id). */
export interface GroupRow {
  id: string;
  detail: GroupDetail;
}

/** 分组一键测试：单平台测试行状态（串行执行，面板实时刷新）。 */
export type GroupTestStatus = "pending" | "testing" | "ok" | "fail";

export interface GroupTestRow {
  platformId: number;
  name: string;
  status: GroupTestStatus;
  durationMs?: number;
  error?: string;
}

/**
 * 分组一键测试结果面板。逐平台串行测试，行状态实时刷新。
 * createPortal 挂 body —— 脱离 transform 祖先（liquidGlass/animate-fade-in）避免 fixed 退化，
 * 参考 toast 修复（commit 0aeff95）与 memory `css-transform-breaks-fixed-modal`。
 */
export function GroupTestPanel({ groupName, rows, running, onClose, t }: {
  groupName: string;
  rows: GroupTestRow[];
  running: boolean;
  onClose: () => void;
  t: TFunction;
}) {
  const ok = rows.filter(r => r.status === "ok").length;
  const fail = rows.filter(r => r.status === "fail").length;
  const done = ok + fail;
  const statusStyle = (s: GroupTestStatus): CSSProperties => ({
    fontSize: 12, fontWeight: 600,
    color: s === "ok" ? "var(--success)" : s === "fail" ? "var(--danger)" : "var(--text-tertiary)",
  });
  const statusText = (r: GroupTestRow): string => {
    if (r.status === "testing") return "…";
    if (r.status === "pending") return t("group.testAllPending", "等待");
    if (r.status === "ok") return t("group.testAllOk", "成功") + (r.durationMs != null ? ` ${r.durationMs}ms` : "");
    return t("group.testAllFail", "失败");
  };
  return createPortal(
    <div onClick={onClose} style={{
      position: "fixed", inset: 0, background: "rgba(0,0,0,0.45)", zIndex: 1000,
      display: "flex", alignItems: "center", justifyContent: "center", padding: 20,
    }}>
      <div className="glass-surface" onClick={e => e.stopPropagation()} style={{
        width: "min(560px, 92vw)", maxHeight: "80vh", overflow: "auto",
        display: "flex", flexDirection: "column", gap: 10, padding: 20,
        background: "var(--bg-floating)",
      }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
          <div style={{ fontSize: 15, fontWeight: 700 }}>
            {t("group.testAllTitle", "测试分组平台")}：{groupName}
          </div>
          <button className="btn btn-ghost btn-icon" onClick={onClose} title={t("action.dismiss", "关闭")}>
            <IconClose size={16} />
          </button>
        </div>
        <div style={{ fontSize: 12, color: "var(--text-secondary)" }}>
          {running
            ? t("group.testAllProgress", "测试中… {{done}}/{{total}}", { done, total: rows.length })
            : t("group.testAllSummary", "完成：{{ok}} 成功 / {{fail}} 失败 / 共 {{total}}", { ok, fail, total: rows.length })}
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {rows.map(r => (
            <div key={r.platformId} style={{
              display: "flex", flexDirection: "column", gap: 4, padding: "6px 8px",
              borderRadius: "var(--radius-sm)", background: "var(--bg-glass)",
              borderLeft: r.status === "ok"
                ? "3px solid var(--success)"
                : r.status === "fail" ? "3px solid var(--danger)" : "3px solid transparent",
            }}>
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <span style={{
                  fontSize: 13, flex: 1, minWidth: 0,
                  overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                }}>{r.name}</span>
                <span style={statusStyle(r.status)}>{statusText(r)}</span>
              </div>
              {r.status === "fail" && r.error && (
                <div
                  title={r.error}
                  style={{
                    fontSize: 11, color: "var(--danger)",
                    overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                  }}
                >
                  {r.error}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>,
    document.body,
  );
}
