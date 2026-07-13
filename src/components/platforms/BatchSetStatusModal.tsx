// ─── 批量改平台状态弹窗（group-batch-ops s5）──────────────────
// 启用/禁用 radio + 无候选警告（选中平台覆盖当前组全部候选 + 改 disabled → 整组将无候选）。
// 确认 → 调 batch_set_status 原子事务（status 仅 enabled/disabled）。
// 复用 shared/Modal 基元（createPortal document.body，liquid glass 居中）。

import { useEffect, useMemo, useState } from "react";
import type { TFunction } from "i18next";
import type { Platform } from "../../services/api";
import { Modal } from "../shared/Modal";

type StatusChoice = "enabled" | "disabled";

export interface BatchSetStatusModalProps {
  open: boolean;
  /** 待改状态平台（全列可滚展示）。 */
  platforms: Platform[];
  /** 当前组内所有 enabled 平台 id（无候选警告数据源）。
   *  警告触发条件：status=disabled 且选中平台覆盖此集合全部 → 整组将无候选。 */
  groupEnabledIds: number[];
  /** 确认改状态：父级执行 invoke + 刷新 + toast，完成后 onClose。 */
  onConfirm: (status: StatusChoice) => void;
  onClose: () => void;
  /** 改状态中（invoke 等），禁用按钮 + 切文案。 */
  busy?: boolean;
  t: TFunction;
}

export function BatchSetStatusModal({
  open,
  platforms,
  groupEnabledIds,
  onConfirm,
  onClose,
  busy = false,
  t,
}: BatchSetStatusModalProps) {
  const [status, setStatus] = useState<StatusChoice>("disabled");

  // 开 modal 时重置为 disabled（用户最常见意图是批量禁用；enabled 无警告风险）
  useEffect(() => {
    if (open) setStatus("disabled");
  }, [open]);

  // 无候选警告：改 disabled + 选中平台覆盖当前组全部 enabled 候选
  const willEmptyGroup = useMemo(() => {
    if (status !== "disabled" || groupEnabledIds.length === 0) return false;
    const selectedIds = new Set(platforms.map(p => p.id));
    return groupEnabledIds.every(id => selectedIds.has(id));
  }, [status, groupEnabledIds, platforms]);

  return (
    <Modal
      open={open}
      onClose={busy ? () => {} : onClose}
      className="glass-elevated"
      maxWidth={480}
      maxHeight="80vh"
      style={{ padding: "20px 22px" }}
      closeOnBackdrop={!busy}
      closeOnEscape={!busy}
    >
      {/* 标题 */}
      <div style={{ fontSize: 16, fontWeight: 700, marginBottom: 4 }}>
        {t("group.batchSetStatusTitle", "批量改状态")}
      </div>
      <div style={{ fontSize: 13, color: "var(--text-secondary)", marginBottom: 12 }}>
        {t("group.batchSetStatusDesc", "将以下 {{count}} 个平台的状态改为：", { count: platforms.length })}
      </div>

      {/* 状态 radio */}
      <div style={{ display: "flex", gap: 16, marginBottom: 10 }}>
        {(["disabled", "enabled"] as const).map(s => (
          <label key={s} style={{ display: "inline-flex", alignItems: "center", gap: 5, fontSize: 13, cursor: "pointer" }}>
            <input
              type="radio"
              name="batch-set-status"
              checked={status === s}
              onChange={() => setStatus(s)}
              style={{ accentColor: "var(--accent)" }}
            />
            {s === "enabled"
              ? t("group.batchSetStatusEnabled", "启用")
              : t("group.batchSetStatusDisabled", "禁用")}
          </label>
        ))}
      </div>

      {/* 无候选警告 */}
      {willEmptyGroup && (
        <div style={{
          fontSize: 12.5,
          color: "var(--color-danger)",
          lineHeight: 1.55,
          marginBottom: 12,
          padding: "8px 12px",
          borderRadius: "var(--radius-sm)",
          background: "var(--bg-glass)",
          border: "1px solid var(--color-danger)",
        }}>
          {t("group.batchSetStatusNoCandidateWarning", "警告：选中平台覆盖了当前分组的全部候选（enabled）平台，禁用后整组将无可用候选，路由请求将失败。")}
        </div>
      )}

      {/* 平台全列（可滚） */}
      <div style={{
        display: "flex", flexDirection: "column", gap: 4,
        maxHeight: "32vh", overflowY: "auto",
        padding: "6px 10px", borderRadius: "var(--radius-sm)",
        background: "var(--bg-glass)", border: "1px solid var(--border)",
      }}>
        {platforms.map(p => (
          <div key={p.id} style={{
            display: "flex", alignItems: "center", gap: 6, minWidth: 0,
            padding: "5px 0", borderBottom: "1px solid var(--border)",
          }}>
            <span style={{
              fontSize: 10, fontWeight: 500, padding: "0 5px", borderRadius: "var(--radius-sm)",
              ...(p.status === "enabled"
                ? { color: "var(--color-success)", background: "color-mix(in srgb, var(--color-success) 14%, transparent)" }
                : p.status === "auto_disabled"
                  ? { color: "var(--color-warning)", background: "color-mix(in srgb, var(--color-warning) 14%, transparent)" }
                  : { color: "var(--text-tertiary)", background: "var(--bg-glass)" }),
            }}>
              {p.status === "enabled"
                ? t("platform.statusEnabled", "启用")
                : p.status === "auto_disabled"
                  ? t("platform.statusAutoDisabled", "熔断")
                  : t("platform.statusDisabled", "禁用")}
            </span>
            <span style={{ fontSize: 13, fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {p.name}
            </span>
          </div>
        ))}
      </div>

      {/* 操作按钮 */}
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
        <button className="btn btn-ghost" style={{ fontSize: 13, padding: "6px 14px" }} onClick={onClose} disabled={busy}>
          {t("action.cancel", "取消")}
        </button>
        <button
          className="btn btn-primary"
          style={{ fontSize: 13, padding: "6px 14px" }}
          onClick={() => onConfirm(status)}
          disabled={busy}
        >
          {busy
            ? t("group.batchSetStatusApplying", "应用中…")
            : t("group.batchSetStatusConfirm", "改状态（{{count}} 个平台）", { count: platforms.length })}
        </button>
      </div>
    </Modal>
  );
}
