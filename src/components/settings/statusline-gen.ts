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
  type SegmentType,
  type StatusLineSegment,
  type RowAlign,
  VALUE_COLORABLE,
  GROUP_SEG_TYPES,
  SEGMENT_DEF_MAP,
  DEFAULT_SEGMENTS,
  DEFAULT_SUBAGENT_SEGMENTS,
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

// ── Python script generation from segments ──
//
// The generated statusline / subagent scripts are Python (PEP723, stdlib only)
// executed by `uv run --script` or `python3` (see ScriptInvoker). Output is
// byte-for-byte identical to the former jq/printf/awk/sed bash implementation,
// guaranteed by the golden-output regression in scripts/statusline-golden/.
//
// Each script embeds the shared rendering engine (ENGINE_PY, auto-generated from
// scripts/statusline-golden/engine.py) plus a JSON segment config and a small
// entry point. The engine reads the config and renders at runtime, so all the
// number-formatting / ANSI / truncation parity logic lives in one place.

import { ENGINE_PY } from "./statusline-runtime";

/** PEP723 inline-metadata header (stdlib only; shebang is the python3 fallback). */
const PEP723_HEADER =
  `#!/usr/bin/env python3\n` +
  `# /// script\n` +
  `# requires-python = ">=3.8"\n` +
  `# dependencies = []\n` +
  `# ///\n`;

/** A serializable per-segment spec consumed by the runtime engine. */
interface SegmentSpec {
  type: SegmentType;
  opts: Record<string, any>;
  /** Fixed-color "r;g;b" triple, or null. */
  rgb: string | null;
  /** Value-driven auto color. */
  autoColor: boolean;
}

/**
 * Base64-encode a (possibly non-ASCII) JSON config string for safe embedding in
 * the generated Python source — avoids all quoting hazards (the config carries
 * arbitrary user prefixes / separators incl. quotes, backslashes, multibyte).
 * The script decodes it via `base64.b64decode(...).decode("utf-8")`.
 */
function b64(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  // btoa exists in the Tauri webview; Node test harness provides it too (>=16).
  return btoa(bin);
}

/** Resolve a segment's runtime spec (merged opts + color/autoColor). */
function segSpec(seg: StatusLineSegment): SegmentSpec {
  const def = SEGMENT_DEF_MAP.get(seg.type);
  const opts = { ...(def?.defaultOptions ?? {}), ...seg.options };
  const useAuto = !!seg.autoColor && VALUE_COLORABLE.has(seg.type);
  const rgbArr = useAuto ? null : hexToRgb(seg.color);
  return {
    type: seg.type,
    opts,
    rgb: rgbArr ? rgbArr.join(";") : null,
    autoColor: useAuto,
  };
}

/**
 * Generate the main statusLine Python script. Output (stdout) is byte-identical
 * to the former bash generator for every fixture in the golden regression set.
 */
export function generateStatusLineScript(segments: StatusLineSegment[]): string {
  const active = segments.filter(s => s.enabled);
  if (active.length === 0) {
    return PEP723_HEADER + "print('')\n";
  }
  const rows = groupRows(active).map(r => ({
    align: r.align,
    segs: r.segs.map(segSpec),
  }));
  const needGroup = active.some(s => GROUP_SEG_TYPES.has(s.type));
  const config = b64(JSON.stringify(rows));
  return [
    PEP723_HEADER,
    "# Generated by aidog — do not edit manually",
    ENGINE_PY,
    "",
    "import base64",
    `ROWS = json.loads(base64.b64decode("${config}").decode("utf-8"))`,
    `NEED_GROUP = ${needGroup ? "True" : "False"}`,
    "",
    "def main():",
    "    payload = json.loads(sys.stdin.read() or '{}')",
    "    gi = fetch_group_info() if NEED_GROUP else None",
    "    for line in render(payload, ROWS, gi):",
    "        sys.stdout.write(line + '\\n')",
    "",
    "main()",
    "",
  ].join("\n");
}

/**
 * Generate the SubagentStatusLine Python script — one JSONL `{"id","content"}`
 * line per task. Byte-identical to the former bash subagent generator.
 */
export function generateSubagentStatusLineScript(segments: StatusLineSegment[]): string {
  const active = segments.filter(s => s.enabled);
  if (active.length === 0) {
    return PEP723_HEADER + "pass\n";
  }
  // Subagent rows are single-line; all active segments render on one row.
  const config = b64(JSON.stringify(active.map(segSpec)));
  return [
    PEP723_HEADER,
    "# Generated by aidog — do not edit manually (SubagentStatusLine)",
    ENGINE_PY,
    "",
    "import base64",
    `SEGS = json.loads(base64.b64decode("${config}").decode("utf-8"))`,
    "",
    "def main():",
    "    payload = json.loads(sys.stdin.read() or '{}')",
    "    now = _now_epoch()",
    "    for line in render_subagent(payload, SEGS, now):",
    "        sys.stdout.write(line + '\\n')",
    "",
    "main()",
    "",
  ].join("\n");
}

/**
 * Resolved materialization for a statusLine / subagentStatusLine config block.
 * `scriptContent` is the bash script body to write (builtin mode) or `null`
 * (custom mode / disabled — nothing to generate). `customCommand` is the
 * user-supplied native command (custom mode only). `padding` is carried so the
 * caller can assemble the native field.
 */
export interface StatuslineMaterialization {
  enabled: boolean;
  mode: "builtin" | "custom";
  scriptContent: string | null;
  customCommand: string;
}

/**
 * Pure resolver: given a stored `_aidog_statusline` / `_aidog_subagent_statusline`
 * block and its scriptType, derive everything needed to materialize the native
 * `statusLine` / `subagentStatusLine` field — applying all default logic
 * (segments → DEFAULT_SEGMENTS, subagent template selection)
 * in one authoritative place. No side effects; the caller persists the result.
 *
 * Mirrors StatusLinePanel's in-component derivations so the on-save materializer
 * and the live UI agree byte-for-byte.
 */
export function materializeStatusline(
  stored: Record<string, any> | undefined,
  scriptType: "statusline" | "subagent",
): StatuslineMaterialization {
  const s = (stored ?? {}) as Record<string, any>;
  const isMain = scriptType === "statusline";
  const enabled = !!s.enabled;
  const mode: "builtin" | "custom" = s.mode === "custom" ? "custom" : "builtin";
    const customCommand = typeof s.customCommand === "string" ? s.customCommand : "";

  let scriptContent: string | null = null;
  if (enabled && mode === "builtin") {
    if (!isMain) {
      // Subagent statusline — native bash generator emitting per-task JSONL
      //   stdin:  {tasks: [{id, name, type, status, …}]}
      //   stdout: {"id":"…","content":"…"} per task
      // (no external dependency; the old python delegation was a non-distributable
      //  dev-machine path).
      const segments: StatusLineSegment[] =
        (s.segments as StatusLineSegment[] | undefined) ?? DEFAULT_SUBAGENT_SEGMENTS.map(seg => ({ ...seg }));
      return {
        enabled: true,
        mode: "builtin",
        scriptContent: generateSubagentStatusLineScript(segments),
        customCommand,
      };
    }
    // main statusline — segment-based bash generator.
    const segments: StatusLineSegment[] =
      (s.segments as StatusLineSegment[] | undefined) ?? DEFAULT_SEGMENTS.map(seg => ({ ...seg }));
    scriptContent = generateStatusLineScript(segments);
  }

  return { enabled, mode, scriptContent, customCommand };
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
