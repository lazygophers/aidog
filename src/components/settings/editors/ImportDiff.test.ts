import { describe, it, expect } from "vitest";
import { buildRecommendedDiffTree } from "./ImportDiff";
import { applySelectedPaths } from "../applySelectedPaths";

describe("buildRecommendedDiffTree", () => {
  it("只列推荐配置有的差异，不产生删除项", () => {
    const current = { a: 1, userOnly: "keep" };
    const diff = buildRecommendedDiffTree(current, { a: 2 });
    expect(diff.map((n) => n.path)).toEqual(["a"]);
    expect(diff[0].incoming).toBe(2);
  });

  it("对象键展开一层，且过滤掉用户独有的子键", () => {
    const current = { env: { A: "1", MINE: "x" } };
    const diff = buildRecommendedDiffTree(current, { env: { A: "2", B: "3" } });
    expect(diff).toHaveLength(1);
    expect(diff[0].children?.map((c) => c.path).sort()).toEqual(["env.A", "env.B"]);
  });

  it("无差异时返回空树", () => {
    expect(buildRecommendedDiffTree({ a: 1 }, { a: 1 })).toEqual([]);
  });

  it("跳过 _aidog_ 内部键", () => {
    const diff = buildRecommendedDiffTree({}, { _aidog_hooks: { enabled: true }, a: 1 });
    expect(diff.map((n) => n.path)).toEqual(["a"]);
  });

  it("勾选后按路径落盘，未选中的保持原值", () => {
    const current = { env: { A: "1", MINE: "x" } };
    const source = { env: { A: "2", B: "3" } };
    const next = applySelectedPaths(current, source, new Set(["env.B"]));
    expect(next).toEqual({ env: { A: "1", MINE: "x", B: "3" } });
  });
});
