// 冲突行：单条冲突项 + 3 段决策（覆盖/跳过/重命名）+ 重命名输入框。
// 抽自原 ImportExport.tsx L1142-1202（diff/冲突域）。依赖 Segmented。

import type { TFunction } from "i18next";
import type { ConflictItem, ImportDecision } from "../../../services/api";
import { SectionIcon } from "../editors";
import { StatChip } from "../../shared/StatChip";
import { SCOPE_ICON } from "./meta";
import { Segmented } from "./primitives";

export function ConflictRow({
  item,
  scopeLabel,
  current,
  onChange,
  t,
}: {
  item: ConflictItem;
  scopeLabel: string;
  current: ImportDecision;
  onChange: (d: ImportDecision) => void;
  t: TFunction;
}) {
  const isRename = current.kind === "rename";
  return (
    <div
      className="glass-surface"
      style={{
        padding: 12,
        borderRadius: "var(--radius-md)",
        border: "1px solid var(--border)",
        display: "flex",
        flexDirection: "column",
        gap: 8,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
        <StatChip
          icon={<SectionIcon name={SCOPE_ICON[item.scope] ?? "folder"} size={12} />}
          value={scopeLabel}
          label=""
        />
        <span style={{ fontWeight: 600, color: "var(--text-primary)", fontSize: 13, wordBreak: "break-all" }}>{item.key}</span>
      </div>
      <div style={{ fontSize: 12, color: "var(--text-tertiary)", lineHeight: 1.4 }}>{item.existing_summary}</div>
      <div style={{ display: "flex", gap: 10, alignItems: "center", flexWrap: "wrap" }}>
        <Segmented
          value={current.kind}
          options={[
            { id: "overwrite", label: t("importExport.overwrite", "覆盖") },
            { id: "skip", label: t("importExport.skip", "跳过") },
            { id: "rename", label: t("importExport.rename", "重命名") },
          ]}
          onSelect={(id) => {
            if (id === "rename") onChange({ kind: "rename", new_key: item.key + "-imported" });
            else onChange({ kind: id as "overwrite" | "skip" });
          }}
        />
        {isRename && (
          <input
            className="input"
            type="text"
            value={(current as { kind: "rename"; new_key: string }).new_key}
            onChange={(e) => onChange({ kind: "rename", new_key: e.target.value })}
            style={{ width: 220 }}
          />
        )}
      </div>
    </div>
  );
}
