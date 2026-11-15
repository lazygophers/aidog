import { describe, expect, it } from "vitest";
import { buildStatusOptions, errorCategory, idsForSelection } from "./statusSelect";
import type { PlatformErrorStatus } from "../../services/api";

const items: PlatformErrorStatus[] = [
  { platform_id: 1, code: 402, source: "cooldown", until_ms: 123 },
  { platform_id: 2, code: 402, source: "log", until_ms: null },
  { platform_id: 3, code: 529, source: "log", until_ms: null },
  { platform_id: 9, code: 401, source: "log", until_ms: null }, // 组外 → 不计
];

describe("errorCategory", () => {
  it.each([
    [401, "key_invalid"], [403, "key_invalid"], [402, "balance"], [429, "rate_limit"],
    [500, "server"], [502, "server"], [503, "server"], [529, "server"],
    [400, null], [404, null], [200, null], [0, null],
  ])("%i → %s", (code, cat) => {
    expect(errorCategory(code)).toBe(cat);
  });
});

describe("buildStatusOptions", () => {
  it("counts per category within group, fixed order, ignores outside-group platforms", () => {
    const r = buildStatusOptions(items, [1, 2, 3, 4]);
    expect(r.map((o) => [o.value, o.count])).toEqual([["c:balance", 2], ["c:server", 1]]);
  });

  it("merges 401 and 403 into key_invalid", () => {
    const r = buildStatusOptions([
      { platform_id: 1, code: 401, source: "log", until_ms: null },
      { platform_id: 2, code: 403, source: "log", until_ms: null },
    ], [1, 2]);
    expect(r.map((o) => [o.value, o.count])).toEqual([["c:key_invalid", 2]]);
  });

  it("empty when no group platform has a categorized error", () => {
    expect(buildStatusOptions(items, [4, 5])).toEqual([]);
    expect(buildStatusOptions([{ platform_id: 1, code: 404, source: "log", until_ms: null }], [1])).toEqual([]);
  });
});

describe("idsForSelection", () => {
  const plats = [
    { id: 1, status: "enabled" as const },
    { id: 2, status: "disabled" as const },
    { id: 3, status: "auto_disabled" as const },
    { id: 4, status: "disabled" as const },
  ];
  it("s:<status> selects by platform status", () => {
    expect(idsForSelection("s:disabled", items, plats)).toEqual(new Set([2, 4]));
    expect(idsForSelection("s:auto_disabled", items, plats)).toEqual(new Set([3]));
    expect(idsForSelection("s:enabled", items, plats)).toEqual(new Set([1]));
  });
  it("c:<category> selects in-group platforms by error category", () => {
    expect(idsForSelection("c:balance", items, plats)).toEqual(new Set([1, 2]));
    expect(idsForSelection("c:server", items, plats)).toEqual(new Set([3]));
    expect(idsForSelection("c:key_invalid", items, plats)).toEqual(new Set()); // 401 属组外平台 9
    expect(idsForSelection("c:rate_limit", items, plats)).toEqual(new Set());
  });
});
