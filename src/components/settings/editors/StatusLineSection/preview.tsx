// ponytail: 自 StatusLineSection.tsx L36-96 外迁，零逻辑变更。

import {
  type StatusLineSegment,
  type SegmentType,
  VALUE_COLORABLE,
  hexToRgb,
  SEGMENT_DEF_MAP,
  PREVIEW_METRIC,
  groupRows,
} from "../../statusline-gen";

/** Map a mock metric to the same semantic color the bash thresholds produce. */
export function autoColorPreviewHex(type: SegmentType): string {
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
export function previewColor(seg: StatusLineSegment): string | null {
  if (seg.autoColor && VALUE_COLORABLE.has(seg.type)) return autoColorPreviewHex(seg.type);
  const rgb = hexToRgb(seg.color);
  return rgb ? `#${rgb.map(v => v.toString(16).padStart(2, "0")).join("")}` : null;
}

/** Render a colored, row-grouped, aligned live preview of the segments. */
export function StatusLinePreview({ segments, empty }: { segments: StatusLineSegment[]; empty: string }) {
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
