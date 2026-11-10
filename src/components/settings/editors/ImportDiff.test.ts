import { describe, it, expect } from "vitest";
import { buildImportDiffTree, buildRecommendedDiffTree, pickAidogKeys } from "./ImportDiff";
import { applySelectedPaths } from "../applySelectedPaths";

describe("buildRecommendedDiffTree", () => {
  it("推荐配置里没有的键列为删除项", () => {
    const diff = buildRecommendedDiffTree({ a: 1, userOnly: "keep" }, { a: 2 });
    expect(diff.map((n) => n.path).sort()).toEqual(["a", "userOnly"]);
    expect(diff.find((n) => n.path === "userOnly")!.incoming).toBeUndefined();
  });

  it("对象键展开一层，用户独有的子键也列为删除项", () => {
    const current = { env: { A: "1", MINE: "x" } };
    const diff = buildRecommendedDiffTree(current, { env: { A: "2", B: "3" } });
    expect(diff).toHaveLength(1);
    expect(diff[0].children?.map((c) => c.path).sort()).toEqual([
      "env.A", "env.B", "env.MINE",
    ]);
  });

  it("勾选删除项后该键从结果里移除", () => {
    const next = applySelectedPaths(
      { env: { A: "1", MINE: "x" } },
      { env: { A: "2" } },
      new Set(["env.MINE"]),
    );
    expect(next).toEqual({ env: { A: "1" } });
  });

  it("无差异时返回空树", () => {
    expect(buildRecommendedDiffTree({ a: 1 }, { a: 1 })).toEqual([]);
  });

  it("除 _aidog_* 外全部对比：物化残留 statusLine 与原生 hooks/language 都入树", () => {
    const current = {
      statusLine: { type: "command", command: "uv run --script ~/.aidog/scripts/aidog-statusline.py" },
      hooks: { PreToolUse: [{ hooks: [{ type: "command", command: "rtk hook claude" }] }] },
      language: "zh-SG",
      userOnly: "keep",
    };
    const recommended = {
      _aidog_statusline: { enabled: true },
      hooks: { Stop: [{ hooks: [{ type: "command", command: "tracking_progress.py" }] }] },
      language: "zh-Hans",
    };
    const diff = buildRecommendedDiffTree(current, recommended);
    expect(diff.map((n) => n.path).sort()).toEqual(["hooks", "language", "statusLine", "userOnly"]);
    // hooks 展开到事件级子节点
    expect(diff.find((n) => n.path === "hooks")!.children?.map((c) => c.path).sort())
      .toEqual(["hooks.PreToolUse", "hooks.Stop"]);
    expect(diff.find((n) => n.path === "language")!.incoming).toBe("zh-Hans");
  });

  it("推荐路径不做 CC runtime 过滤：effortLevel 等也对比", () => {
    const diff = buildRecommendedDiffTree({ effortLevel: "high" }, { effortLevel: "medium" });
    expect(diff.map((n) => n.path)).toEqual(["effortLevel"]);
  });

  it("推荐配置新增的未知 key 自动入 diff（兼容后续新增配置 key）", () => {
    const diff = buildRecommendedDiffTree({}, { futureNewKey: { nested: [1, 2] } });
    expect(diff.map((n) => n.path)).toEqual(["futureNewKey"]);
    expect(diff[0].children?.map((c) => c.path)).toEqual(["futureNewKey.nested"]);
  });

  it("跳过 _aidog_ 内部键", () => {
    const diff = buildRecommendedDiffTree({}, { _aidog_hooks: { enabled: true }, a: 1 });
    expect(diff.map((n) => n.path)).toEqual(["a"]);
  });

  it("推荐路径展示 _aidog_claude_json 段的键级变更（写 ~/.claude.json，非内部状态）", () => {
    const diff = buildRecommendedDiffTree(
      { _aidog_claude_json: { copyOnSelect: true } },
      { _aidog_claude_json: { copyOnSelect: false, diffTool: "delta" }, a: 1 },
    );
    expect(diff.map((n) => n.path)).toEqual(["_aidog_claude_json", "a"]);
    expect(diff[0].children?.map((c) => c.path).sort()).toEqual([
      "_aidog_claude_json.copyOnSelect", "_aidog_claude_json.diffTool",
    ]);
    // 勾选结果逐路径生效
    const next = applySelectedPaths(
      { _aidog_claude_json: { copyOnSelect: true } },
      { _aidog_claude_json: { copyOnSelect: false, diffTool: "delta" } },
      new Set(["_aidog_claude_json.copyOnSelect"]),
    );
    expect(next).toEqual({ _aidog_claude_json: { copyOnSelect: false } });
  });

  it("导入路径（Claude Code settings.json）仍跳过 _aidog_claude_json", () => {
    const diff = buildImportDiffTree({}, { _aidog_claude_json: { copyOnSelect: true } }, new Set());
    expect(diff).toEqual([]);
  });

  it("pickAidogKeys 只取无条件合并的 _aidog_* 内部键，_aidog_claude_json 走勾选", () => {
    const picked = pickAidogKeys({
      _aidog_statusline: { enabled: true },
      _aidog_claude_json: { copyOnSelect: false },
      a: 1,
    });
    expect(picked).toEqual({ _aidog_statusline: { enabled: true } });
  });

  it("勾选后按路径落盘，未选中的保持原值", () => {
    const current = { env: { A: "1", MINE: "x" } };
    const source = { env: { A: "2", B: "3" } };
    const next = applySelectedPaths(current, source, new Set(["env.B"]));
    expect(next).toEqual({ env: { A: "1", MINE: "x", B: "3" } });
  });
});
