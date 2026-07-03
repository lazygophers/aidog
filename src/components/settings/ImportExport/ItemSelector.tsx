// 逐项勾选器：全局头（全选/反选 + 计数）+ 按菜单组分组可折叠，每组内逐项 checkbox。
// 导出/导入共用。抽自原 ImportExport.tsx L926-1085。分组逻辑由 groupOf 注入
// （默认按菜单组聚合，platform/group/group_platform 三 scope 各自独立、setting 子归类）。

import { useState } from "react";
import type { TFunction } from "i18next";
import type { ImportItem } from "../../../services/api";
import { SectionIcon } from "../editors";
import { StatChip } from "../../shared/StatChip";
import type { ColorLevel } from "../../shared/colorScale";
import { SCOPE_ICON, settingLabelKey } from "./meta";
import { TextButton, CheckBox, Chevron } from "./primitives";

export function ItemSelector({
  items,
  selected,
  onToggle,
  onGroupSet,
  onAllSet,
  itemKey,
  groupOf,
  groupLabel,
  groupIcon,
  t,
}: {
  items: ImportItem[];
  selected: Set<string>;
  onToggle: (it: ImportItem) => void;
  /** 组级全选/反选：传入组内全部 items + select 方向。 */
  onGroupSet: (groupItems: ImportItem[], select: boolean) => void;
  onAllSet: (select: boolean) => void;
  itemKey: (scope: string, key: string) => string;
  /** item → 分组 id（默认菜单组）。 */
  groupOf: (it: ImportItem) => string;
  groupLabel: (groupId: string) => string;
  groupIcon: (groupId: string) => string;
  t: TFunction;
}) {
  // 按菜单组分组（保持出现顺序）。
  const groups: { gid: string; items: ImportItem[] }[] = [];
  for (const it of items) {
    const gid = groupOf(it);
    let g = groups.find((x) => x.gid === gid);
    if (!g) {
      g = { gid, items: [] };
      groups.push(g);
    }
    g.items.push(it);
  }
  // 默认展开（条目多时用户可手动折叠）。
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const toggleCollapse = (scope: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(scope)) next.delete(scope);
      else next.add(scope);
      return next;
    });

  const total = items.length;
  const selCount = items.filter((it) => selected.has(itemKey(it.scope, it.key))).length;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
        <strong style={{ fontSize: 14, color: "var(--text-primary)" }}>
          {t("importExport.selectItems", "选择导入项")}
        </strong>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <TextButton onClick={() => onAllSet(true)}>{t("importExport.selectAll", "全选")}</TextButton>
          <TextButton onClick={() => onAllSet(false)}>{t("importExport.deselectAll", "反选")}</TextButton>
          <StatChip value={`${selCount} / ${total}`} label={t("importExport.selectedLabel", "已选")} level={(selCount > 0 ? "success" : "neutral") as ColorLevel} />
        </div>
      </div>

      {groups.map((g) => {
        const open = !collapsed.has(g.gid);
        const gSel = g.items.filter((it) => selected.has(itemKey(it.scope, it.key))).length;
        const allOn = gSel === g.items.length;
        const someOn = gSel > 0 && !allOn;
        // skills 条目落在「扩展」组；只要本组含 skills 且全未选则提示。
        const hasSkills = g.items.some((it) => it.scope === "skills");
        const skillsSel = g.items.filter((it) => it.scope === "skills" && selected.has(itemKey(it.scope, it.key))).length;
        return (
          <div
            key={g.gid}
            className="glass-surface"
            style={{ borderRadius: "var(--radius-md)", border: "1px solid var(--border)", overflow: "hidden" }}
          >
            {/* 组头：折叠箭头 + 组复选框（全选/反选本组）+ 标题 + 计数 */}
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                padding: "10px 12px",
                cursor: "pointer",
                background: "var(--bg-glass)",
              }}
              onClick={() => toggleCollapse(g.gid)}
            >
              <Chevron open={open} />
              <span
                onClick={(e) => {
                  e.stopPropagation();
                  onGroupSet(g.items, !allOn);
                }}
                style={{ display: "inline-flex" }}
              >
                <CheckBox checked={allOn} indeterminate={someOn} />
              </span>
              <SectionIcon name={groupIcon(g.gid)} size={14} style={{ color: "var(--text-secondary)" }} />
              <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>{groupLabel(g.gid)}</span>
              {/* 方案 C 后 skills 默认勾选；用户手动全清 skills 时提醒（npx 操作需留意） */}
              {hasSkills && skillsSel === 0 && (
                <span
                  style={{
                    fontSize: 11,
                    color: "var(--color-warning)",
                    background: "var(--color-warning-bg)",
                    padding: "2px 6px",
                    borderRadius: "var(--radius-sm)",
                    marginLeft: 4,
                  }}
                >
                  {t("importExport.skillsScopeHint", "未选 Skills（确认无需导入）")}
                </span>
              )}
              <span style={{ fontSize: 12, color: "var(--text-tertiary)", marginLeft: "auto" }}>
                {gSel} / {g.items.length}
              </span>
            </div>

            {open && (
              <div style={{ display: "flex", flexDirection: "column" }}>
                {g.items.map((it) => {
                  const k = itemKey(it.scope, it.key);
                  const on = selected.has(k);
                  // setting scope 的 label 后端存裸 key（`app:theme` 等稳定标识），前端按 (scope:key) 映射本地化；
                  // 未命中映射（新增/未知 setting key）→ 回退裸 key（不崩不空）。
                  const settingLk = settingLabelKey(it);
                  const displayLabel = settingLk ? t(settingLk, it.label) : it.label;
                  return (
                    <div
                      key={k}
                      onClick={() => onToggle(it)}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 10,
                        padding: "8px 12px 8px 34px",
                        cursor: "pointer",
                        borderTop: "1px solid var(--border)",
                        transition: "var(--transition)",
                      }}
                    >
                      <CheckBox checked={on} />
                      <SectionIcon name={SCOPE_ICON[it.scope] ?? "folder"} size={12} style={{ color: "var(--text-tertiary)", flexShrink: 0 }} />
                      <span style={{ fontSize: 13, color: "var(--text-primary)", wordBreak: "break-all", flex: 1 }}>{displayLabel}</span>
                      {it.conflict && (
                        <StatChip value={t("importExport.conflictTag", "冲突")} label="" level={"warning" as ColorLevel} />
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
