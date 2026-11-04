// 路由画布 03 就近测试：按设计稿 v2 五列重画。只锚 data-node-id / data-edge / data-kind / data-state / data-*，
// 禁文案断言（测试 t 无 fallback 时回传 key，文本断言只比 key，不比 locale 值）。
import { describe, it, expect, afterEach } from "vitest";
import { render } from "../../test/render";
import { RoutingCanvas } from "./RoutingCanvas";
import type { RouteCandidate } from "../../services/api/types/generated/RouteCandidate";
import type { RoutingSnapshot } from "../../services/api/types/generated/RoutingSnapshot";
import type { ProxyAttempt } from "../../services/api/types/generated/ProxyAttempt";

type Props = Parameters<typeof RoutingCanvas>[0];
const t = ((key: string) => key) as unknown as Props["t"];

function cand(o: Partial<RouteCandidate> & { platform_id: number }): RouteCandidate {
  return { order: 1, platform: "p", level_priority: 1, priority: 1, reason: "selected", tags: [], model_via: [], model_chain: [], ...o };
}
function att(o: Partial<ProxyAttempt> & { platform_id: number }): ProxyAttempt {
  return { platform_name: "p", status_code: 200, error: "", duration_ms: 100, ts: 0, ...o };
}

function mount(p: Partial<Props> & { routing: RoutingSnapshot | null }) {
  const names = new Map<number, string>([[1, "One"], [2, "Two"], [3, "Three"], [11, "X11"], [12, "X12"], [13, "X13"]]);
  return render(
    <RoutingCanvas attempts={undefined} platformMap={names} durationMs={1000} statusCode={200} t={t} {...p} />,
  ).container as HTMLElement;
}
const nodeIds = (c: HTMLElement) => [...c.querySelectorAll("[data-node-id]")].map((e) => e.getAttribute("data-node-id"));
const edgeSet = (c: HTMLElement) =>
  [...c.querySelectorAll("path[data-edge]")].map((e) => `${e.getAttribute("data-edge")}:${e.getAttribute("data-kind")}`).sort();
const state = (c: HTMLElement, id: string) => c.querySelector(`[data-node-id="${id}"]`)?.getAttribute("data-state");
const edgeLabel = (c: HTMLElement, edge: string) => c.querySelector(`path[data-edge="${edge}"]`)?.getAttribute("data-label");

// ── 场景 A：换模型后换平台成功 ──────────────────────────────
const snapA: RoutingSnapshot = {
  mode: "health_aware", eval_ms: 30, max_retries: 3,
  entries: [
    cand({ order: 1, platform_id: 1, level_priority: 1, priority: 1, tags: ["coding-plan"], target_model: "m1", model_chain: ["m1", "m2"], model_via: ["slot-opus"] }),
    cand({ order: 2, platform_id: 2, priority: 2, target_model: "mm" }),
    cand({ order: 3, platform_id: 3, level_priority: 2, tags: ["demoted"], target_model: "sf" }),
    cand({ order: 4, platform_id: 11, reason: "peak-disabled" }),
    cand({ order: 5, platform_id: 12, reason: "rate-limited" }),
    cand({ order: 6, platform_id: 13, reason: "client-not-allowed" }),
  ],
};
const attsA = [
  att({ platform_id: 1, status_code: 502, error: "bad gateway", model: "m1", error_class: "model" }),
  att({ platform_id: 1, status_code: 200, error: "empty", model: "m2", kind: "empty_200", error_class: "service", chain_stop: "service", effects: ["penalty_server"] }),
  att({ platform_id: 2, status_code: 200, model: "mm", duration_ms: 4400, ttfb_ms: 300 }),
];

// ── 场景 B：全部失败 503 ──────────────────────────────
const snapB: RoutingSnapshot = {
  mode: "failover", eval_ms: 12, max_retries: 3,
  entries: [
    cand({ order: 1, platform_id: 1, target_model: "g46", model_chain: ["g46", "g45"] }),
    cand({ order: 2, platform_id: 2, tags: ["breaker-backfill"], target_model: "g46" }),
    cand({ order: 3, platform_id: 11, reason: "not-declared" }),
  ],
};
const attsB = [
  att({ platform_id: 1, status_code: 0, error: "connect", model: "g46", kind: "retry", error_class: "service" }),
  att({ platform_id: 1, status_code: 0, error: "connect", model: "g46", error_class: "service", chain_stop: "service", effects: ["breaker_count", "penalty_server"] }),
  att({ platform_id: 2, status_code: 0, error: "concurrency", model: "g46", kind: "rate_limit_platform" }),
];

// ── 场景 C：单平台短路 ──────────────────────────────
const snapC: RoutingSnapshot = {
  mode: "failover", eval_ms: 2, max_retries: 3,
  shortcut: { literal: true, bypassed: ["breaker"] },
  entries: [cand({ order: 1, platform_id: 1, target_model: "gpt-5", model_via: ["slot-default"] })],
};

describe("RoutingCanvas 场景 A：换模型后换平台成功", () => {
  const go = () => mount({ routing: snapA, attempts: attsA, statusCode: 200, durationMs: 263202, actualModel: "mm" });

  it("节点顺序 = 入站 → 路由 → 逐候选行（候选后跟其尝试）→ 响应", () => {
    expect(nodeIds(go())).toEqual([
      "req", "route",
      "c-1", "s-1-0", "s-1-1",
      "c-2", "s-2-0",
      "c-3", "c-11", "c-12", "c-13",
      "resp",
    ]);
  });

  it("连线集合：每个尝试过的节点都连上；换平台线 bad；成功连响应；排除/未轮到为 out", () => {
    expect(edgeSet(go())).toEqual([
      "c-1->s-1-0:eval",
      "c-2->s-2-0:eval",
      "req->route:eval",
      "route->c-1:eval",
      "route->c-11:out",
      "route->c-12:out",
      "route->c-13:out",
      "route->c-3:out",
      "s-1-0->s-1-1:bad",
      "s-1-1->c-2:bad",
      "s-2-0->resp:ok",
    ].sort());
  });

  it("状态：失败候选 fail、胜出 win、未轮到 / 排除 dim；空 200 按失败画", () => {
    const c = go();
    expect(state(c, "c-1")).toBe("fail");
    expect(state(c, "c-2")).toBe("win");
    expect(state(c, "c-3")).toBe("dim");
    expect(state(c, "c-11")).toBe("dim");
    expect(state(c, "s-1-0")).toBe("bad");
    expect(state(c, "s-1-1")).toBe("bad");
    expect(c.querySelector('[data-node-id="s-1-1"]')!.getAttribute("data-kind")).toBe("empty_200");
    expect(state(c, "s-2-0")).toBe("ok");
    expect(state(c, "resp")).toBe("win");
  });

  it("原因：排除项「未选：原因」；入选未轮到写已有成功；失败写类别与后果；换平台线带原因", () => {
    const c = go();
    const why = (id: string) => c.querySelector(`[data-node-id="${id}"] [data-testid="routing-not-chosen"]`)?.getAttribute("data-reason");
    expect(why("c-11")).toBe("peak-disabled");
    expect(why("c-12")).toBe("rate-limited");
    expect(why("c-13")).toBe("client-not-allowed");
    expect(why("c-3")).toBe("already-succeeded");
    expect(why("c-1")).toBeUndefined();
    expect(c.querySelector('[data-node-id="c-11"]')!.textContent).toContain("logs.routing.reason.peak-disabled");
    const s0 = c.querySelector('[data-node-id="s-1-0"]')!;
    expect(s0.querySelector('[data-testid="routing-step-class"]')!.getAttribute("data-class")).toBe("model");
    expect(s0.querySelector('[data-testid="routing-step-status"]')!.textContent).toContain("502");
    const s1 = c.querySelector('[data-node-id="s-1-1"]')!;
    expect(s1.querySelector('[data-testid="routing-step-stop"]')!.getAttribute("data-stop")).toBe("service");
    expect(s1.querySelector('[data-testid="routing-step-effect"]')!.getAttribute("data-effect")).toBe("penalty_server");
    expect(edgeLabel(c, "s-1-0->s-1-1")).toBe("next-model");
    expect(edgeLabel(c, "s-1-1->c-2")).toBe("switch-service");
  });

  it("候选标签与排序号：coding plan / 惩罚降级", () => {
    const c = go();
    const tags = (id: string) => [...c.querySelectorAll(`[data-node-id="${id}"] [data-testid="routing-tag"]`)].map((e) => e.getAttribute("data-tag"));
    expect(tags("c-1")).toEqual(["coding-plan"]);
    expect(tags("c-3")).toEqual(["demoted"]);
    expect(c.querySelector('[data-node-id="c-1"] [data-testid="routing-order"]')!.getAttribute("data-order")).toBe("1");
  });

  it("响应节点：状态码、总耗时、首字、tokens、费用、重试次数", () => {
    const c = mount({ routing: snapA, attempts: attsA, statusCode: 200, durationMs: 263202, inputTokens: 18200, outputTokens: 3100, estCost: 0.042, retryCount: 1 });
    const r = c.querySelector('[data-node-id="resp"]')!;
    expect(r.querySelector('[data-testid="routing-resp-status"]')!.getAttribute("data-value")).toBe("200");
    expect(r.querySelector('[data-testid="routing-resp-duration"]')!.getAttribute("data-ms")).toBe("263202");
    expect(r.querySelector('[data-testid="routing-resp-ttfb"]')!.getAttribute("data-ms")).toBe("300");
    expect(r.querySelector('[data-testid="routing-resp-tokens"]')!.getAttribute("data-in")).toBe("18200");
    expect(r.querySelector('[data-testid="routing-resp-cost"]')!.getAttribute("data-value")).toBe("0.042");
    expect(r.querySelector('[data-testid="routing-resp-retry"]')!.getAttribute("data-value")).toBe("1");
    expect(r.querySelector('[data-testid="routing-resp-blocked"]')).toBeNull();
  });

  it("路由格含 eval_ms；单模型链每格带模型名与耗时", () => {
    const c = go();
    expect(c.querySelector('[data-node-id="route"] [data-testid="routing-eval-ms"]')!.getAttribute("data-ms")).toBe("30");
    expect(c.querySelector('[data-node-id="s-2-0"] [data-testid="routing-step-model"]')!.textContent).toBe("mm");
    expect(c.querySelector('[data-node-id="s-2-0"] [data-testid="routing-step-ms"]')!.getAttribute("data-ms")).toBe("4400");
  });
});

describe("RoutingCanvas 场景 B：全部失败 503", () => {
  const go = () => mount({ routing: snapB, attempts: attsB, statusCode: 503, durationMs: 1872, blockedBy: "router", blockedReason: "rate_limit", retryCount: 2 });

  it("节点顺序：链上未尝试的模型补跳过格", () => {
    expect(nodeIds(go())).toEqual(["req", "route", "c-1", "s-1-0", "s-1-1", "s-1-2", "c-2", "s-2-0", "c-11", "resp"]);
  });

  it("连线集合：重试线、跳过线 out、换平台 bad、候选耗尽连响应 bad", () => {
    expect(edgeSet(go())).toEqual([
      "c-1->s-1-0:eval",
      "c-2->s-2-0:eval",
      "req->route:eval",
      "route->c-1:eval",
      "route->c-11:out",
      "s-1-0->s-1-1:bad",
      "s-1-1->c-2:bad",
      "s-1-1->s-1-2:out",
      "s-2-0->resp:bad",
    ].sort());
    const c = go();
    expect(edgeLabel(c, "s-1-0->s-1-1")).toBe("retry");
    expect(edgeLabel(c, "s-1-1->c-2")).toBe("switch-service");
    expect(edgeLabel(c, "s-2-0->resp")).toBe("exhausted");
  });

  it("状态与原因：跳过格带链停止原因；限频跳过种类；非声明平台；熔断回填标签；副作用", () => {
    const c = go();
    expect(state(c, "s-1-2")).toBe("skip");
    expect(c.querySelector('[data-node-id="s-1-2"] [data-testid="routing-step-stop"]')!.getAttribute("data-stop")).toBe("service");
    expect(c.querySelector('[data-node-id="s-1-0"]')!.getAttribute("data-kind")).toBe("retry");
    expect(c.querySelector('[data-node-id="s-2-0"]')!.getAttribute("data-kind")).toBe("rate_limit_platform");
    expect(state(c, "s-2-0")).toBe("bad");
    expect(c.querySelector('[data-node-id="c-11"] [data-testid="routing-not-chosen"]')!.getAttribute("data-reason")).toBe("not-declared");
    expect(c.querySelector('[data-node-id="c-2"] [data-testid="routing-tag"]')!.getAttribute("data-tag")).toBe("breaker-backfill");
    const effects = [...c.querySelectorAll('[data-node-id="s-1-1"] [data-testid="routing-step-effect"]')].map((e) => e.getAttribute("data-effect"));
    expect(effects).toEqual(["breaker_count", "penalty_server"]);
    expect(state(c, "c-1")).toBe("fail");
    expect(state(c, "c-2")).toBe("fail");
  });

  it("路由格：直通 / 熔断回填检查项；响应节点带 blocked_by / blocked_reason", () => {
    const c = go();
    expect(c.querySelector('[data-node-id="route"] [data-check="backfill"]')!.getAttribute("data-state")).toBe("warn");
    expect(state(c, "resp")).toBe("fail");
    const b = c.querySelector('[data-node-id="resp"] [data-testid="routing-resp-blocked"]')!;
    expect(b.getAttribute("data-by")).toBe("router");
    expect(b.getAttribute("data-reason")).toBe("rate_limit");
  });
});

describe("RoutingCanvas 场景 C：单平台短路", () => {
  const go = () => mount({ routing: snapC, attempts: [att({ platform_id: 1, model: "gpt-5", duration_ms: 820, ttfb_ms: 310 })], statusCode: 200, durationMs: 820 });

  it("节点与连线：单候选直达响应", () => {
    const c = go();
    expect(nodeIds(c)).toEqual(["req", "route", "c-1", "s-1-0", "resp"]);
    expect(edgeSet(c)).toEqual(["c-1->s-1-0:eval", "req->route:eval", "route->c-1:eval", "s-1-0->resp:ok"].sort());
  });

  it("路由格：短路已触发并列出绕过项（字面单平台）", () => {
    const c = go();
    const sc = c.querySelector('[data-node-id="route"] [data-check="shortcut"]')!;
    expect(sc.getAttribute("data-state")).toBe("warn");
    expect(sc.querySelector("[data-literal]")!.getAttribute("data-literal")).toBe("true");
    expect([...sc.querySelectorAll('[data-testid="routing-bypass"]')].map((e) => e.getAttribute("data-bypass"))).toEqual(["breaker"]);
    expect(c.querySelector('[data-node-id="route"] [data-check="backfill"]')!.getAttribute("data-state")).toBe("na");
  });
});

describe("RoutingCanvas 其他分支", () => {
  it("空快照渲染占位不报错", () => {
    expect(mount({ routing: null }).querySelector('[data-testid="routing-empty"]')).not.toBeNull();
  });

  it("模式徽标", () => {
    expect(mount({ routing: snapA, attempts: attsA }).querySelector('[data-testid="routing-mode"]')).not.toBeNull();
  });

  it("旧快照（无 shortcut / max_retries / eval_ms / kind / error_class）：缺字段处「未记录」，不报错无 NaN", () => {
    const old: RoutingSnapshot = { mode: "failover", entries: [cand({ order: 1, platform_id: 1 }), cand({ order: 2, platform_id: 2 })] };
    const c = mount({ routing: old, attempts: [att({ platform_id: 1, status_code: 502, error: "x" }), att({ platform_id: 2 })] });
    for (const k of ["shortcut", "backfill"]) {
      expect(c.querySelector(`[data-node-id="route"] [data-check="${k}"]`)!.getAttribute("data-state")).toBe("unrec");
    }
    expect(c.querySelector('[data-node-id="route"] [data-testid="routing-eval-ms"]')).toBeNull();
    expect(c.querySelector('[data-node-id="s-1-0"] [data-testid="routing-step-class"]')).toBeNull();
    expect(c.querySelector('[data-node-id="s-1-0"] [data-testid="routing-unrecorded"]')).not.toBeNull();
    expect(state(c, "c-2")).toBe("win");
    expect(c.textContent).not.toMatch(/NaN|undefined/);
  });

  it("无 attempts 的旧日志：候选 dim 未尝试，不崩溃", () => {
    const c = mount({ routing: snapA, attempts: undefined });
    expect(state(c, "c-1")).toBe("dim");
    expect(nodeIds(c)).not.toContain("s-1-0");
  });

  it("限频跳过链（chain_stop=rate_limit）：剩余链模型画成跳过而不是未尝试", () => {
    const snap: RoutingSnapshot = { mode: "failover", max_retries: 3, entries: [cand({ order: 1, platform_id: 1, target_model: "a", model_chain: ["a", "b"] })] };
    const c = mount({ routing: snap, attempts: [att({ platform_id: 1, status_code: 429, error: "rl", model: "a", kind: "rate_limit_platform", chain_stop: "rate_limit" })], statusCode: 503 });
    expect(state(c, "s-1-1")).toBe("skip");
    expect(c.querySelector('[data-node-id="s-1-1"] [data-testid="routing-step-stop"]')!.getAttribute("data-stop")).toBe("rate_limit");
  });

  it("重试上限截断的候选：未选原因 retry-limit", () => {
    const snap: RoutingSnapshot = { mode: "failover", max_retries: 1, entries: [cand({ order: 1, platform_id: 1 }), cand({ order: 3, platform_id: 3, reason: "retry-limit" })] };
    const c = mount({ routing: snap, attempts: [att({ platform_id: 1, status_code: 502, error: "x" })], statusCode: 502 });
    expect(c.querySelector('[data-node-id="c-3"] [data-testid="routing-not-chosen"]')!.getAttribute("data-reason")).toBe("retry-limit");
  });

  it("无尝试成功且无候选可连：路由直连响应", () => {
    const snap: RoutingSnapshot = { mode: "failover", eval_ms: 3, entries: [cand({ order: 1, platform_id: 11, reason: "peak-disabled" })] };
    const c = mount({ routing: snap, attempts: [], statusCode: 503, blockedBy: "router", blockedReason: "peak" });
    expect(edgeSet(c)).toEqual(["req->route:eval", "route->c-11:out", "route->resp:bad"].sort());
  });

  describe("窄屏", () => {
    const saved = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientWidth");
    afterEach(() => { if (saved) Object.defineProperty(HTMLElement.prototype, "clientWidth", saved); else delete (HTMLElement.prototype as unknown as Record<string, unknown>).clientWidth; });

    it("容器 < 720px：纵向布局，节点顺序不变", () => {
      Object.defineProperty(HTMLElement.prototype, "clientWidth", { configurable: true, get: () => 500 });
      const c = mount({ routing: snapC, attempts: [att({ platform_id: 1 })] });
      expect(c.querySelector('[data-testid="routing-flow"]')!.getAttribute("data-layout")).toBe("vertical");
      expect(nodeIds(c)).toEqual(["req", "route", "c-1", "s-1-0", "resp"]);
    });

    it("容器 >= 720px：横向布局", () => {
      Object.defineProperty(HTMLElement.prototype, "clientWidth", { configurable: true, get: () => 1400 });
      const c = mount({ routing: snapC, attempts: [att({ platform_id: 1 })] });
      expect(c.querySelector('[data-testid="routing-flow"]')!.getAttribute("data-layout")).toBe("wide");
    });
  });
});
