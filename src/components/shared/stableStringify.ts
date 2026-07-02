// Order-insensitive JSON serialization: object keys are sorted recursively so
// reordered keys produce an identical string. Used as the dirty-state signature
// for config forms (Settings / CodexSettings) so key reorders don't register
// as a change.
export function stableStringify(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
  const obj = value as Record<string, unknown>;
  const keys = Object.keys(obj).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${stableStringify(obj[k])}`).join(",")}}`;
}
