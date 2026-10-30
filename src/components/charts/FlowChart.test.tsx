// FlowChart 测试（请求流向票 04）：选择器只锚 data-testid / data-value，不断言文案。
import { describe, it, expect } from "vitest";
import { render } from "../../test/render";
import { FlowChart, formatPct, type FlowChartRow } from "./FlowChart";

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

  it("总和为 0 不显示占比", () => {
    render(<FlowChart columns={[col("a"), col("b")]} rows={rows} metric="cost" />);
    expect(document.querySelectorAll('[data-testid="flow-pct"]')).toHaveLength(0);
    expect(q("flow-node-a-x")).toBeTruthy();
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
});
