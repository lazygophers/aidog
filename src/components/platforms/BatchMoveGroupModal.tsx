// ─── 批量移组/加组弹窗（group-batch-ops s5）──────────────────
// 目标组下拉 + move/add radio。
// move = 从所有现组移除 + 加目标组；add = 仅加目标组保留现组。
// 目标组=当前组 → 禁用确认（无意义）。
// 确认 → 调 batch_move_group 原子事务。
// Dialog 走 Radix Portal（替代 shared/Modal，liquid glass 居中由 Portal 保证）。

import { useEffect, useState } from "react";
import type { TFunction } from "i18next";
import type { Platform } from "../../services/api";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

type MoveMode = "move" | "add";

export interface BatchMoveGroupModalProps {
  open: boolean;
  /** 待移组平台（全列可滚展示）。 */
  platforms: Platform[];
  /** 全部分组（目标组下拉数据源）。 */
  groups: { id: number; name: string }[];
  /** 当前分组 id（目标=当前 → 禁用确认）。 */
  currentGroupId: number;
  /** 确认移组：父级执行 invoke + 刷新 + toast，完成后 onClose。 */
  onConfirm: (targetGroupId: number, mode: MoveMode) => void;
  onClose: () => void;
  /** 移组中（invoke 等），禁用按钮 + 切文案。 */
  busy?: boolean;
  t: TFunction;
}

export function BatchMoveGroupModal({
  open,
  platforms,
  groups,
  currentGroupId,
  onConfirm,
  onClose,
  busy = false,
  t,
}: BatchMoveGroupModalProps) {
  const [targetGroupId, setTargetGroupId] = useState<number | "">("");
  const [mode, setMode] = useState<MoveMode>("move");

  useEffect(() => {
    if (open) {
      setTargetGroupId("");
      setMode("move");
    }
  }, [open]);

  const isCurrentGroup = targetGroupId === currentGroupId;
  const canConfirm = targetGroupId !== "" && !isCurrentGroup;

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
          <DialogTitle>{t("group.batchMoveGroupTitle", "批量移组")}</DialogTitle>
          <DialogDescription>
            {t("group.batchMoveGroupDesc", "将以下 {{count}} 个平台移动或加入到目标分组：", { count: platforms.length })}
          </DialogDescription>
        </DialogHeader>

        <div style={{ marginBottom: 10 }}>
          <span style={{ fontSize: 12, color: "var(--text-secondary)", fontWeight: 500, display: "block", marginBottom: 4 }}>
            {t("group.batchMoveGroupTarget", "目标分组")}
          </span>
          <Select
            value={targetGroupId === "" ? undefined : String(targetGroupId)}
            onValueChange={(v) => setTargetGroupId(v === "" ? "" : Number(v))}
          >
            <SelectTrigger style={{ fontSize: 13, width: "100%" }} className="input">
              <SelectValue placeholder={t("group.batchMoveGroupSelect", "选择目标分组…")} />
            </SelectTrigger>
            <SelectContent>
              {groups.map(g => (
                <SelectItem key={g.id} value={String(g.id)}>
                  {g.name}{g.id === currentGroupId ? ` (${t("group.batchMoveGroupCurrent", "当前组")})` : ""}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {isCurrentGroup && (
          <div style={{
            fontSize: 12, color: "var(--color-warning)", lineHeight: 1.5,
            marginBottom: 10, padding: "6px 10px", borderRadius: "var(--radius-sm)",
            background: "var(--bg-glass)", border: "1px solid var(--color-warning)",
          }}>
            {t("group.batchMoveGroupSameAsCurrent", "目标分组与当前分组相同，无需操作。")}
          </div>
        )}

        <RadioGroup
          value={mode}
          onValueChange={(v) => setMode(v as MoveMode)}
          style={{ display: "flex", gap: 16, marginBottom: 12 }}
        >
          {(["move", "add"] as const).map(m => (
            <div key={m} style={{ display: "inline-flex", alignItems: "center", gap: 5, fontSize: 13 }}>
              <RadioGroupItem value={m} id={`batch-move-${m}`} />
              <label htmlFor={`batch-move-${m}`} style={{ cursor: "pointer" }}>
                {m === "move"
                  ? t("group.batchMoveGroupModeMove", "移动（移出所有现组）")
                  : t("group.batchMoveGroupModeAdd", "加入（保留现组）")}
              </label>
            </div>
          ))}
        </RadioGroup>

        <div style={{
          display: "flex", flexDirection: "column", gap: 4,
          maxHeight: "28vh", overflowY: "auto",
          padding: "6px 10px", borderRadius: "var(--radius-sm)",
          background: "var(--bg-glass)", border: "1px solid var(--border)",
        }}>
          {platforms.map(p => (
            <div key={p.id} style={{
              display: "flex", alignItems: "center", gap: 6, minWidth: 0,
              padding: "5px 0", borderBottom: "1px solid var(--border)",
            }}>
              <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="var(--text-tertiary)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}>
                <path d="M2 6h8M8 4l2 2-2 2" />
              </svg>
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
          <Button
            onClick={() => targetGroupId !== "" && onConfirm(targetGroupId, mode)}
            disabled={busy || !canConfirm}
          >
            {busy
              ? t("group.batchMoveGroupApplying", "移组中…")
              : t("group.batchMoveGroupConfirm", "{{mode}} {{count}} 个平台", {
                  count: platforms.length,
                  mode: mode === "move"
                    ? t("group.batchMoveGroupModeMoveShort", "移动")
                    : t("group.batchMoveGroupModeAddShort", "加入"),
                })}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
