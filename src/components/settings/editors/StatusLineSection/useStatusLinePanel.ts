// ponytail: 自 StatusLineSection.tsx L301-480 外迁，零逻辑变更。
// 收 Panel 全部 state + derived + actions + effect。

import { useState, useEffect } from "react";
import {
  type RowAlign,
  type StatusLineSegment,
  type SegmentType,
  DEFAULT_SEGMENTS,
  DEFAULT_SUBAGENT_SEGMENTS,
  normalizeSegments,
  SEGMENT_DEF_MAP,
} from "../../statusline-gen";
import { statuslineApi } from "../../../../services/api";

export type ScriptType = "statusline" | "subagent";

export function useStatusLinePanel({
  config,
  updateField,
  scriptType,
}: {
  config: Record<string, any>;
  updateField: (field: string, value: any) => void;
  scriptType: ScriptType;
}) {
  const isMain = scriptType === "statusline";
  const aidogKey = isMain ? "_aidog_statusline" : "_aidog_subagent_statusline";
  const fieldName = isMain ? "statusLine" : "subagentStatusLine";

  const stored = (config[aidogKey] ?? {}) as Record<string, any>;
  const enabled = !!stored.enabled;
  const mode: "builtin" | "custom" = stored.mode === "custom" ? "custom" : "builtin";
  const customCommand = typeof stored.customCommand === "string" ? stored.customCommand : "";

  // Segments — main and subagent share the same editor; only the first-run /
  // reset default layout differs.
  const defaultSegments = isMain ? DEFAULT_SEGMENTS : DEFAULT_SUBAGENT_SEGMENTS;
  const segments: StatusLineSegment[] =
    stored.segments ?? defaultSegments.map(s => ({ ...s }));

  const [showScript, setShowScript] = useState(false);
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

  // Script preview — rendered by Rust (same generator that materializes the real
  // file during sync), so what's shown is exactly what lands on disk. Read-only:
  // the actual .py files are (re)written by do_sync_group_settings on every
  // startup and on every settings change, never from here.
  const [scriptPreview, setScriptPreview] = useState("");
  useEffect(() => {
    if (!showScript || !enabled || mode !== "builtin") return;
    let cancelled = false;
    statuslineApi.preview(scriptType, stored).then(src => {
      if (!cancelled) setScriptPreview(src);
    }).catch(e => console.error("preview_statusline_script:", e));
    return () => { cancelled = true; };
    // stored is a fresh object each render — key off its serialized content.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showScript, enabled, mode, scriptType, JSON.stringify(stored)]);

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

  return {
    // derived
    isMain, aidogKey, fieldName, stored, enabled, mode, customCommand, defaultSegments,
    segments, scriptPreview,
    // state
    showScript, setShowScript,
    editSeg, setEditSeg,
    showAddMenu, setShowAddMenu,
    // actions
    handleToggle, updateSegments, deleteRow, handleApplyCustom,
    switchMode, addSegment, addRow, resetToDefaultLayout, cycleRowAlign,
  };
}

export type StatusLinePanelData = ReturnType<typeof useStatusLinePanel>;
