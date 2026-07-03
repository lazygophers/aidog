// Deep-merge two plain-object configs.
//
// Semantics:
// - override wins: keys present in `override` overwrite `base`.
// - nested plain objects are merged recursively.
// - arrays and scalars in `override` replace the corresponding `base` value (no union).
// - keys only present in `base` are preserved untouched.
//
// Used by "load recommended config" on both the Claude (Settings) and Codex
// (CodexSettings) pages: current draft = base, recommended = override.

// ponytail: D7 刻意不合 — 此处是严版（input unknown + 拒 Map/Set/Date/类实例，
// 用 toString 锁 [object Object]），deepMerge 递归合并配置树时必须排掉宿主对象避免误并。
// 另有一份松版在 components/settings/editors.tsx（any 入参，仅 typeof+!Array），
// 语义不同，禁合并。见 arch-redesign D7 决策。
function isPlainObject(v: unknown): v is Record<string, unknown> {
  return (
    typeof v === "object" &&
    v !== null &&
    !Array.isArray(v) &&
    Object.prototype.toString.call(v) === "[object Object]"
  );
}

export function deepMerge<T extends Record<string, unknown>>(
  base: T,
  override: Record<string, unknown>,
): T {
  const result: Record<string, unknown> = { ...base };
  for (const key of Object.keys(override)) {
    const overrideVal = override[key];
    const baseVal = result[key];
    if (isPlainObject(baseVal) && isPlainObject(overrideVal)) {
      result[key] = deepMerge(baseVal, overrideVal);
    } else {
      result[key] = overrideVal;
    }
  }
  return result as T;
}
