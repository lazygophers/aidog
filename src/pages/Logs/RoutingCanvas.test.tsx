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

function renderCanvas(routing: typeof snapshot | null, attempts?: { platform_id: number; status_code: number }[]) {
  return render(
    <RoutingCanvas
      routing={routing}
      attempts={attempts}
      platformMap={platformMap}
      durationMs={1900}
      statusCode={200}
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
});
