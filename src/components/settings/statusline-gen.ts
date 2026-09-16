// ─── Statusline script generator (pure, no React) ──────────
// Extracted from editors.tsx: segment model + bash→Python script generation.
// Pure functions only — no React/Tauri imports — so a Node test harness can
// import these to build golden-output regression fixtures.
//
// arch-redesign phase 6 S6: large constant tables (SEGMENT_DEFS + derived) and
// the segment type definitions moved to ./statusline-segments. This file keeps
// the generation functions and re-exports every moved symbol so consumers
// import paths stay unchanged (barrel — data/function split, not a component).

// Re-export the moved types + constants so existing `from "./statusline-gen"`
// imports keep resolving verbatim (zero consumer churn).
export {
  type RowAlign,
  type SegmentType,
  type StatusLineSegment,
  type SegmentDef,
  VALUE_COLORABLE,
  GROUP_SEG_TYPES,
  SEGMENT_DEFS,
  SEGMENT_DEF_MAP,
  SEGMENT_CATEGORIES,
  DEFAULT_SEGMENTS,
  DEFAULT_SUBAGENT_SEGMENTS,
  STATUSLINE_DATA_FIELDS,
} from "./statusline-segments";

import {
  type StatusLineSegment,
  type RowAlign,
} from "./statusline-segments";

/** Parse "#RRGGBB" / "#RGB" → [r,g,b] (0–255) or null when invalid. */
export function hexToRgb(hex?: string): [number, number, number] | null {
  if (!hex) return null;
  let h = hex.trim().replace(/^#/, "");
  if (h.length === 3) h = h.split("").map(c => c + c).join("");
  if (!/^[0-9a-fA-F]{6}$/.test(h)) return null;
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

// ── Script generation from segments ──

/** Group active segments into rows (split on `newline`). Returns rows with align. */
export function groupRows(segments: StatusLineSegment[]): { align: RowAlign; segs: StatusLineSegment[] }[] {
  const rows: { align: RowAlign; segs: StatusLineSegment[] }[] = [];
  let cur: StatusLineSegment[] | null = null;
  for (const seg of segments) {
    if (cur === null || (seg.newline && cur.length > 0)) {
      cur = [];
      rows.push({ align: seg.align ?? "left", segs: cur });
    }
    cur.push(seg);
  }
  return rows;
}

/**
 * Re-derive `newline` flags so the row model stays self-consistent after any
 * structural mutation (drag-reorder, delete, enable-toggle).
 *
 * The row model is *derived* from `newline`: a row break is any segment with
 * `newline === true`, plus the implicit break before the first segment. Drag
 * reordering moves items in the flat array without touching `newline`, which can
 * leave the new first segment carrying `newline: true` (a redundant leading
 * break) or strand a row break inside the array in a way that silently merges
 * rows. Both make "this row" ambiguous and break per-row delete.
 *
 * Invariant enforced here: the first segment never carries `newline: true`
 * (its row break is implicit). All other `newline` flags are preserved, so the
 * visible row count and membership are stable across reorders.
 */
export function normalizeSegments(segments: StatusLineSegment[]): StatusLineSegment[] {
  if (segments.length === 0) return segments;
  return segments.map((s, i) =>
    i === 0 ? (s.newline ? { ...s, newline: false } : s) : s,
  );
}

/** True when the segment starts a row (first active segment, or newline=true). */
export function isRowLeaderSeg(segments: StatusLineSegment[], id: string): boolean {
  const active = segments.filter(s => s.enabled);
  const idx = active.findIndex(s => s.id === id);
  if (idx < 0) {
    // disabled segment — leads if it has explicit newline
    return !!segments.find(s => s.id === id)?.newline;
  }
  return idx === 0 || !!active[idx].newline;
}

/** Mock metric values used to drive autoColor preview (matches bash thresholds). */
export const PREVIEW_METRIC: Record<string, number> = {
  "context-pct": 65,
  "context-bar": 65,
  "cost": 12,          // cents
  "cost-usd": 12,      // cents
  "rate-limits": 41,
  "rate-limit-5h": 34,
  "rate-limit-7d": 62,
  "context-remaining": 49,
  "session-duration": 285, // seconds
  "api-duration": 15,      // seconds
};
