// ponytail: 自 StatusLineSection.tsx L301-480 外迁，零逻辑变更。
// 收 Panel 全部 state + derived + actions + effect。

import { useState, useEffect, useRef } from "react";
import {
  type RowAlign,
  type StatusLineSegment,
  type SegmentType,
  DEFAULT_SEGMENTS,
  DEFAULT_SUBAGENT_SEGMENTS,
  generateStatusLineScript,
  generateSubagentStatusLineScript,
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

  return {
    // derived
    isMain, aidogKey, fieldName, stored, enabled, mode, customCommand, defaultSegments,
    segments, scriptPreview,
    // state
    showScript, setShowScript,
    saving,
    editSeg, setEditSeg,
    showAddMenu, setShowAddMenu,
    // actions
    handleToggle, updateSegments, deleteRow, handleSave, handleApplyCustom,
    switchMode, addSegment, addRow, resetToDefaultLayout, cycleRowAlign,
  };
}

export type StatusLinePanelData = ReturnType<typeof useStatusLinePanel>;
