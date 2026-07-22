// ─── StatusLine Section facade ─────────────────────────────────
// arch-redesign 阶段 6 S7：自 892 行巨石拆为 facade + 子目录。
// 本文件仅编排（外层布局 + FileSuggestion + DataRef），主体逻辑见 StatusLineSection/。
// ponytail: 外部 import 路径 `editors/StatusLineSection` 不变（barrel 零 churn）。

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { STATUSLINE_DATA_FIELDS } from "../statusline-gen";
import { F, S } from "./tokens";
import { SectionIcon } from "./icons";
import { Hint } from "./_shared";
import { FieldRenderer } from "./FieldRenderer";
import { type SettingField } from "../../../services/claude-settings-schema";
import { StatusLinePanel } from "./StatusLineSection/StatusLinePanel";
import { Button } from "@/components/ui/button";

/** Combined section for status tab */
export function StatusLineSection({
  config,
  updateField,
  t,
}: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const [showDataRef, setShowDataRef] = useState(false);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
      {/* StatusLine */}
      <div style={{
        padding: 16, border: "1px solid var(--border)", borderRadius: "var(--radius-md)",
        display: "flex", flexDirection: "column", gap: 4,
      }}>
        <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-primary)", marginBottom: 8, display: "flex", alignItems: "center", gap: 6 }}>
          <SectionIcon name="status" size={15} />
          StatusLine
        </div>
        <StatusLinePanel config={config} updateField={updateField} scriptType="statusline" t={t} />
      </div>

      {/* SubagentStatusLine */}
      <div style={{
        padding: 16, border: "1px solid var(--border)", borderRadius: "var(--radius-md)",
        display: "flex", flexDirection: "column", gap: 4,
      }}>
        <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-primary)", marginBottom: 8, display: "flex", alignItems: "center", gap: 6 }}>
          <SectionIcon name="team" size={15} />
          SubagentStatusLine
        </div>
        <StatusLinePanel config={config} updateField={updateField} scriptType="subagent" t={t} />
      </div>

      {/* FileSuggestion (keep existing behavior) */}
      {(() => {
        const field: SettingField = {
          key: "fileSuggestion",
          label: "File Suggestion",
          type: "string",
          description: t("statusline.fileSuggestionDesc", "自定义文件建议脚本路径"),
          pathType: "file",
        };
        return (
          <FieldRenderer
            field={field}
            value={config.fileSuggestion}
            onChange={(v) => updateField("fileSuggestion", v)}
            t={t}
          />
        );
      })()}

      {/* Data reference panel */}
      <div style={{
        padding: 16, border: "1px solid var(--border)", borderRadius: "var(--radius-md)",
      }}>
        <Button variant="ghost" type="button" 
          style={{ fontSize: F.body, padding: "4px 8px", display: "flex", alignItems: "center", gap: 4, width: "100%", justifyContent: "flex-start" }}
          onClick={() => setShowDataRef(!showDataRef)}>
          <span style={{ transform: showDataRef ? "rotate(90deg)" : "rotate(0deg)", transition: "transform 150ms", display: "inline-block" }}>▶</span>
          {t("statusline.dataFieldsRef", "可用数据字段参考")}
        </Button>
        {showDataRef && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12, marginTop: 12 }}>
            <Hint>{t("statusline.dataFieldsHint", "Claude Code 通过 stdin 注入以下 JSON 字段，可在脚本中用 jq 提取")}</Hint>
            {STATUSLINE_DATA_FIELDS.map(group => (
              <div key={group.id}>
                <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 6 }}>
                  {t(`statusline.dataGroup.${group.id}`, group.group)}
                </div>
                <table style={{ width: "100%", borderCollapse: "collapse" }}>
                  {group.fields.map(f => (
                    <tr key={f.key} style={{ borderBottom: "1px solid var(--border)" }}>
                      <td style={{
                        padding: "4px 12px 4px 0", fontSize: F.hint,
                        fontFamily: '"SF Mono", "Fira Code", monospace',
                        color: "var(--accent)", whiteSpace: "nowrap",
                      }}>
                        {f.key}
                      </td>
                      <td style={{ padding: "4px 0", fontSize: F.hint, color: "var(--text-tertiary)" }}>
                        {f.desc}
                      </td>
                    </tr>
                  ))}
                </table>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
