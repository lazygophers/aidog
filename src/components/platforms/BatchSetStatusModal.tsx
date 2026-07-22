// ─── 批量改平台状态弹窗（group-batch-ops s5）──────────────────
// 启用/禁用 radio + 无候选警告（选中平台覆盖当前组全部候选 + 改 disabled → 整组将无候选）。
// 确认 → 调 batch_set_status 原子事务（status 仅 enabled/disabled）。
// Dialog 走 Radix Portal（替代 shared/Modal，liquid glass 居中由 Portal 保证）。

import { useEffect, useMemo, useState } from "react";
import type { TFunction } from "i18next";
import type { Platform } from "../../services/api";
import { Button } from "@/components/ui/button";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

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

  useEffect(() => {
    if (open) setStatus("disabled");
  }, [open]);

  const willEmptyGroup = useMemo(() => {
    if (status !== "disabled" || groupEnabledIds.length === 0) return false;
    const selectedIds = new Set(platforms.map(p => p.id));
    return groupEnabledIds.every(id => selectedIds.has(id));
  }, [status, groupEnabledIds, platforms]);

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next && !busy) onClose();
      }}
    >
      <DialogContent
        className="glass-elevated"
        style={{ maxWidth: 480, maxHeight: "80vh", overflowY: "auto", padding: "20px 22px" }}
        onEscapeKeyDown={(e) => { if (busy) e.preventDefault(); }}
        onPointerDownOutside={(e) => { if (busy) e.preventDefault(); }}
      >
        <DialogHeader>
          <DialogTitle>{t("group.batchSetStatusTitle", "批量改状态")}</DialogTitle>
          <DialogDescription>
            {t("group.batchSetStatusDesc", "将以下 {{count}} 个平台的状态改为：", { count: platforms.length })}
          </DialogDescription>
        </DialogHeader>

        <RadioGroup
          value={status}
          onValueChange={(v) => setStatus(v as StatusChoice)}
          style={{ display: "flex", gap: 16, marginBottom: 10 }}
        >
          {(["disabled", "enabled"] as const).map(s => (
            <div key={s} style={{ display: "inline-flex", alignItems: "center", gap: 5, fontSize: 13 }}>
              <RadioGroupItem value={s} id={`batch-status-${s}`} />
              <label htmlFor={`batch-status-${s}`} style={{ cursor: "pointer" }}>
                {s === "enabled"
                  ? t("group.batchSetStatusEnabled", "启用")
                  : t("group.batchSetStatusDisabled", "禁用")}
              </label>
            </div>
          ))}
        </RadioGroup>

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

        <DialogFooter>
          <Button variant="ghost" onClick={onClose} disabled={busy}>
            {t("action.cancel", "取消")}
          </Button>
          <Button onClick={() => onConfirm(status)} disabled={busy}>
            {busy
              ? t("group.batchSetStatusApplying", "应用中…")
              : t("group.batchSetStatusConfirm", "改状态（{{count}} 个平台）", { count: platforms.length })}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
