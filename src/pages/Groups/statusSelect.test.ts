import { describe, expect, it } from "vitest";
import { buildStatusOptions, errorCategory, idsForSelection, platformCategories } from "./statusSelect";
import type { CategorizablePlatform } from "./statusSelect";
import type { PlatformErrorStatus } from "../../services/api";

const NOW = 1_800_000_000_000;
const item = (platform_id: number, code: number): PlatformErrorStatus => ({ platform_id, code, source: "log", until_ms: null });
const plat = (id: number, extra: Partial<CategorizablePlatform> = {}): CategorizablePlatform => ({ id, status: "enabled", ...extra });
const cats = (p: CategorizablePlatform, code?: number) => [...platformCategories(p, code === undefined ? undefined : item(p.id, code), NOW)].sort();

describe("errorCategory", () => {
  it.each([
    [401, "key_invalid"], [403, "key_invalid"], [402, "balance"], [429, "rate_limit"], [529, "rate_limit"],
    [451, "censorship"], [500, "server"], [502, "server"], [503, "server"],
    [400, null], [404, null], [200, null], [0, null],
  ])("%i → %s", (code, cat) => {
    expect(errorCategory(code)).toBe(cat);
  });
});

describe("platformCategories", () => {
  it("401 with expired text → token_expired only (not key_invalid)", () => {
    expect(cats(plat(1, { last_error: "HTTP 401: 该令牌已过期" }), 401)).toEqual(["token_expired"]);
    expect(cats(plat(1, { last_error: "HTTP 401: token expired" }), 401)).toEqual(["token_expired"]);
    expect(cats(plat(1, { last_error: "HTTP 401: invalid key" }), 401)).toEqual(["key_invalid"]);
  });
  it("token_expired also recognised from last_error HTTP 401 without status item", () => {
    expect(cats(plat(1, { last_error: "HTTP 401: 该令牌已过期" }))).toEqual(["token_expired"]);
  });
  it("429 with quota words → quota, else rate_limit; 529 treated as 429", () => {
    expect(cats(plat(1, { last_error: "HTTP 429: quota exceeded" }), 429)).toEqual(["quota"]);
    expect(cats(plat(1, { last_error: "HTTP 429: 已达到 Token Plan 用量上限" }), 529)).toEqual(["quota"]);
    expect(cats(plat(1, { last_error: "HTTP 429: 额度不足" }), 429)).toEqual(["quota"]);
    expect(cats(plat(1, { last_error: "HTTP 429: slow down" }), 429)).toEqual(["rate_limit"]);
    expect(cats(plat(1), 529)).toEqual(["rate_limit"]);
  });
  it("last_error status code is the fallback for all code categories when no status item", () => {
    expect(cats(plat(1, { last_error: "HTTP 429: quota exceeded" }))).toEqual(["quota"]);
    expect(cats(plat(1, { last_error: "HTTP 429: too many requests" }))).toEqual(["rate_limit"]);
    expect(cats(plat(1, { last_error: "HTTP 529: overloaded" }))).toEqual(["rate_limit"]);
    expect(cats(plat(1, { last_error: "HTTP 402: payment required" }))).toEqual(["balance"]);
    expect(cats(plat(1, { last_error: "HTTP 403: forbidden" }))).toEqual(["key_invalid"]);
    expect(cats(plat(1, { last_error: "HTTP 503: unavailable" }))).toEqual(["server"]);
  });
  it("timeout / connect failure from last_error", () => {
    expect(cats(plat(1, { last_error: "upstream error: Connect: operation timed out" }))).toEqual(["timeout"]);
    expect(cats(plat(1, { last_error: "request Timeout" }))).toEqual(["timeout"]);
    expect(cats(plat(1, { last_error: "upstream error: connection closed before message completed" }))).toEqual(["connect"]);
    expect(cats(plat(1, { last_error: "upstream error: dns error" }))).toEqual(["connect"]);
    expect(cats(plat(1, { last_error: "HTTP 500: connect failed" }))).toEqual(["server"]);
  });
  it("platform expired when expires_at > 0 and <= now", () => {
    expect(cats(plat(1, { expires_at: NOW - 1 }))).toEqual(["expired"]);
    expect(cats(plat(1, { expires_at: NOW }))).toEqual(["expired"]);
    expect(cats(plat(1, { expires_at: NOW + 1 }))).toEqual([]);
    expect(cats(plat(1, { expires_at: 0 }))).toEqual([]);
  });
  it("censorship via marker or 451", () => {
    expect(cats(plat(1, { last_error: "HTTP 400 [censorship_blocked]" }))).toEqual(["censorship"]);
    expect(cats(plat(1), 451)).toEqual(["censorship"]);
  });
  it("independent signals coexist with code category", () => {
    expect(cats(plat(1, { last_error: "HTTP 504: gateway timed out", expires_at: NOW - 1 }), 504)).toEqual(["expired", "server", "timeout"]);
  });
});

describe("buildStatusOptions", () => {
  const items = [item(1, 402), item(2, 402), item(3, 502), item(9, 401)];
  it("counts per category within group, fixed order, ignores outside-group platforms", () => {
    const r = buildStatusOptions(items, [plat(1), plat(2), plat(3), plat(4)], NOW);
    expect(r.map((o) => [o.value, o.count])).toEqual([["c:balance", 2], ["c:server", 1]]);
  });

  it("merges 401 and 403 into key_invalid", () => {
    const r = buildStatusOptions([item(1, 401), item(2, 403)], [plat(1), plat(2)], NOW);
    expect(r.map((o) => [o.value, o.count])).toEqual([["c:key_invalid", 2]]);
  });

  it("lists new categories only when hit, in fixed order", () => {
    const r = buildStatusOptions([item(1, 401), item(2, 429)], [
      plat(1, { last_error: "HTTP 401: 已过期" }),
      plat(2, { last_error: "HTTP 429: quota exceeded" }),
      plat(3, { last_error: "upstream error: timed out" }),
      plat(4, { expires_at: NOW - 5 }),
    ], NOW);
    expect(r.map((o) => [o.value, o.count])).toEqual([
      ["c:token_expired", 1], ["c:quota", 1], ["c:timeout", 1], ["c:expired", 1],
    ]);
  });

  it("empty when no group platform has a categorized error", () => {
    expect(buildStatusOptions(items, [plat(4), plat(5)], NOW)).toEqual([]);
    expect(buildStatusOptions([item(1, 404)], [plat(1)], NOW)).toEqual([]);
  });
});

describe("idsForSelection", () => {
  const items = [item(1, 402), item(2, 402), item(3, 502), item(9, 401)];
  const plats: CategorizablePlatform[] = [
    plat(1), plat(2, { status: "disabled" }), plat(3, { status: "auto_disabled" }), plat(4, { status: "disabled" }),
  ];
  it("s:<status> selects by platform status", () => {
    expect(idsForSelection("s:disabled", items, plats, NOW)).toEqual(new Set([2, 4]));
    expect(idsForSelection("s:auto_disabled", items, plats, NOW)).toEqual(new Set([3]));
    expect(idsForSelection("s:enabled", items, plats, NOW)).toEqual(new Set([1]));
  });
  it("c:<category> selects in-group platforms by error category", () => {
    expect(idsForSelection("c:balance", items, plats, NOW)).toEqual(new Set([1, 2]));
    expect(idsForSelection("c:server", items, plats, NOW)).toEqual(new Set([3]));
    expect(idsForSelection("c:key_invalid", items, plats, NOW)).toEqual(new Set()); // 401 属组外平台 9
    expect(idsForSelection("c:rate_limit", items, plats, NOW)).toEqual(new Set());
  });
  it("c:<new category> selects by last_error / expires_at", () => {
    const ps = [plat(1, { last_error: "upstream error: timed out" }), plat(2, { expires_at: NOW - 1 }), plat(3)];
    expect(idsForSelection("c:timeout", [], ps, NOW)).toEqual(new Set([1]));
    expect(idsForSelection("c:expired", [], ps, NOW)).toEqual(new Set([2]));
  });
});
