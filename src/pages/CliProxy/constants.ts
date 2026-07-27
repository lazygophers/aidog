import type { CSSProperties } from "react";

// radix Select 空值哨兵：`<SelectItem value="">` 会抛，用 __none__ 映射回 ""/null。
export const NONE = "__none__";

export const fieldLabel: CSSProperties = {
  display: "flex", flexDirection: "column", gap: 4,
  fontSize: 12, color: "var(--text-secondary)",
};
