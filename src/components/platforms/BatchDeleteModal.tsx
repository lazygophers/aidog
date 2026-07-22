// ─── 批量删除平台确认弹窗（group-batch-ops s3）──────────────────
// 选中平台全列可滚 + 跨组警告（平台属多组时显式「将从 N 组移除」）。
// 物理删（非移除关联）：确认后调 batch_delete_platforms 原子事务。
// AlertDialog 走 Radix Portal（替代 shared/Modal，liquid glass 居中由 Portal 保证）。

import { useMemo } from "react";
import type { TFunction } from "i18next";
import type { Platform } from "../../services/api";
import { Button } from "@/components/ui/button";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogCancel,
} from "@/components/ui/alert-dialog";

export interface BatchDeleteModalProps {
  /** 待删平台全量信息（全列可滚展示）。 */
  open: boolean;
  platforms: Platform[];
  /** 每个平台所属分组名（跨组警告数据）；key = platformId，value = group names。
   *  未命中 = 仅属当前分组（单组），不显跨组警告。 */
  groupNamesByPlatform: Record<number, string[]>;
  /** 确认删除：父级执行 invoke + 刷新 + toast，完成后 onClose。 */
  onConfirm: () => void;
  onClose: () => void;
  /** 删除进行中（invoke 等），禁用按钮 + 按钮文案切换。 */
  busy?: boolean;
  t: TFunction;
}

export function BatchDeleteModal({
  open,
  platforms,
  groupNamesByPlatform,
  onConfirm,
  onClose,
  busy = false,
  t,
}: BatchDeleteModalProps) {
  // 跨组平台（属 >1 分组）→ 需显式警告
  const crossGroupPlatforms = useMemo(
    () => platforms.filter(p => (groupNamesByPlatform[p.id]?.length ?? 0) > 1),
    [platforms, groupNamesByPlatform],
  );
  const hasCrossGroup = crossGroupPlatforms.length > 0;

  return (
    <AlertDialog
      open={open}
      onOpenChange={(next) => {
        if (!next && !busy) onClose();
      }}
    >
      <AlertDialogContent
        className="glass-elevated"
        style={{ maxWidth: 520, maxHeight: "80vh", overflowY: "auto", padding: "20px 22px" }}
        // ponytail: AlertDialog 设计上禁外部点击关闭（ radix 不暴露 onPointerDownOutside）；
        // busy 时阻止 Escape 关闭（onOpenChange 也会因 busy 回滚，双保险防 invoke 中关闭）。
        onEscapeKeyDown={(e) => { if (busy) e.preventDefault(); }}
      >
        <AlertDialogHeader>
          <AlertDialogTitle>{t("group.batchDeleteTitle", "批量删除平台")}</AlertDialogTitle>
          <AlertDialogDescription>
            {t("group.batchDeleteDesc", "将永久删除以下 {{count}} 个平台及其所有分组关联，此操作不可撤销。", { count: platforms.length })}
          </AlertDialogDescription>
        </AlertDialogHeader>

        {hasCrossGroup && (
          <div style={{
            fontSize: 12.5,
            color: "var(--color-danger)",
            lineHeight: 1.55,
            padding: "8px 12px",
            borderRadius: "var(--radius-sm)",
            background: "var(--bg-glass)",
            border: "1px solid var(--color-danger)",
          }}>
            {t(
              "group.batchDeleteCrossGroupWarning",
              "其中 {{count}} 个平台属于多个分组，删除后将从所有分组移除：",
              { count: crossGroupPlatforms.length },
            )}
          </div>
        )}

        <div style={{
          display: "flex",
          flexDirection: "column",
          gap: 4,
          maxHeight: "40vh",
          overflowY: "auto",
          padding: "6px 10px",
          borderRadius: "var(--radius-sm)",
          background: "var(--bg-glass)",
          border: "1px solid var(--border)",
        }}>
          {platforms.map(p => {
            const groups = groupNamesByPlatform[p.id] ?? [];
            const isCross = groups.length > 1;
            return (
              <div key={p.id} style={{
                display: "flex",
                flexDirection: "column",
                gap: 2,
                padding: "5px 0",
                borderBottom: "1px solid var(--border)",
              }}>
                <div style={{ display: "flex", alignItems: "center", gap: 6, minWidth: 0 }}>
                  <svg width="12" height="12" viewBox="0 0 14 14" fill="none" stroke="var(--color-danger)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}>
                    <path d="M2 4h10M5 4V2h4v2M4 4v8a1 1 0 001 1h4a1 1 0 001-1V4" />
                  </svg>
                  <span style={{ fontSize: 13, fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {p.name}
                  </span>
                </div>
                {isCross && (
                  <div style={{ fontSize: 11.5, color: "var(--color-danger)", paddingLeft: 18, lineHeight: 1.4 }}>
                    {t("group.batchDeleteCrossGroupItem", "属 {{count}} 组：{{groups}}", { count: groups.length, groups: groups.join("、") })}
                  </div>
                )}
              </div>
            );
          })}
        </div>

        <AlertDialogFooter>
          <AlertDialogCancel disabled={busy}>{t("action.cancel", "取消")}</AlertDialogCancel>
          <Button variant="destructive" disabled={busy} onClick={onConfirm}>
            {busy
              ? t("group.batchDeleting", "删除中…")
              : t("group.batchDeleteConfirm", "删除 {{count}} 个平台", { count: platforms.length })}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
