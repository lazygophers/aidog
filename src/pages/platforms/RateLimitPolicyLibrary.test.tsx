// 策略库区：导入覆盖（含来源字段）+「内置策略已更新」提示。选择器锚稳定 id + data-state。
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent, waitFor } from "../../test/render";
import { RateLimitPolicyLibrary } from "./RateLimitPolicyLibrary";
import type { RateLimitPolicy } from "../../domains/platforms/defaults";

const POLICIES: RateLimitPolicy[] = [
  { id: "v0", label: "V0", source_url: "https://example.com/v0", concurrency: 50, rules: [{ models: ["glm-4-plus"], concurrency: 5 }] },
  { id: "mw", label: "MW", source_url: "https://example.com/mw", rules: [{ models: ["glm-4-plus"], start_hour: 14, end_hour: 18, rpm: 5 }] },
  { id: "skel", label: "Skeleton", source_url: "https://example.com/s" },
  {
    id: "many", label: "Many", source_url: "https://example.com/m", concurrency: 1000, rpm: 60,
    rules: [
      ...Array.from({ length: 8 }, (_, i) => ({ models: [`m${i}`], concurrency: i + 1 })),
      { start_hour: 9, end_hour: 18, days_of_week: [1, 2, 3], timezone: "Asia/Shanghai", rpm: 30 },
    ],
  },
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
  it("导入：确认后覆盖写入，规则原样拷贝（不转换），带 source_policy_id / source_last_updated", async () => {
    const { onImport } = await renderLib();
    fireEvent.click(byId("rl-policy-import-v0")!);
    expect(onImport).not.toHaveBeenCalled(); // 未确认不写
    await waitFor(() => expect(byId("rl-import-confirm")).not.toBeNull());
    fireEvent.click(byId("rl-import-confirm-ok")!);
    expect(onImport).toHaveBeenCalledWith({
      concurrency: 50,
      rules: [{ models: ["glm-4-plus"], concurrency: 5 }],
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

  it("只选模型的规则：一条一行并带数字；平台级有数字时不显示骨架提示", async () => {
    await renderLib();
    const row = byId("rl-policy-rule-v0-0")!;
    expect(row.textContent).toContain("glm-4-plus");
    expect(row.textContent).toContain("5");
    expect(row.getAttribute("data-kind")).toBe("model");
    expect(byId("rl-policy-empty-v0")).toBeNull();
  });

  it("骨架策略（无任何数字）→ 显示 empty 提示", async () => {
    await renderLib();
    expect(byId("rl-policy-empty-skel")).not.toBeNull();
    expect(byId("rl-policy-empty-v0")).toBeNull();
  });

  it("只选时段的规则渲染小时 / 星期 / 时区 / 数字", async () => {
    await renderLib();
    fireEvent.click(byId("rl-policy-toggle-many")!);
    const w = byId("rl-policy-rule-many-8")!;
    expect(w.getAttribute("data-kind")).toBe("window");
    expect(w.textContent).toContain("9");
    expect(w.textContent).toContain("18");
    expect(w.textContent).toContain("Asia/Shanghai");
    expect(w.textContent).toContain("30");
  });

  it("规则多于 5 行默认折叠，可展开", async () => {
    await renderLib();
    expect(byId("rl-policy-rule-many-0")).not.toBeNull();
    expect(byId("rl-policy-rule-many-7")).toBeNull();
    fireEvent.click(byId("rl-policy-toggle-many")!);
    expect(byId("rl-policy-rule-many-7")).not.toBeNull();
  });

  it("数字带标签且并发显示完整整数（无 CONC / 1.0K 缩写）", async () => {
    await renderLib();
    const plat = byId("rl-policy-platform-many")!.textContent!;
    expect(plat).toContain("并发 1,000");
    expect(plat).toContain("RPM 60");
    expect(plat).not.toMatch(/CONC|1\.0K/);
    expect(byId("rl-policy-platform-v0")!.textContent).toBe("并发 50");
    expect(byId("rl-policy-rule-many-0")!.textContent).toContain("并发 1");
    fireEvent.click(byId("rl-policy-toggle-many")!);
    expect(byId("rl-policy-rule-many-8")!.textContent).toContain("RPM 30");
  });

  it("模型 + 时段规则：同一行显示模型名与时段，并整条拷贝导入", async () => {
    const { onImport } = await renderLib();
    const row = byId("rl-policy-rule-mw-0")!;
    expect(row.getAttribute("data-kind")).toBe("model-window");
    expect(row.textContent).toContain("glm-4-plus");
    expect(row.textContent).toContain("14:00–18:00");
    fireEvent.click(byId("rl-policy-import-mw")!);
    await waitFor(() => expect(byId("rl-import-confirm-ok")).not.toBeNull());
    fireEvent.click(byId("rl-import-confirm-ok")!);
    expect(onImport).toHaveBeenCalledWith({
      rules: [{ models: ["glm-4-plus"], start_hour: 14, end_hour: 18, rpm: 5 }],
      source_policy_id: "mw",
      source_last_updated: 200,
    });
  });
});
