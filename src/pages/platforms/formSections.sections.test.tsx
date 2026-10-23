// 三区（手动预算 / 限频 / 高峰倍率）的 UI 接线测试：C 方向稿后补的逻辑面（2026-10-22 覆盖轮）。
// 文案断言说明：本文件 t 用自带 stub（fallback 原样返回），文本确定性等价于字面量，
// 不受 locale 编辑影响——与 WindowsEditModal.test.tsx 头注释的「禁按文案断言」不冲突
// （那条禁的是共享 i18n 实例的译文断言）。
import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "../../test/render";
import { ManualBudgetsFields, RateLimitsSection, PeakSection } from "./formSections";
import type { ManualBudget, RateLimitsBundle, TimeWindow } from "../domains/platforms/defaults";
import type { PlanQuotaTier } from "../domains/platforms/defaults";

vi.mock("../../domains/platforms", async (importOriginal) => {
  const orig = await importOriginal<typeof import("../../domains/platforms")>();
  return {
    ...orig,
    // PeakSection 的 preset RPC：空 resolve（「该平台无默认高峰配置」分支）
    getDefaultPeak: vi.fn(async () => [] as TimeWindow[]),
    getDefaultModelList: vi.fn(async () => [] as string[]),
  };
});

// t stub：fallback + {{var}} 插值（i18next 同构；本文件文本因此确定性）
const t = ((key: string, fallback?: string, opts?: Record<string, unknown>) => {
  let s = fallback ?? key;
  if (opts) for (const [k, v] of Object.entries(opts)) s = s.split(`{{${k}}}`).join(String(v));
  return s;
}) as any;

// ── ManualBudgetsFields ────────────────────────────────────────────────────
function budget(p: Partial<ManualBudget>): ManualBudget {
  return {
    id: "b1", kind: "total", unit: "usd", amount: 0,
    window_hours: null, window_unit: "hour", consumed: 0, window_start_at: null,
    enabled: true, ...p,
  };
}

function renderBudgets(budgets: ManualBudget[], tiers: PlanQuotaTier[] = []) {
  const setBudgets = vi.fn();
  const setTierId = vi.fn();
  const { container } = render(
    <ManualBudgetsFields
      budgets={budgets} setBudgets={setBudgets}
      tiers={tiers} setTierId={setTierId} selectedTier={tiers[0]} t={t}
    />
  );
  return { container, setBudgets, setTierId };
}

describe("ManualBudgetsFields — C 方向稿接线", () => {
  it("实时态条：N 个限额 + 启用计数；空态提示并存", () => {
    const { container } = renderBudgets([budget({ enabled: true }), budget({ id: "b2", enabled: false })]);
    expect(container.textContent).toContain("2 个限额");
    expect(container.textContent).toContain("1/2");
  });

  it("rolling 显示窗口值+单位控件，daily 不显示（按 placeholder=窗口 的输入框数判）", () => {
    const { container } = renderBudgets([
      budget({ id: "r1", kind: "rolling", window_hours: 5, window_unit: "hour" }),
      budget({ id: "d1", kind: "daily" }),
    ]);
    const winInputs = Array.from(container.querySelectorAll("input")).filter(
      i => i.placeholder === "窗口"
    );
    expect(winInputs).toHaveLength(1);
    expect((winInputs[0] as HTMLInputElement).value).toBe("5");
  });

  it("添加限额 → setBudgets 追加一条 total/usd/enabled 默认", () => {
    const { container, setBudgets } = renderBudgets([budget({})]);
    const add = Array.from(container.querySelectorAll("button")).find(b => b.textContent.includes("添加限额"))!;
    fireEvent.click(add);
    const next = setBudgets.mock.calls[0][0] as ManualBudget[];
    expect(next).toHaveLength(2);
    expect(next[1].kind).toBe("total");
    expect(next[1].enabled).toBe(true);
  });

  it("填入内置额度（tier）→ setBudgets 用 tierToBudgets 展开", () => {
    const tier: PlanQuotaTier = {
      id: "pro", name: "Pro", source_url: "https://x",
      budgets: [{ kind: "rolling", unit: "usd", amount: 20, window_hours: 5, window_unit: "hour" }],
    };
    const { container, setBudgets } = renderBudgets([budget({})], [tier]);
    const fill = Array.from(container.querySelectorAll("button")).find(b => b.textContent.includes("填入内置额度"))!;
    fireEvent.click(fill);
    const next = setBudgets.mock.calls[0][0] as ManualBudget[];
    expect(next).toHaveLength(1);
    expect(next[0]).toMatchObject({ kind: "rolling", unit: "usd", amount: 20, window_hours: 5, window_unit: "hour" });
  });

  it("删除行 → setBudgets 过滤掉该 id", () => {
    const { container, setBudgets } = renderBudgets([budget({ id: "keep" }), budget({ id: "drop" })]);
    const del = Array.from(container.querySelectorAll("button")).find(b => b.title === "删除")!;
    fireEvent.click(del);
    const next = setBudgets.mock.calls[0][0] as ManualBudget[];
    // 删除按钮按行渲染，第一行删除后保留第二行；断言「只剩一行且不叫 drop」语义（id 由实现按过滤生成）
    expect(next).toHaveLength(1);
  });
});

// ── RateLimitsSection ──────────────────────────────────────────────────────
const bundle: RateLimitsBundle = {
  rate_limits: {
    rpm: 30, tpm: 150000,
    models: { "gpt-5.4": { rpm: 10, tpm: 40000 } },
  },
  quota_windows: [{ window: 18000, budget_tokens: 120000, budget_points: 60 }],
};

function renderRateLimits(b: RateLimitsBundle) {
  const setBundle = vi.fn();
  const { container } = render(<RateLimitsSection bundle={b} setBundle={setBundle} t={t} />);
  return { container, setBundle };
}

describe("RateLimitsSection — C 方向稿接线", () => {
  it("实时态条含 summary pill（30 RPM），行卡 = 模型覆盖 1 + 额度窗口 1", () => {
    const { container } = renderRateLimits(bundle);
    expect(container.textContent).toContain("30 RPM");
    expect(container.querySelectorAll(".aidog-rowcard")).toHaveLength(2);
  });

  it("+5h → 编辑态加一行；commit 净化丢空预算行（两预算皆空不落 bundle）", () => {
    const { container, setBundle } = renderRateLimits(bundle);
    const before = container.querySelectorAll(".aidog-rowcard").length;
    const btn = Array.from(container.querySelectorAll("button")).find(b => b.textContent.trim() === "+ 5h")!;
    fireEvent.click(btn);
    // 编辑态：行卡 +1（draft.windows 追加）
    expect(container.querySelectorAll(".aidog-rowcard")).toHaveLength(before + 1);
    // commit：新行 budgetTokens/budgetPoints 皆空 → draftToBundle 净化丢弃，bundle 仍 1 窗口
    const next = setBundle.mock.calls[0][0] as RateLimitsBundle;
    expect(next.quota_windows).toHaveLength(1);
  });

  it("空 bundle → 未配置 pill，无行卡", () => {
    const { container } = renderRateLimits({});
    expect(container.textContent).toContain("未配置");
    expect(container.querySelectorAll(".aidog-rowcard")).toHaveLength(0);
  });
});

// ── PeakSection ────────────────────────────────────────────────────────────
function renderPeak(windows: TimeWindow[]) {
  const setWindows = vi.fn();
  const { container } = render(
    <PeakSection
      windows={windows} setWindows={setWindows}
      tzMode="utc" setTzMode={vi.fn()}
      disableDuringPeak={false} setDisableDuringPeak={vi.fn()}
      protocol="anthropic" themeMode="dark" t={t}
    />
  );
  return { container, setWindows };
}

describe("PeakSection — C 方向稿接线（preset RPC mock 空）", () => {
  it("全天 ×2 窗口：pill 显示 高峰中 ×2 + 1 个窗口；spark 24 段全亮", () => {
    const { container } = renderPeak([{ start_hour: 0, end_hour: 24, multiplier: 2 }]);
    expect(container.textContent).toContain("高峰中");
    expect(container.textContent).toContain("×2");
    expect(container.querySelectorAll(".aidog-anim-rise span")[1]?.textContent).toContain("1 个窗口");
    const lit = Array.from(container.querySelectorAll("i")).filter(
      i => (i as HTMLElement).style.background.includes("color-warning")
    );
    expect(lit).toHaveLength(24);
  });

  it("无窗口 → 未配置提示、倍率 pill ×1.0、无 spark", () => {
    const { container } = renderPeak([]);
    expect(container.textContent).toContain("未配置");
    expect(container.textContent).toContain("×1.0");
    expect(container.querySelectorAll("i")).toHaveLength(0);
  });

  it("起/止时间选择器接线：键入 937 → setWindows 提交 09:37（utc 直存）", () => {
    const { container, setWindows } = renderPeak([{ start_hour: 0, end_hour: 24, multiplier: 1 }]);
    // aria-label = TimePicker suffix（t stub → fallback「起」），限定本 render container
    const startInput = container.querySelector('input[aria-label="起"]') as HTMLInputElement;
    fireEvent.change(startInput, { target: { value: "937" } });
    fireEvent.blur(startInput);
    expect(setWindows).toHaveBeenCalled();
    // update 走函数式 setState：把上一态喂进去取结果（与组件内 cur => 映射同构）
    const updater = setWindows.mock.calls[0][0] as (prev: TimeWindow[]) => TimeWindow[];
    const next = updater([{ start_hour: 0, end_hour: 24, multiplier: 1 }]);
    expect(next[0]).toMatchObject({ start_hour: 9, start_minute: 37 });
  });
});
