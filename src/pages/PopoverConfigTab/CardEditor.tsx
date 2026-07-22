// ─── 卡片编辑体（标题 / 摘要 / 显隐 / 删除 / scope 配置 / 尺寸 / 颜色）──
// 自 PopoverConfigTab.tsx 外迁（arch 阶段6 S5）。
import { useState, useEffect } from "react";
import type { PopoverItem, Group, Platform } from "../../services/api";
import { SIZE_OPTIONS, COLOR_PRESETS, TYPE_LABELS, defaultColor, isValidHex } from "./constants";
import { ScopeConfig } from "./ScopeConfig";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";

type TFn = (k: string, d: string, o?: Record<string, unknown>) => string;

export function CardEditor({
  item, t, platforms, groups, summary, onToggleVisible, onRemove, onUpdate,
}: {
  item: PopoverItem;
  t: TFn;
  platforms: Platform[];
  groups: Group[];
  summary: string;
  onToggleVisible: () => void;
  onRemove: () => void;
  onUpdate: (patch: Partial<PopoverItem>) => void;
}) {
  const color = item.color ?? defaultColor();
  const size = item.size ?? "m";
  return (
    <div style={{
      display: "flex", flexDirection: "column", gap: 8,
      padding: "8px 10px 8px 22px", borderRadius: 8,
      background: "var(--bg-glass)", border: "1px solid var(--border)",
      height: "100%", boxSizing: "border-box",
    }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 13, fontWeight: 500 }}>
            {t(TYPE_LABELS[item.item_type].key, TYPE_LABELS[item.item_type].fallback)}
          </div>
          <div className="text-tertiary" style={{ fontSize: 11, marginTop: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {summary}
          </div>
        </div>
        <Switch
          checked={item.visible}
          onCheckedChange={onToggleVisible}
          title={t("popover.toggleVisible", "显隐")}
        />
        <Button
          variant="ghost"
          style={{ fontSize: 12, padding: "2px 8px", height: 24, color: "var(--color-danger)" }}
          onClick={onRemove}
          title={t("common.delete", "删除")}
        >
          ✕
        </Button>
      </div>

      {/* scope 配置（cost_trend / platform_metric / group_*） */}
      <ScopeConfig item={item} t={t} platforms={platforms} groups={groups} onUpdate={onUpdate} />

      {/* 尺寸选择 */}
      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
        <span style={{ fontSize: 11, color: "var(--text-secondary)", width: 36 }}>{t("popover.size", "尺寸")}</span>
        {SIZE_OPTIONS.map((s) => (
          <Button
            key={s}
            variant={size === s ? "default" : "outline"}
            style={{
              fontSize: 11, padding: "2px 8px", height: 22,
            }}
            onClick={() => onUpdate({ size: s })}
            title={t(`popover.size_${s}`, s === "s" ? "小" : s === "l" ? "大" : "中")}
          >
            {s.toUpperCase()}
          </Button>
        ))}
      </div>

      {/* 颜色编辑 */}
      <div style={{ display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap" }}>
        <span style={{ fontSize: 11, color: "var(--text-secondary)", width: 36 }}>{t("popover.color", "颜色")}</span>
        {COLOR_PRESETS.map((c) => {
          const selected = c.mode === "follow"
            ? color.mode === "follow"
            : color.mode === "preset" && color.value === c.value;
          return (
            <Button
              key={`${c.mode}-${c.value}`}
              variant="ghost"
              title={c.mode === "follow" ? t("popover.colorFollow", "跟随") : c.value}
              style={{
                width: 18, height: 18, borderRadius: "50%", padding: 0, minWidth: 18,
                border: selected ? "2px solid var(--accent)" : "1px solid var(--glass-border)",
                background: c.css,
              }}
              onClick={() => onUpdate({ color: { mode: c.mode, value: c.value } })}
            />
          );
        })}
        {/* custom hex */}
        <CustomHexInput
          value={color.mode === "custom" ? color.value : ""}
          active={color.mode === "custom"}
          onChange={(hex) => onUpdate({ color: { mode: "custom", value: hex } })}
          t={t}
        />
      </div>
    </div>
  );
}

function CustomHexInput({
  value, active, onChange, t,
}: {
  value: string;
  active: boolean;
  onChange: (hex: string) => void;
  t: (k: string, d: string) => string;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => { setDraft(value); }, [value]);
  const valid = draft === "" || isValidHex(draft);
  return (
    <Input
      placeholder={t("popover.colorHex", "#RRGGBB")}
      value={draft}
      onChange={(e) => {
        const v = e.target.value;
        setDraft(v);
        if (isValidHex(v)) onChange(v.replace(/^#/, ""));
      }}
      style={{
        fontSize: 11, width: 80, height: 22, padding: "2px 6px",
        border: active ? "1px solid var(--accent)" : valid ? "1px solid var(--glass-border)" : "1px solid var(--color-danger)",
      }}
    />
  );
}
