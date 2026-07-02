// ─── StatusLine Section (structured editor) ────────────────────
// Extracted verbatim from editors.tsx (arch-redesign phase 3).
// ponytail: 861 行超 800，阶段 4 二次拆 SegmentEditModal/StatusLinePanel。

import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { IconClose, IconMenu, IconEdit } from "../../icons";
import { SortableList } from "../../SortableList";
import { statuslineApi } from "../../../services/api";
import { type SettingField } from "../../../services/claude-settings-schema";
import {
  type RowAlign,
  type StatusLineSegment,
  type SegmentType,
  VALUE_COLORABLE,
  hexToRgb,
  SEGMENT_DEF_MAP,
  SEGMENT_CATEGORIES,
  STATUSLINE_DATA_FIELDS,
  DEFAULT_SEGMENTS,
  DEFAULT_SUBAGENT_SEGMENTS,
  generateStatusLineScript,
  generateSubagentStatusLineScript,
  groupRows,
  normalizeSegments,
  isRowLeaderSeg,
  PREVIEW_METRIC,
} from "../statusline-gen";
import { F, S } from "./tokens";
import { SectionIcon } from "./icons";
import { Toggle, Hint } from "./_shared";
import { FieldRenderer } from "./FieldRenderer";


/** Map a mock metric to the same semantic color the bash thresholds produce. */
function autoColorPreviewHex(type: SegmentType): string {
  const m = PREVIEW_METRIC[type] ?? 0;
  if (type === "cost" || type === "cost-usd") {
    if (m > 1000) return "var(--color-danger)";
    if (m > 100) return "var(--color-warning)";
    return "var(--color-success)";
  }
  if (type === "context-remaining") {
    if (m < 20) return "var(--color-danger)";
    if (m < 40) return "var(--color-warning)";
    return "var(--color-success)";
  }
  if (type === "session-duration" || type === "api-duration") {
    if (m > 300) return "var(--color-danger)";
    if (m > 60) return "var(--color-warning)";
    return "var(--color-success)";
  }
  if (m > 80) return "var(--color-danger)";
  if (m > 60) return "var(--color-warning)";
  return "var(--color-success)";
}

/** Resolve the preview color for a segment (fixed hex or autoColor), or null. */
function previewColor(seg: StatusLineSegment): string | null {
  if (seg.autoColor && VALUE_COLORABLE.has(seg.type)) return autoColorPreviewHex(seg.type);
  const rgb = hexToRgb(seg.color);
  return rgb ? `#${rgb.map(v => v.toString(16).padStart(2, "0")).join("")}` : null;
}

/** Render a colored, row-grouped, aligned live preview of the segments. */
function StatusLinePreview({ segments, empty }: { segments: StatusLineSegment[]; empty: string }) {
  const active = segments.filter(s => s.enabled);
  if (active.length === 0) {
    return <span style={{ color: "var(--text-tertiary)" }}>{empty}</span>;
  }
  const rows = groupRows(active);
  return (
    <>
      {rows.map((row, ri) => (
        <div key={ri} style={{
          display: "flex",
          justifyContent: row.align === "center" ? "center" : row.align === "right" ? "flex-end" : "flex-start",
        }}>
          {row.segs.map((seg) => {
            const def = SEGMENT_DEF_MAP.get(seg.type);
            if (!def) return null;
            const color = previewColor(seg);
            const opts = { ...def.defaultOptions, ...seg.options };
            const affixPre = typeof opts.affixPre === "string" ? opts.affixPre : "";
            const affixSuf = typeof opts.affixSuf === "string" ? opts.affixSuf : "";
            return (
              <span key={seg.id} style={color ? { color } : undefined}>
                {affixPre}{def.toPreview(opts)}{affixSuf}
              </span>
            );
          })}
        </div>
      ))}
    </>
  );
}

// ── Segment Edit Modal ──

function SegmentEditModal({
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

  return (
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
    </div>
  );
}

// ── StatusLine Panel Component ──

function StatusLinePanel({
  config,
  updateField,
  scriptType,
  t,
}: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
  scriptType: "statusline" | "subagent";
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const isMain = scriptType === "statusline";
  const aidogKey = isMain ? "_aidog_statusline" : "_aidog_subagent_statusline";
  const fieldName = isMain ? "statusLine" : "subagentStatusLine";

  const stored = (config[aidogKey] ?? {}) as Record<string, any>;
  const enabled = !!stored.enabled;
  // Generation mode: "builtin" → aidog structured segments; "custom" → user-supplied
  // native statusLine command (no aidog script generated). Back-compat: default builtin.
  const mode: "builtin" | "custom" = stored.mode === "custom" ? "custom" : "builtin";
  const customCommand: string = typeof stored.customCommand === "string" ? stored.customCommand : "";

  // Segments — main and subagent share the same editor; only the first-run /
  // reset default layout differs.
  const defaultSegments = isMain ? DEFAULT_SEGMENTS : DEFAULT_SUBAGENT_SEGMENTS;
  const segments: StatusLineSegment[] =
    stored.segments ?? defaultSegments.map(s => ({ ...s }));

  const [showScript, setShowScript] = useState(false);
  const [saving, setSaving] = useState(false);
  const [editSeg, setEditSeg] = useState<StatusLineSegment | null>(null);
  const [showAddMenu, setShowAddMenu] = useState(false);

  const setStored = (patch: Record<string, any>) => {
    updateField(aidogKey, { ...stored, ...patch });
  };

  const handleToggle = (val: boolean) => {
    if (!val) {
      updateField(fieldName, undefined);
      setStored({ enabled: false });
    } else {
      setStored({ enabled: true });
    }
  };

  const updateSegments = (next: StatusLineSegment[]) => setStored({ segments: normalizeSegments(next) });

  /**
   * Delete an entire row by its leader segment id. Resolves the row membership
   * from the *current* derived grouping (over ALL segments, enabled or not, so
   * the visual row and the deleted set always match), then removes exactly those
   * segment ids. Fixes the bug where, after dragging a segment into another row,
   * deleting "that row" removed the wrong segment set and dropped moved content.
   */
  const deleteRow = (leaderId: string) => {
    // Derive rows over the full segment list (matches the rendered grouping,
    // which keys off `newline` regardless of enabled state).
    const rows: StatusLineSegment[][] = [];
    let cur: StatusLineSegment[] | null = null;
    for (const seg of segments) {
      if (cur === null || (seg.newline && cur.length > 0)) {
        cur = [];
        rows.push(cur);
      }
      cur.push(seg);
    }
    const row = rows.find(r => r.some(s => s.id === leaderId));
    if (!row) return;
    const ids = new Set(row.map(s => s.id));
    updateSegments(segments.filter(s => !ids.has(s.id)));
  };

  // Generate script — 必须按 scriptType 分流, 否则 subagent 文件会被写入主脚本内容
  // (Claude Code 期望 subagent 输出每任务一行 JSONL, 写错→输出乱→CC 回退默认 `◯ <type> <desc> <dur>`)
  const scriptPreview = scriptType === "subagent"
    ? generateSubagentStatusLineScript(segments)
    : generateStatusLineScript(segments);


  const handleSave = async () => {
    setSaving(true);
    try {
      const command = await statuslineApi.generate(scriptType, scriptPreview);
      const value: Record<string, any> = { type: "command", command };
      updateField(fieldName, value);
    } catch (e: any) {
      console.error("generate_statusline_script:", e);
    }
    setSaving(false);
  };

  // Live-preview convenience only: keep the native `statusLine` / `subagentStatusLine`
  // draft field roughly in sync while the user edits builtin segments, so the JSON
  // view reflects changes without a save round-trip. This is NO LONGER the
  // persistence path — Settings.handleSave → materializeStatuslineFields is the
  // authoritative, race-free materializer (covers disabled/custom/subagent too,
  // which this effect deliberately does not touch). The effect keys off the *real
  // inputs* (scriptPreview / padding / hideVim / enabled), NOT the generated path,
  // and skips the write when the value is unchanged — keeping `updateField`
  // idempotent so the dirty state never thrashes / loops. Because the save writes
  // the same stable command path, the two never conflict.
  const lastWrittenRef = useRef<string>("");
  useEffect(() => {
    if (!enabled || mode !== "builtin") return;
    let cancelled = false;
    const timer = setTimeout(async () => {
      try {
        const command = await statuslineApi.generate(scriptType, scriptPreview);
        if (cancelled) return;
        const value: Record<string, any> = { type: "command", command };
  
        const signature = JSON.stringify(value);
        // Skip when the field already holds this exact value → no spurious dirty.
        const current = config[fieldName];
        if (signature === lastWrittenRef.current && JSON.stringify(current) === signature) return;
        lastWrittenRef.current = signature;
        updateField(fieldName, value);
      } catch (e: any) {
        console.error("auto generate_statusline_script:", e);
      }
    }, 500);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // Depends on real inputs only (scriptPreview captures segments/template).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, mode, scriptPreview, scriptType, isMain, fieldName]);

  // Apply custom mode: write the native Claude Code statusLine command directly,
  // bypassing aidog script generation. Empty command clears the field.
  const handleApplyCustom = () => {
    const cmd = customCommand.trim();
    if (!cmd) {
      updateField(fieldName, undefined);
      return;
    }
    const value: Record<string, any> = { type: "command", command: cmd };
    updateField(fieldName, value);
  };

  // Switch generation mode. Clears the live native field so the two modes never
  // leave a stale config behind (user re-applies in the newly selected mode).
  const switchMode = (next: "builtin" | "custom") => {
    if (next === mode) return;
    updateField(fieldName, undefined);
    setStored({ mode: next });
  };

  const addSegment = (type: SegmentType, newline = false) => {
    const def = SEGMENT_DEF_MAP.get(type);
    if (!def) return;
    const newSeg: StatusLineSegment = {
      id: `s${Date.now()}`,
      type,
      enabled: true,
      newline,
      options: { ...def.defaultOptions },
    };
    updateSegments([...segments, newSeg]);
    setShowAddMenu(false);
  };

  // Add a brand-new row: append a model segment that starts a new line.
  const addRow = () => {
    addSegment("model", segments.length > 0);
  };

  // Restore the built-in default 3-line layout (segments + empty affix-carried
  // separator). Explicit user action only — never auto-applied over a saved layout.
  const resetToDefaultLayout = () => {
    setStored({
      segments: defaultSegments.map(s => ({ ...s, options: { ...s.options } })),
    });
  };

  // Toggle alignment on the row that owns the given segment (set on its leader).
  const cycleRowAlign = (segId: string) => {
    const active = segments.filter(s => s.enabled);
    const idx = active.findIndex(s => s.id === segId);
    if (idx < 0) return;
    // Walk back to the row leader (first seg or newline=true).
    let leaderIdx = idx;
    while (leaderIdx > 0 && !active[leaderIdx].newline) leaderIdx--;
    const leaderId = active[leaderIdx].id;
    const order: RowAlign[] = ["left", "center", "right"];
    const cur = active[leaderIdx].align ?? "left";
    const nextAlign = order[(order.indexOf(cur) + 1) % order.length];
    updateSegments(segments.map(s => s.id === leaderId ? { ...s, align: nextAlign } : s));
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: S.gap }}>
      {/* Enable toggle */}
      <div style={{
        display: "flex", alignItems: "center", gap: 12,
        padding: "12px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
      }}>
        <Toggle active={enabled} onChange={handleToggle} />
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: F.label, fontWeight: 600, color: "var(--text-primary)" }}>
            {isMain ? t("statusline.useBuiltin", "使用内置状态栏") : t("statusline.useBuiltinSubagent", "使用内置子代理状态栏")}
          </div>
          <Hint>{isMain
            ? t("statusline.builtinDesc", "开启后 aidog 生成脚本到 ~/.aidog/aidog-statusline.sh")
            : t("statusline.builtinSubagentDesc", "开启后 aidog 生成脚本到 ~/.aidog/aidog-subagent-statusline.sh")}</Hint>
        </div>
        {enabled && (
          <span style={{
            fontSize: F.small, fontWeight: 600, color: "var(--color-success)",
            padding: "2px 8px", background: "color-mix(in srgb, var(--color-success) 12%, transparent)", borderRadius: "var(--radius-sm)",
          }}>● {t("statusline.enabled", "已启用")}</span>
        )}
      </div>

      {enabled && (
        <>
          {/* Mode selector: builtin structured segments vs custom native command */}
          <div style={{ display: "flex", gap: 6 }}>
            {(["builtin", "custom"] as const).map(m => {
              const active = mode === m;
              return (
                <button key={m} type="button"
                  style={{
                    flex: 1, padding: "8px 12px", fontSize: F.body, fontWeight: active ? 600 : 400,
                    color: active ? "var(--accent)" : "var(--text-secondary)",
                    background: active ? "var(--accent-subtle, rgba(0,122,255,0.1))" : "transparent",
                    border: `1px solid ${active ? "var(--accent)" : "var(--border)"}`,
                    borderRadius: "var(--radius-sm)", cursor: "pointer",
                  }}
                  onClick={() => switchMode(m)}>
                  {m === "builtin"
                    ? t("statusline.modeBuiltin", "内置结构化")
                    : t("statusline.modeCustom", "自定义脚本")}
                </button>
              );
            })}
          </div>
        </>
      )}

      {enabled && mode === "custom" && (
        <div style={{
          padding: "12px 16px", background: "var(--bg-surface)", borderRadius: "var(--radius-md)",
          border: "1px solid var(--border)", display: "flex", flexDirection: "column", gap: 12,
        }}>
          <Hint>{t("statusline.customDesc", "按原生 statusLine 格式分字段填写，写入 settings 的 command 字段，不生成 aidog 脚本")}</Hint>
          {/* type — 固定 command，只读展示 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <label style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("statusline.customType", "类型")}</label>
            <input className="input" readOnly value="command"
              style={{ fontSize: F.body, padding: S.inputPad, width: 140, opacity: 0.7, fontFamily: '"SF Mono", "Fira Code", monospace' }} />
          </div>
          {/* command — 脚本路径 / 命令 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <label style={{ fontSize: F.hint, color: "var(--text-secondary)" }}>{t("statusline.customCommand", "命令 / 脚本路径")}</label>
            <input className="input" style={{ fontSize: F.body, padding: S.inputPad }}
              value={customCommand}
              placeholder={t("statusline.customPlaceholder", "~/.claude/my-statusline.sh 或 inline 命令")}
              onChange={(e) => setStored({ customCommand: e.target.value })} />
            <Hint>{t("statusline.customCommandDesc", "支持绝对路径、~ 路径或内联命令")}</Hint>
          </div>
          <div style={{ display: "flex", justifyContent: "flex-end" }}>
            <button className="btn btn-primary" style={{ fontSize: F.body, padding: S.btnPad }}
              onClick={handleApplyCustom}>
              {t("statusline.applyCustom", "应用自定义脚本")}
            </button>
          </div>
        </div>
      )}

      {enabled && mode === "builtin" && (
        <>
          {/* Preview */}
          <div style={{
            padding: "12px 16px", background: "var(--bg-surface)", borderRadius: "var(--radius-md)",
            border: "1px solid var(--border)",
          }}>
            <div style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginBottom: 6 }}>{t("statusline.preview")}</div>
            <div style={{
              fontFamily: '"SF Mono", "Fira Code", monospace', fontSize: F.body,
              color: "var(--text-primary)", lineHeight: 1.6,
            }}>
              <StatusLinePreview segments={segments} empty={t("statusline.previewEmpty")} />
            </div>
          </div>

          {/* ── Drag-sortable segment list (shared by main & subagent) ── */}
          <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <SortableList
                items={segments}
                onReorder={updateSegments}
                renderItem={(seg, handle) => {
                  const def = SEGMENT_DEF_MAP.get(seg.type);
                  if (!def) return null;
                  const leader = isRowLeaderSeg(segments, seg.id);
                  const segColor = previewColor(seg);
                  return (
                    <div style={{ marginBottom: 6 }}>
                    {/* Row-leader bar: new-line marker + alignment */}
                    {leader && (
                      <div style={{
                        display: "flex", alignItems: "center", gap: 8,
                        padding: "2px 4px 4px", fontSize: F.hint, color: "var(--text-tertiary)",
                      }}>
                        <span style={{ fontWeight: 600 }}>{t("statusline.rowLabel")}</span>
                        <button type="button" className="btn btn-ghost"
                          style={{ fontSize: F.hint, padding: "2px 8px", color: "var(--accent)" }}
                          onClick={() => cycleRowAlign(seg.id)}
                          title={t("statusline.rowAlign")}>
                          {t(`statusline.align.${seg.align ?? "left"}`)}
                        </button>
                        <button type="button" className="btn btn-ghost"
                          style={{ fontSize: F.hint, padding: "2px 8px", color: "var(--text-tertiary)" }}
                          onClick={() => deleteRow(seg.id)}
                          title={t("statusline.deleteRow", "删除整行")}>
                          {t("statusline.deleteRow", "删除整行")}
                        </button>
                      </div>
                    )}
                    <div className="glass-surface" style={{
                      display: "flex", alignItems: "center", gap: 10,
                      padding: "10px 12px",
                      borderRadius: "var(--radius-md)",
                      opacity: seg.enabled ? 1 : 0.45,
                      border: handle.isDragging ? "1px solid var(--accent)" : "1px solid var(--border)",
                      boxShadow: handle.isDragging ? "0 6px 20px rgba(0,0,0,0.18)" : "none",
                      transition: "opacity 150ms, border-color 150ms",
                    }}>
                      {/* Drag handle (only this element starts the drag) */}
                      <span
                        ref={handle.ref}
                        {...handle.attributes}
                        {...handle.listeners}
                        style={{
                          color: "var(--text-tertiary)", fontSize: F.body,
                          cursor: handle.isDragging ? "grabbing" : "grab",
                          userSelect: "none", touchAction: "none",
                          padding: "0 2px", lineHeight: 1,
                        }}
                        title={t("statusline.dragSort", "拖动排序")}
                      ><IconMenu size={15} /></span>
                      {/* Toggle */}
                      <Toggle active={seg.enabled} onChange={(v) => {
                        const next = segments.map(s => s.id === seg.id ? { ...s, enabled: v } : s);
                        updateSegments(next);
                      }} />
                      {/* Name */}
                      <span style={{ fontSize: F.body, fontWeight: 600, color: "var(--text-primary)", flexShrink: 0 }}>
                        {t(`statusline.seg.${def.type}.name`, def.name)}
                      </span>
                      {/* Inline preview (colored) */}
                      <span style={{
                        flex: 1, fontSize: F.hint,
                        color: segColor ?? "var(--text-tertiary)",
                        fontFamily: '"SF Mono", "Fira Code", monospace',
                        overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                      }}>
                        {def.toPreview({ ...def.defaultOptions, ...seg.options })}
                      </span>
                      {/* Break-to-new-line toggle (moves segment between rows) */}
                      <button type="button" className="btn btn-ghost btn-icon"
                        style={{
                          width: 24, height: 24, minWidth: 24, fontSize: F.hint,
                          color: seg.newline ? "var(--accent)" : "var(--text-tertiary)",
                        }}
                        title={t("statusline.toggleNewline")}
                        onClick={() => updateSegments(segments.map(s => s.id === seg.id ? { ...s, newline: !s.newline } : s))}>↵</button>
                      {/* Edit button */}
                      <button type="button" className="btn btn-ghost"
                        style={{ fontSize: F.hint, padding: "2px 8px", color: "var(--accent)" }}
                        onClick={() => setEditSeg({ ...seg })}>
                        <IconEdit size={13} />
                      </button>
                      {/* Delete */}
                      <button type="button" className="btn btn-ghost btn-icon"
                        style={{ width: 24, height: 24, minWidth: 24, fontSize: F.hint, color: "var(--text-tertiary)" }}
                        onClick={() => updateSegments(segments.filter((s) => s.id !== seg.id))}>
                        <IconClose size={13} />
                      </button>
                    </div>
                    </div>
                  );
                }}
              />

              {/* Add segment / row */}
              <div style={{ position: "relative", display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 4 }}>
                <button className="btn btn-ghost" style={{ fontSize: F.body, padding: "6px 14px", marginRight: "auto", color: "var(--text-tertiary)" }}
                  onClick={resetToDefaultLayout}
                  title={t("statusline.resetLayoutHint", "恢复内置默认 3 行布局")}>
                  {t("statusline.resetLayout", "恢复默认布局")}
                </button>
                <button className="btn btn-ghost" style={{ fontSize: F.body, padding: "6px 14px" }}
                  onClick={addRow}>
                  {t("statusline.addRow")}
                </button>
                <button className="btn btn-ghost" style={{ fontSize: F.body, padding: "6px 14px" }}
                  onClick={() => setShowAddMenu(!showAddMenu)}>
                  {t("statusline.addSegment")}
                </button>
                {showAddMenu && (
                  <div style={{
                    position: "absolute", bottom: "100%", right: 0, zIndex: 100,
                    background: "var(--bg-surface)", border: "1px solid var(--border)",
                    borderRadius: "var(--radius-md)", padding: 4,
                    maxHeight: 360, overflow: "auto", minWidth: 280,
                    boxShadow: "0 8px 32px rgba(0,0,0,0.2)",
                  }}>
                    {SEGMENT_CATEGORIES.map(cat => (
                      <div key={cat.id}>
                        <div style={{
                          padding: "6px 12px 2px", fontSize: F.small, fontWeight: 600,
                          color: "var(--text-tertiary)", textTransform: "uppercase", letterSpacing: 0.4,
                        }}>{t(`statusline.segCat.${cat.id}`, cat.label)}</div>
                        {cat.types.map(type => {
                          const d = SEGMENT_DEF_MAP.get(type);
                          if (!d) return null;
                          return (
                            <button key={d.type} type="button" style={{
                              display: "block", width: "100%", textAlign: "left",
                              padding: "6px 12px", fontSize: F.body,
                              background: "transparent", border: "none", borderRadius: "var(--radius-sm)",
                              cursor: "pointer", color: "var(--text-primary)",
                            }}
                              onMouseEnter={(e) => { e.currentTarget.style.background = "var(--bg-glass)"; }}
                              onMouseLeave={(e) => { e.currentTarget.style.background = "transparent"; }}
                              onClick={() => addSegment(d.type)}>
                              <span style={{ fontWeight: 500 }}>{t(`statusline.seg.${d.type}.name`, d.name)}</span>
                              <span style={{ fontSize: F.hint, color: "var(--text-tertiary)", marginLeft: 8 }}>{t(`statusline.seg.${d.type}.desc`, d.desc)}</span>
                            </button>
                          );
                        })}
                      </div>
                    ))}
                  </div>
                )}
              </div>
          </div>


          {/* Script preview (collapsible) */}
          <div style={{
            padding: "10px 16px", background: "var(--bg-glass)", borderRadius: "var(--radius-md)",
          }}>
            <button type="button" className="btn btn-ghost"
              style={{ fontSize: F.body, padding: "4px 8px", display: "flex", alignItems: "center", gap: 4, width: "100%", justifyContent: "space-between" }}
              onClick={() => setShowScript(!showScript)}>
              <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
                <span style={{ transform: showScript ? "rotate(90deg)" : "rotate(0deg)", transition: "transform 150ms", display: "inline-block" }}>▶</span>
                {t("statusline.scriptPreview", "脚本预览")}
              </span>
              <span style={{ fontSize: F.small, color: "var(--text-tertiary)", fontFamily: '"SF Mono", "Fira Code", monospace' }}>
                ~/.aidog/aidog-{scriptType === "subagent" ? "subagent-" : ""}statusline.sh
              </span>
            </button>
            {showScript && (
              <pre style={{
                fontFamily: '"SF Mono", "Fira Code", monospace',
                fontSize: F.hint, lineHeight: 1.6,
                background: "var(--bg-surface)", borderRadius: "var(--radius-sm)",
                padding: 12, overflow: "auto", whiteSpace: "pre",
                color: "var(--text-primary)", margin: 0, marginTop: 8,
              }}>
                {scriptPreview}
              </pre>
            )}
          </div>

          {/* Apply button */}
          <div style={{ display: "flex", justifyContent: "flex-end" }}>
            <button className="btn btn-primary" style={{ fontSize: F.body, padding: S.btnPad }}
              onClick={handleSave} disabled={saving}>
              {saving ? t("statusline.generating", "生成中…") : t("statusline.applyGenerate", "应用并生成脚本")}
            </button>
          </div>
        </>
      )}

      {/* Edit modal */}
      {editSeg && (
        <SegmentEditModal
          segment={editSeg}
          isRowLeader={isRowLeaderSeg(segments, editSeg.id)}
          t={t}
          onClose={() => setEditSeg(null)}
          onSave={(patch) => {
            const idx = segments.findIndex(s => s.id === editSeg.id);
            if (idx >= 0) {
              const next = [...segments];
              next[idx] = { ...next[idx], ...patch };
              updateSegments(next);
            }
            setEditSeg(null);
          }}
        />
      )}
    </div>
  );
}

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
        <button type="button" className="btn btn-ghost"
          style={{ fontSize: F.body, padding: "4px 8px", display: "flex", alignItems: "center", gap: 4, width: "100%", justifyContent: "flex-start" }}
          onClick={() => setShowDataRef(!showDataRef)}>
          <span style={{ transform: showDataRef ? "rotate(90deg)" : "rotate(0deg)", transition: "transform 150ms", display: "inline-block" }}>▶</span>
          {t("statusline.dataFieldsRef", "可用数据字段参考")}
        </button>
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

// ── Import Diff Modal ──
