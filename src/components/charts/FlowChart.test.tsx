// FlowChart 测试（请求流向票 04）：选择器只锚 data-testid / data-value，不断言文案。
import { describe, it, expect, afterEach } from "vitest";
import { fireEvent } from "@testing-library/react";
import { render } from "../../test/render";
import { FlowChart, formatPct, computeLayout, countCrossings, orderColumns, type FlowChartRow } from "./FlowChart";

const col = (key: string) => ({ key, title: key });
const rows: FlowChartRow[] = [
  { a: "x", b: "p", c: "m1", d: "z", tokens: 60, requests: 1, cost: 0 },
  { a: "y", b: "q", c: "m2", d: "z", tokens: 30, requests: 5, cost: 0 },
  { a: "y", b: "p", c: "m1", d: "w", tokens: 10, requests: 4, cost: 0 },
];
const pcts = (el: Element) => [...el.querySelectorAll('[data-testid="flow-pct"]')].map(e => Number(e.getAttribute("data-value")));
const q = (id: string) => document.querySelector(`[data-testid="${id}"]`)!;

describe("FlowChart", () => {
  it.each([3, 4, 5])("列数可配：%i 列渲染对应 flow-col-*", n => {
    const keys = ["a", "b", "c", "d", "e"].slice(0, n);
    const data = rows.map(r => ({ ...r, e: "k" }));
    render(<FlowChart columns={keys.map(col)} rows={data} metric="tokens" />);
    expect(document.querySelectorAll('[data-testid^="flow-col-"]')).toHaveLength(n);
    for (const k of keys) expect(q(`flow-col-${k}`)).toBeTruthy();
  });

  it("每列 Top6 + 其他；同列占比之和约 100", () => {
    const many = Array.from({ length: 9 }, (_, i) => ({ a: `n${i}`, b: "p", tokens: 10 + i, requests: 1, cost: 0 }));
    render(<FlowChart columns={[col("a"), col("b")]} rows={many} metric="tokens" />);
    const colA = q("flow-col-a");
    expect(colA.querySelectorAll('[data-testid^="flow-node-a-"]')).toHaveLength(7);
    expect(q("flow-node-a-__other__")).toBeTruthy();
    expect(pcts(colA).reduce((s, v) => s + v, 0)).toBeCloseTo(100, 5);
  });

  it("度量切换：数值与占比同步变化", () => {
    const { rerender } = render(<FlowChart columns={[col("a"), col("b")]} rows={rows} metric="tokens" />);
    expect(pcts(q("flow-node-a-x"))).toEqual([60]);
    rerender(<FlowChart columns={[col("a"), col("b")]} rows={rows} metric="requests" />);
    expect(pcts(q("flow-node-a-x"))).toEqual([10]);
    expect(pcts(q("flow-node-a-y"))).toEqual([90]);
  });

  it("度量总和为 0：不画任何节点与占比", () => {
    render(<FlowChart columns={[col("a"), col("b")]} rows={rows} metric="cost" />);
    expect(document.querySelectorAll('[data-testid="flow-pct"]')).toHaveLength(0);
    expect(document.querySelectorAll('[data-testid^="flow-node-"]')).toHaveLength(0);
  });

  it("值为 0 的节点与连线不画；其他为 0 不出现", () => {
    const data = [
      { a: "x", b: "p", tokens: 50, requests: 0, cost: 0 },
      { a: "z", b: "p", tokens: 0, requests: 3, cost: 0 },
    ];
    render(<FlowChart columns={[col("a"), col("b")]} rows={data} metric="tokens" />);
    expect(document.querySelector('[data-testid="flow-node-a-z"]')).toBeNull();
    expect(document.querySelector('[data-testid="flow-node-a-__other__"]')).toBeNull();
    expect(document.querySelectorAll('[data-testid^="flow-link-"]')).toHaveLength(1);
  });

  it("节点高度与值成比例，连线宽度 = 流量占比 × 比例尺", () => {
    render(<FlowChart columns={[col("a"), col("b")]} rows={rows} metric="tokens" />);
    const h = (id: string) => Number(q(`flow-node-a-${id}`).getAttribute("data-height"));
    expect(h("x") / h("y")).toBeCloseTo(60 / 40, 5);
    const w = Number(q("flow-link-a-x-p").getAttribute("data-width"));
    expect(w).toBeCloseTo(h("x"), 5); // x 的全部流量走 x→p
  });

  it("空 id 有可读显示名", () => {
    const data = [{ a: "", b: "p", tokens: 5, requests: 1, cost: 0 }];
    render(<FlowChart columns={[col("a"), col("b")]} rows={data} metric="tokens" />);
    expect(q("flow-node-a-").textContent?.length).toBeGreaterThan(0);
  });

  it("hover 节点：链路外的连线 data-state=off，链路内 on", () => {
    render(<FlowChart columns={[col("a"), col("b")]} rows={rows} metric="tokens" />);
    fireEvent.mouseEnter(q("flow-node-a-x"));
    expect(q("flow-link-a-x-p").getAttribute("data-state")).toBe("on");
    expect(q("flow-link-a-y-q").getAttribute("data-state")).toBe("off");
    expect(q("flow-tooltip")).toBeTruthy();
  });

  it("labelOf 缺省回退原 id；提供则用其结果", () => {
    const cols = [{ key: "a", title: "a", labelOf: (id: string) => (id === "x" ? "显示X" : id) }, col("b")];
    render(<FlowChart columns={cols} rows={rows} metric="tokens" />);
    expect(q("flow-node-a-x").textContent).toContain("显示X");
    expect(q("flow-node-a-y").textContent).toContain("y");
  });

  it("formatPct：1 位小数，<0.1 兜底", () => {
    expect(formatPct(12.345)).toBe("12.3%");
    expect(formatPct(0.05)).toBe("<0.1%");
    expect(formatPct(0)).toBe("0.0%");
  });

  describe("prefers-reduced-motion", () => {
    const orig = window.matchMedia;
    afterEach(() => { window.matchMedia = orig; });
    const mockReduced = (reduce: boolean) => {
      window.matchMedia = ((query: string) => ({ matches: reduce && query.includes("reduce"), media: query, addEventListener() {}, removeEventListener() {} })) as unknown as typeof window.matchMedia;
    };

    it("减弱模式：根节点标记 reduced，流光仍渲染，节点直接落位（无高度补间）", () => {
      mockReduced(true);
      render(<FlowChart columns={[col("a"), col("b")]} rows={rows} metric="tokens" />);
      expect(q("flow-chart").getAttribute("data-motion")).toBe("reduced");
      expect(document.querySelectorAll('[data-testid="flow-shine"]').length).toBeGreaterThan(0);
      expect(Number(q("flow-node-a-x").querySelector("rect")!.getAttribute("height"))).toBeCloseTo(Number(q("flow-node-a-x").getAttribute("data-height")), 5);
    });

    it("常规模式：data-motion=full", () => {
      mockReduced(false);
      render(<FlowChart columns={[col("a"), col("b")]} rows={rows} metric="tokens" />);
      expect(q("flow-chart").getAttribute("data-motion")).toBe("full");
    });
  });
});

describe("FlowChart 降交叉", () => {
  // 按流量降序的初始序 a:[a1,a2,a3] b:[b1,b2] c:[c1,c2]，但 a1→b2→c2、a2→b1→c1、a3→b1→c1：初始 2 次交叉，重排后 0
  const rev: FlowChartRow[] = [
    { a: "a1", b: "b2", c: "c2", tokens: 30, requests: 1, cost: 0 },
    { a: "a2", b: "b1", c: "c1", tokens: 20, requests: 1, cost: 0 },
    { a: "a3", b: "b1", c: "c1", tokens: 15, requests: 1, cost: 0 },
  ];
  const cols = ["a", "b", "c"].map(col);
  const edgesOf = (L: ReturnType<typeof computeLayout>) =>
    cols.slice(0, -1).map((_c, ci) => new Map(L.links.filter(l => l.ci === ci).map(l => [`${l.a}\u0000${l.b}`, l.v])));

  it("orderColumns：反向数据交叉数下降，且不会比初始更差", () => {
    const init = [["a1", "a2", "a3"], ["b1", "b2", "b3"], ["c1", "c2", "c3"]];
    const edges = [
      new Map([["a1\u0000b3", 30], ["a2\u0000b2", 20], ["a3\u0000b1", 10]]),
      new Map([["b3\u0000c3", 30], ["b2\u0000c2", 20], ["b1\u0000c1", 10]]),
    ];
    const before = countCrossings(init, edges);
    expect(before).toBeGreaterThan(0);
    expect(countCrossings(orderColumns(init, edges), edges)).toBe(0);
  });

  it("computeLayout：典型数据 0 交叉；节点排序后 ids 仍是原集合", () => {
    const L = computeLayout(cols, rev, "tokens");
    const order = L.cols.map(c => c.ids);
    expect(countCrossings(order, edgesOf(L))).toBe(0);
    expect(order.map(o => [...o].sort())).toEqual([["a1", "a2", "a3"], ["b1", "b2"], ["c1", "c2"]]);
    const init = [["a1", "a2", "a3"], ["b1", "b2"], ["c1", "c2"]];
    expect(countCrossings(init, edgesOf(L))).toBe(2);
  });

  it("「其他」恒在列底，且节点高度比例不变", () => {
    const many = Array.from({ length: 9 }, (_, i) => ({ a: `n${i}`, b: `p${8 - i}`, tokens: 10 + i, requests: 1, cost: 0 }));
    const L = computeLayout([col("a"), col("b")], many, "tokens");
    for (const c of L.cols) expect(c.ids[c.ids.length - 1]).toBe("__other__");
    const ns = L.nodes.filter(n => n.ci === 0);
    for (const n of ns) expect(n.h / n.v).toBeCloseTo(ns[0].h / ns[0].v, 5);
  });
});
