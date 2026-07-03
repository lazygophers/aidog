import type { useTranslation } from "react-i18next";

export type TFunc = ReturnType<typeof useTranslation>["t"];

/** 时间范围预设 */
export type TimePreset = "all" | "1h" | "6h" | "24h" | "7d" | "30d";

export function timePresetToRange(preset: TimePreset): { start?: number; end?: number } {
  if (preset === "all") return {};
  const now = Date.now();
  const ms: Record<string, number> = { "1h": 3600000, "6h": 21600000, "24h": 86400000, "7d": 604800000, "30d": 2592000000 };
  return { start: now - (ms[preset] ?? 0), end: now };
}

export function safeParseJson(str: string): any {
  try { return JSON.parse(str); } catch { return str; }
}

export const NO_GROUP_SENTINEL = "__none__";
