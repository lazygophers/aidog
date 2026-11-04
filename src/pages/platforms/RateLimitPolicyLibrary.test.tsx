// 策略库区：导入覆盖（含来源字段）+「内置策略已更新」提示。选择器锚稳定 id + data-state。
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent, waitFor } from "../../test/render";
import { RateLimitPolicyLibrary } from "./RateLimitPolicyLibrary";
import type { RateLimitPolicy } from "../../domains/platforms/defaults";

const POLICIES: RateLimitPolicy[] = [
  { id: "v0", label: "V0", source_url: "https://example.com/v0", concurrency: 50, models: { "glm-4-plus": { concurrency: 5 } } },
  { id: "skel", label: "Skeleton", source_url: "https://example.com/s" },
];

vi.mock("../../domains/platforms", async (importOriginal) => {
  const orig = await importOriginal<typeof import("../../domains/platforms")>();
  return {
    ...orig,
    getDefaultRateLimitPolicies: vi.fn(async () => ({ policies: structuredClone(POLICIES), lastUpdated: 200 })),
  };
});

const t = ((key: string, fallback?: string) => fallback ?? key) as never;
const byId = (id: string) => document.getElementById(id);

async function renderLib(source?: { id?: string; at?: number }) {
  const onImport = vi.fn();
  const r = render(<RateLimitPolicyLibrary protocol="glm" source={source} onImport={onImport} t={t} />);
  await waitFor(() => expect(byId("rl-policy-v0")).not.toBeNull());
  return { ...r, onImport };
}

describe("RateLimitPolicyLibrary", () => {
  it("导入：确认后覆盖写入，带 source_policy_id / source_last_updated 与模型级并发", async () => {
    const { onImport } = await renderLib();
    fireEvent.click(byId("rl-policy-import-v0")!);
    expect(onImport).not.toHaveBeenCalled(); // 未确认不写
    await waitFor(() => expect(byId("rl-import-confirm")).not.toBeNull());
    fireEvent.click(byId("rl-import-confirm-ok")!);
    expect(onImport).toHaveBeenCalledWith({
      concurrency: 50,
      models: { "glm-4-plus": { concurrency: 5 } },
      source_policy_id: "v0",
      source_last_updated: 200,
    });
  });

  it("取消确认 → 不导入", async () => {
    const { onImport } = await renderLib();
    fireEvent.click(byId("rl-policy-import-skel")!);
    await waitFor(() => expect(byId("rl-import-confirm-cancel")).not.toBeNull());
    fireEvent.click(byId("rl-import-confirm-cancel")!);
    expect(onImport).not.toHaveBeenCalled();
  });

  it("source_last_updated 小于内置 last_updated → stale 提示", async () => {
    await renderLib({ id: "v0", at: 100 });
    expect(byId("rl-policy-library")!.getAttribute("data-state")).toBe("stale");
    expect(byId("rl-policy-updated")).not.toBeNull();
  });

  it("source_last_updated 不旧 / 未导入 → 无提示", async () => {
    const a = await renderLib({ id: "v0", at: 200 });
    expect(byId("rl-policy-library")!.getAttribute("data-state")).toBe("current");
    expect(byId("rl-policy-updated")).toBeNull();
    a.unmount();
    await renderLib();
    expect(byId("rl-policy-library")!.getAttribute("data-state")).toBe("none");
    expect(byId("rl-policy-updated")).toBeNull();
  });
});
