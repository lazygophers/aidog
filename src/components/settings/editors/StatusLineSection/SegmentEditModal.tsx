// ponytail: 自 StatusLineSection.tsx L100-286 外迁，零逻辑变更。

import { useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import {
  type RowAlign,
  type StatusLineSegment,
  VALUE_COLORABLE,
  hexToRgb,
  SEGMENT_DEF_MAP,
} from "../../statusline-gen";
import { F, S } from "../tokens";
import { Toggle } from "../_shared";
import { autoColorPreviewHex } from "./preview";

export function SegmentEditModal({
  segment,
  isRowLeader,
  onSave,
  onClose,
  t,
}: {
  segment: StatusLineSegment;
  isRowLeader: boolean;
  onSave: (patch: Partial<StatusLineSegment>) => void;
  onClose: () => void;
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const def = SEGMENT_DEF_MAP.get(segment.type);
  const [opts, setOpts] = useState({ ...(def?.defaultOptions ?? {}), ...segment.options });
  const [newline, setNewline] = useState(segment.newline);
  const [color, setColor] = useState<string>(segment.color ?? "");
  const [autoColor, setAutoColor] = useState<boolean>(!!segment.autoColor);
  const [align, setAlign] = useState<RowAlign>(segment.align ?? "left");
  if (!def) return null;
  const canAutoColor = VALUE_COLORABLE.has(segment.type);
  const validHex = hexToRgb(color) != null;
  const effectiveColor = autoColor && canAutoColor
    ? autoColorPreviewHex(segment.type)
    : (validHex ? color : null);

  // portal 到 body：祖先 transform/backdrop-filter 会让 fixed 退化相对祖先，致弹窗只在 page 内居中。
  return createPortal(
    <div style={{
      position: "fixed", inset: 0, zIndex: 1000,
      display: "flex", alignItems: "center", justifyContent: "center",
      background: "rgba(0,0,0,0.5)", animation: "fadeIn 150ms ease both",
    }} onClick={onClose}>
      <div className="glass-elevated"
        style={{
          width: 420, maxHeight: "80vh", overflow: "auto",
          padding: 24, borderRadius: "var(--radius-lg)",
          animation: "fadeIn 200ms ease both",
        }}
        onClick={(e) => e.stopPropagation()}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
          <div style={{ fontSize: F.title, fontWeight: 600, color: "var(--text-primary)" }}>
            {t(`statusline.seg.${def.type}.name`, def.name)}
          </div>
          <button type="button" className="btn btn-ghost btn-icon"
            style={{ width: 28, height: 28, fontSize: F.body }}
            onClick={onClose}>×</button>
        </div>
        <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 16 }}>{t(`statusline.seg.${def.type}.desc`, def.desc)}</div>

        {/* Newline toggle */}
        <label style={{
          display: "flex", alignItems: "center", gap: 8, marginBottom: 16,
          padding: "8px 12px", background: "var(--bg-glass)", borderRadius: "var(--radius-sm)",
          fontSize: F.body, color: "var(--text-primary)", cursor: "pointer",
        }}>
          <Toggle active={newline} onChange={setNewline} />
          {t("statusline.segNewline")}
        </label>

        {/* Row alignment (only meaningful when this segment leads a row) */}
        {(isRowLeader || newline) && (
          <div style={{ marginBottom: 16 }}>
            <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 6 }}>{t("statusline.rowAlign")}</div>
            <div style={{ display: "flex", gap: 6 }}>
              {(["left", "center", "right"] as RowAlign[]).map(a => {
                const active = align === a;
                return (
                  <button key={a} type="button"
                    style={{
                      flex: 1, padding: "6px 10px", fontSize: F.body,
                      fontWeight: active ? 600 : 400,
                      color: active ? "var(--accent)" : "var(--text-secondary)",
                      background: active ? "var(--accent-subtle, rgba(0,122,255,0.1))" : "transparent",
                      border: `1px solid ${active ? "var(--accent)" : "var(--border)"}`,
                      borderRadius: "var(--radius-sm)", cursor: "pointer",
                    }}
                    onClick={() => setAlign(a)}>
                    {t(`statusline.align.${a}`)}
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {/* Color controls */}
        <div style={{ marginBottom: 16 }}>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 6 }}>{t("statusline.color")}</div>
          {canAutoColor && (
            <label style={{
              display: "flex", alignItems: "center", gap: 8, marginBottom: 10,
              fontSize: F.body, color: "var(--text-primary)", cursor: "pointer",
            }}>
              <Toggle active={autoColor} onChange={setAutoColor} />
              {t("statusline.autoColor")}
            </label>
          )}
          <div style={{
            display: "flex", alignItems: "center", gap: 8,
            opacity: autoColor && canAutoColor ? 0.45 : 1,
            pointerEvents: autoColor && canAutoColor ? "none" : "auto",
          }}>
            <input
              type="color"
              value={validHex ? `#${hexToRgb(color)!.map(v => v.toString(16).padStart(2, "0")).join("")}` : "#4a9eff"}
              onChange={(e) => setColor(e.target.value)}
              style={{
                width: 36, height: 30, padding: 0, border: "1px solid var(--border)",
                borderRadius: "var(--radius-sm)", background: "transparent", cursor: "pointer", flexShrink: 0,
              }}
            />
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}
              value={color} placeholder="#4A9EFF"
              onChange={(e) => setColor(e.target.value)} />
            <button type="button" className="btn btn-ghost"
              style={{ fontSize: F.hint, padding: "4px 10px", color: "var(--text-tertiary)" }}
              onClick={() => setColor("")}>
              {t("statusline.clearColor")}
            </button>
          </div>
        </div>

        {/* Type-specific fields */}
        {def.fields.length > 0 && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12, marginBottom: 16 }}>
            {def.fields.map(f => (
              <div key={f.key} style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <label style={{ fontSize: F.body, color: "var(--text-secondary)", minWidth: 100, flexShrink: 0 }}>
                  {t(`statusline.seg.${def.type}.field.${f.key}`, f.label)}
                </label>
                {f.type === "select" ? (
                  <select className="input" style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}
                    value={String(opts[f.key] ?? f.options?.[0] ?? "")}
                    onChange={(e) => setOpts({ ...opts, [f.key]: e.target.value })}>
                    {f.options?.map(o => <option key={o} value={o}>{o}</option>)}
                  </select>
                ) : f.type === "number" ? (
                  <input className="input" type="number" style={{ fontSize: F.body, padding: S.inputPad, flex: 1, width: 80 }}
                    value={opts[f.key] ?? ""} placeholder={f.placeholder}
                    onChange={(e) => setOpts({ ...opts, [f.key]: Number(e.target.value) })} />
                ) : (
                  <input className="input" style={{ fontSize: F.body, padding: S.inputPad, flex: 1 }}
                    value={String(opts[f.key] ?? "")} placeholder={f.placeholder}
                    onChange={(e) => setOpts({ ...opts, [f.key]: e.target.value })} />
                )}
              </div>
            ))}
          </div>
        )}

        {/* Preview */}
        <div style={{ marginBottom: 16 }}>
          <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 4 }}>{t("statusline.preview")}</div>
          <div style={{
            padding: "8px 14px", background: "var(--bg-surface)",
            borderRadius: "var(--radius-sm)", fontSize: F.body,
            fontFamily: '"SF Mono", "Fira Code", monospace',
            color: "var(--text-primary)",
          }}>
            <span style={effectiveColor ? { color: effectiveColor } : undefined}>
              {def.toPreview(opts)}
            </span>
          </div>
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button className="btn btn-ghost" style={{ fontSize: F.body, padding: S.btnPad }} onClick={onClose}>
            {t("statusline.cancel")}
          </button>
          <button className="btn btn-primary" style={{ fontSize: F.body, padding: S.btnPad }}
            onClick={() => {
              onSave({
                options: opts,
                newline,
                color: validHex ? color : undefined,
                autoColor: canAutoColor ? autoColor : undefined,
                align: (isRowLeader || newline) ? align : undefined,
              });
              onClose();
            }}>
            {t("statusline.save")}
          </button>
        </div>
      </div>
    </div>,
    document.body
  );
}
