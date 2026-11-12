// Flow3DChart 链路聚合纯函数：按列元组聚合完整链路、每列 Top N 外合并为「其他」、占比相对全部。
import { describe, it, expect } from "vitest";
import { aggregateFlowPaths, FLOW3D_OTHER } from "../Flow3DChart";

const cols = [{ key: "a", title: "A" }, { key: "b", title: "B" }];

describe("aggregateFlowPaths", () => {
  it("同列元组的行合并成一条链路，按度量值降序，占比相对全部", () => {
    const rows = [
      { a: "x", b: "m1", requests: 3, tokens: 30, cost: 0 },
      { a: "x", b: "m1", requests: 1, tokens: 10, cost: 0 },
      { a: "y", b: "m2", requests: 4, tokens: 5, cost: 0 },
    ];
    const r = aggregateFlowPaths(rows, cols, "requests");
    expect(r.total).toBe(8);
    expect(r.paths.map(p => [p.ids, p.value, p.share])).toEqual([
      [["x", "m1"], 4, 50],
      [["y", "m2"], 4, 50],
    ]);
    expect(aggregateFlowPaths(rows, cols, "tokens").paths[0]).toMatchObject({ ids: ["x", "m1"], value: 40 });
  });

  it("值为 0 的行不成链路", () => {
    const r = aggregateFlowPaths([{ a: "x", b: "m", requests: 0, tokens: 0, cost: 0 }], cols, "requests");
    expect(r.paths).toEqual([]);
    expect(r.total).toBe(0);
  });

  it("每列超过 Top N 的节点合并为「其他」，链路随之合并", () => {
    const rows = Array.from({ length: 8 }, (_, i) => ({ a: "x", b: `m${i}`, requests: 10 - i, tokens: 0, cost: 0 }));
    const r = aggregateFlowPaths(rows, cols, "requests", 6);
    expect(r.layers[1].map(n => n.id)).toEqual(["m0", "m1", "m2", "m3", "m4", "m5", FLOW3D_OTHER]);
    const other = r.paths.find(p => p.ids[1] === FLOW3D_OTHER)!;
    expect(other.value).toBe(4 + 3);
    expect(r.paths).toHaveLength(7);
    expect(r.total).toBe(52);
    expect(other.share).toBeCloseTo(7 / 52 * 100);
  });
});
