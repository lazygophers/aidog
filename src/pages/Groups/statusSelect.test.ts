import { describe, expect, it } from "vitest";
import { buildStatusOptions, idsForCode, idsForSelection } from "./statusSelect";
import type { PlatformErrorStatus } from "../../services/api";

const items: PlatformErrorStatus[] = [
  { platform_id: 1, code: 402, source: "cooldown", until_ms: 123 },
  { platform_id: 2, code: 402, source: "log", until_ms: null },
  { platform_id: 3, code: 529, source: "log", until_ms: null },
  { platform_id: 9, code: 401, source: "log", until_ms: null }, // 组外 → 不计
];

describe("buildStatusOptions", () => {
  it("counts per code within group, sorted asc, ignores outside-group platforms", () => {
    expect(buildStatusOptions(items, [1, 2, 3, 4])).toEqual([
      { code: 402, count: 2 },
      { code: 529, count: 1 },
    ]);
  });

  it("empty when no group platform has error status", () => {
    expect(buildStatusOptions(items, [4, 5])).toEqual([]);
  });
});

describe("idsForCode", () => {
  it("selects only in-group platforms with the code", () => {
    expect(idsForCode(items, [1, 2, 3, 4], 402)).toEqual(new Set([1, 2]));
    expect(idsForCode(items, [1, 2, 3, 4], 529)).toEqual(new Set([3]));
    expect(idsForCode(items, [1, 2, 3, 4], 401)).toEqual(new Set());
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
  it("numeric value selects by error code", () => {
    expect(idsForSelection("402", items, plats)).toEqual(new Set([1, 2]));
  });
});
