// 票 02 就近测试：只锚 data-testid / data-node-id / data-state，禁文案断言（另一 agent 改 locale 值不假红）。
import { describe, it, expect } from "vitest";
import { render } from "../../test/render";
import { RoutingCanvas } from "./RoutingCanvas";
import type { RouteCandidate } from "../../services/api/types/generated/RouteCandidate";

const t = ((key: string, fallback?: string) => fallback ?? key) as unknown as Parameters<typeof RoutingCanvas>[0]["t"];
const platformMap = new Map<number, string>([[1, "alpha"], [2, "beta"], [3, "gamma"]]);

function cand(o: Partial<RouteCandidate> & { platform_id: number }): RouteCandidate {
  return { order: 1, platform: "p", level_priority: 8, priority: 5, reason: "selected", tags: [], ...o };
}

const snapshot = {
  mode: "failover",
  entries: [
    cand({ order: 1, platform_id: 1, target_model: "model-one" }),
    cand({ order: 2, platform_id: 2, tags: ["sticky"] }),
    cand({ order: 3, platform_id: 3, reason: "breaker" }),
  ],
};

function renderCanvas(routing: typeof snapshot | null, attempts?: { platform_id: number; status_code: number; model?: string; error?: string; chain_stop?: string }[], actualModel?: string, requestModel?: string) {
  return render(
    <RoutingCanvas
      routing={routing}
      attempts={attempts}
      platformMap={platformMap}
      durationMs={1900}
      statusCode={200}
      actualModel={actualModel}
      requestModel={requestModel}
      t={t}
    />,
  );
}

describe("RoutingCanvas", () => {
  it("空快照渲染占位不报错", () => {
    const { container } = renderCanvas(null);
    expect(container.querySelector('[data-testid="routing-empty"]')).not.toBeNull();
  });

  it("渲染模式徽标 + 请求/路由/平台/响应节点", () => {
    const { container } = renderCanvas(snapshot, [{ platform_id: 2, status_code: 200 }]);
    expect(container.querySelector('[data-testid="routing-mode"]')).not.toBeNull();
    for (const id of ["req", "route", "p-1", "p-2", "p-3", "resp"]) {
      expect(container.querySelector(`[data-node-id="${id}"]`)).not.toBeNull();
    }
  });

  it("胜出平台 data-state=win，失败尝试 fail，排除项 dim；胜出路径到响应", () => {
    const { container } = renderCanvas(snapshot, [{ platform_id: 1, status_code: 502 }, { platform_id: 2, status_code: 200 }]);
    expect(container.querySelector('[data-node-id="p-2"]')!.getAttribute("data-state")).toBe("win");
    expect(container.querySelector('[data-node-id="p-1"]')!.getAttribute("data-state")).toBe("fail");
    expect(container.querySelector('[data-node-id="p-3"]')!.getAttribute("data-state")).toBe("dim");
    expect(container.querySelector('[data-node-id="resp"]')!.getAttribute("data-state")).toBe("resp");
    expect(container.querySelector("path.rc-edge.win")).not.toBeNull();
    expect(container.querySelector("path.rc-edge.bad")).not.toBeNull();
  });

  it("仅有 target_model 时显示模型元素，旧快照不渲染模型元素", () => {
    const { container } = renderCanvas(snapshot, [{ platform_id: 2, status_code: 200 }]);
    expect(container.querySelectorAll('[data-testid="routing-target-model"]')).toHaveLength(1);
    expect(container.querySelector('[data-node-id="p-1"] [data-testid="routing-target-model"]')?.textContent).toBe("model-one");
  });

  it("排除项带原因胶囊，胜出带数据胶囊", () => {
    const { container } = renderCanvas(snapshot, [{ platform_id: 2, status_code: 200 }]);
    const dim = container.querySelector('[data-node-id="p-3"]')!;
    expect(dim.querySelectorAll(".rc-why").length).toBe(1);
    const win = container.querySelector('[data-node-id="p-2"]')!;
    expect(win.querySelectorAll(".rc-why").length).toBe(1);
    expect(container.querySelector('[data-node-id="p-1"]')!.getAttribute("data-state")).toBe("dim");
    expect(container.querySelector('[data-node-id="p-1"]')!.querySelectorAll(".rc-why").length).toBe(1);
  });

  it("胜出候选无 target_model 时回落 actualModel", () => {
    const { container } = renderCanvas(snapshot, [{ platform_id: 2, status_code: 200 }], "served-model");
    expect(container.querySelector('[data-node-id="p-2"] [data-testid="routing-target-model"]')?.textContent).toBe("served-model");
  });

  it("胜出候选有 target_model 时不被 actualModel 覆盖", () => {
    const { container } = renderCanvas(snapshot, [{ platform_id: 1, status_code: 200 }], "served-model");
    expect(container.querySelector('[data-node-id="p-1"] [data-testid="routing-target-model"]')?.textContent).toBe("model-one");
  });

  it("排除候选出 routing-model-unused；非胜出 selected 无 target_model 两个 testid 都没有", () => {
    const { container } = renderCanvas(snapshot, [{ platform_id: 1, status_code: 200 }], "served-model");
    expect(container.querySelector('[data-node-id="p-3"] [data-testid="routing-model-unused"]')).not.toBeNull();
    expect(container.querySelector('[data-node-id="p-3"] [data-testid="routing-target-model"]')).toBeNull();
    const p2 = container.querySelector('[data-node-id="p-2"]')!;
    expect(p2.querySelector('[data-testid="routing-model-unused"]')).toBeNull();
    expect(p2.querySelector('[data-testid="routing-target-model"]')).toBeNull();
  });

  it("旧日志（无 target_model 字段）不报错", () => {
    const old = { mode: "failover", entries: [{ order: 1, platform: "p", platform_id: 1, level_priority: 8, priority: 5, reason: "selected", tags: [] }] } as unknown as typeof snapshot;
    const { container } = renderCanvas(old, [{ platform_id: 1, status_code: 200 }]);
    expect(container.querySelector('[data-node-id="p-1"]')).not.toBeNull();
  });

  it("模型选择摘要：请求模型 → 胜出候选模型 + 选择来源（来源取自胜出候选 model_via）", () => {
    const snap = {
      mode: "failover",
      entries: [cand({ order: 1, platform_id: 1, target_model: "glm-4.7", model_via: ["mapped", "slot-sonnet"] })],
    } as typeof snapshot;
    const { container } = renderCanvas(snap, [{ platform_id: 1, status_code: 200 }], undefined, "claude-sonnet-4");
    const box = container.querySelector('[data-testid="routing-model-choice"]')!;
    expect(box).not.toBeNull();
    expect(box.querySelector('[data-testid="routing-model-from"]')?.textContent).toBe("claude-sonnet-4");
    expect(box.querySelector('[data-testid="routing-model-to"]')?.textContent).toBe("glm-4.7");
    expect(box.querySelectorAll('[data-testid="routing-model-via"]')).toHaveLength(2);
  });

  it("旧快照无 model_via：摘要仍显示请求→实际，不渲染来源", () => {
    const { container } = renderCanvas(snapshot, [{ platform_id: 1, status_code: 200 }], undefined, "req-m");
    const box = container.querySelector('[data-testid="routing-model-choice"]')!;
    expect(box.querySelector('[data-testid="routing-model-to"]')?.textContent).toBe("model-one");
    expect(box.querySelector('[data-testid="routing-model-via"]')).toBeNull();
  });

  it("空快照不渲染模型选择摘要", () => {
    const { container } = renderCanvas(null, undefined, undefined, "req-m");
    expect(container.querySelector('[data-testid="routing-model-choice"]')).toBeNull();
  });

  // 票 06：平台下整串模型逐项状态（已选用 / 失败 / 跳过 / 未选）
  const chainSnap = {
    mode: "failover",
    entries: [cand({ order: 1, platform_id: 1, target_model: "m-a", model_chain: ["m-a", "m-b", "m-c"] }), cand({ order: 2, platform_id: 2, target_model: "m-x", model_chain: ["m-x", "m-y"] })],
  } as typeof snapshot;
  const chainStates = (c: HTMLElement, id: string) =>
    [...c.querySelectorAll(`[data-node-id="${id}"] [data-testid="routing-chain-item"]`)].map((e) => e.getAttribute("data-state"));

  it("模型链：首模型失败、次模型成功、其后没轮到 → failed / used / untried", () => {
    const { container } = renderCanvas(chainSnap, [
      { platform_id: 1, status_code: 404, model: "m-a", error: "not found" },
      { platform_id: 1, status_code: 200, model: "m-b" },
    ]);
    expect(chainStates(container, "p-1")).toEqual(["failed", "used", "untried"]);
    expect(chainStates(container, "p-2")).toEqual(["untried", "untried"]);
  });

  it("模型链：服务类错误终止 → 未尝试的后续模型标 skipped", () => {
    const { container } = renderCanvas(chainSnap, [
      { platform_id: 1, status_code: 429, model: "m-a", chain_stop: "service" },
      { platform_id: 2, status_code: 200, model: "m-x" },
    ]);
    expect(chainStates(container, "p-1")).toEqual(["failed", "skipped", "skipped"]);
    expect(chainStates(container, "p-2")).toEqual(["used", "untried"]);
  });

  it("链长 1 / 无 model_chain：不渲染逐项行，保持单行模型", () => {
    const { container } = renderCanvas(snapshot, [{ platform_id: 1, status_code: 200 }]);
    expect(container.querySelector('[data-testid="routing-chain-item"]')).toBeNull();
    expect(container.querySelector('[data-node-id="p-1"] [data-testid="routing-target-model"]')).not.toBeNull();
  });
});
