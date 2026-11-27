import { describe, expect, it, vi } from "vitest";
import type { Platform } from "../../services/api";
import { computeQuotaDisplay } from "./health";

describe("calendar quota display", () => {
  it("uses the backend's actual end of the monthly window", () => {
    vi.useFakeTimers();
    try {
      vi.setSystemTime(new Date("2026-02-28T00:00:00+08:00"));
      const end = Date.parse("2026-03-31T00:00:00+08:00");
      const platform = {
        est_balance_remaining: 0,
        est_coding_plan: JSON.stringify({ tiers: [{ name: "monthly", est_utilization: 20, window_start: Date.now(), window_end: end, limit: 100 }] }),
      } as Platform;
      const result = computeQuotaDisplay(platform, undefined, false);
      expect(result.tiers[0].resetsAt).toBe(new Date(end).toISOString());
    } finally {
      vi.useRealTimers();
    }
  });
});
