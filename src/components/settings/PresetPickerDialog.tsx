// 推荐配置预设选择弹窗（Radix Dialog，自带 Portal 居中）。票 04 接 Codex/Pi 时复用。
import { useTranslation } from "react-i18next";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from "@/components/ui/dialog";
import type { SettingsPreset } from "../../services/api";
import { F, S } from "./editors";

/** oss 显示名走 locale（静态 key），其余显示文件内名字。 */
export function usePresetName() {
  const { t } = useTranslation();
  return (p: SettingsPreset) => (p.id === "oss" ? t("settings.preset.name.oss") : p.name);
}

export function PresetPickerDialog({
  presets,
  onSelect,
  onCancel,
}: {
  presets: SettingsPreset[];
  onSelect: (p: SettingsPreset) => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const nameOf = usePresetName();
  return (
    <Dialog open onOpenChange={(o) => { if (!o) onCancel(); }}>
      <DialogContent id="preset-picker" className="glass-elevated" style={{ maxWidth: 420, padding: "22px 24px" }}>
        <DialogHeader>
          <DialogTitle style={{ fontSize: F.title, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("settings.preset.pickTitle")}
          </DialogTitle>
          <DialogDescription style={{ fontSize: F.body, color: "var(--text-secondary)", lineHeight: 1.6 }}>
            {t("settings.preset.pickBody")}
          </DialogDescription>
        </DialogHeader>
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {presets.map((p) => (
            <button
              key={p.id}
              id={`preset-option-${p.id}`}
              type="button"
              onClick={() => onSelect(p)}
              className="hover:bg-[var(--accent-subtle)] focus-visible:bg-[var(--accent-subtle)]"
              style={{
                textAlign: "start",
                fontSize: F.body,
                padding: S.btnPad,
                borderRadius: 8,
                border: "1px solid var(--border)",
                color: "var(--text-primary)",
                background: "transparent",
                cursor: "pointer",
              }}
            >
              {nameOf(p)}
            </button>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
