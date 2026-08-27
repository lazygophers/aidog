// updateConfigField 的外部行为：写一边，另一边跟着变；删一边，另一边一起删。
// 只断言返回的配置对象，不碰组件。

import { describe, it, expect } from "vitest";
import { updateConfigField } from "./settings-env-pairs";

describe("updateConfigField", () => {
  it("改 settings 键，语义相反的环境变量写成相反值", () => {
    const next = updateConfigField({}, "fastMode", false);
    expect(next.fastMode).toBe(false);
    expect(next.env).toEqual({ CLAUDE_CODE_DISABLE_FAST_MODE: "1" });

    const on = updateConfigField(next, "fastMode", true);
    expect(on.env).toEqual({ CLAUDE_CODE_DISABLE_FAST_MODE: "0" });
  });

  it("改 settings 键，语义相同的环境变量写成同值", () => {
    const next = updateConfigField({}, "disableArtifact", true);
    expect(next.env).toEqual({ CLAUDE_CODE_DISABLE_ARTIFACT: "1" });
  });

  it("标量配对按原样写进环境变量", () => {
    const next = updateConfigField({}, "effortLevel", "high");
    expect(next.env).toEqual({ CLAUDE_CODE_EFFORT_LEVEL: "high" });
  });

  it("改环境变量，对应 settings 键跟着变（相反语义会翻转）", () => {
    const prev = { fastMode: true, env: { CLAUDE_CODE_DISABLE_FAST_MODE: "0" } };
    const next = updateConfigField(prev, "env", { CLAUDE_CODE_DISABLE_FAST_MODE: "1" });
    expect(next.fastMode).toBe(false);
  });

  it("删掉 settings 键时，配对的环境变量一起删", () => {
    const prev = { effortLevel: "high", env: { CLAUDE_CODE_EFFORT_LEVEL: "high", DEBUG: "1" } };
    const next = updateConfigField(prev, "effortLevel", undefined);
    expect(next.effortLevel).toBeUndefined();
    expect(next.env).toEqual({ DEBUG: "1" });
  });

  it("删掉环境变量时，配对的 settings 键一起删", () => {
    const prev = { effortLevel: "high", env: { CLAUDE_CODE_EFFORT_LEVEL: "high" } };
    const next = updateConfigField(prev, "env", undefined);
    expect(next.effortLevel).toBeUndefined();
    expect(next.env).toBeUndefined();
  });

  it("没动过的环境变量不会反压 settings 键", () => {
    // 用户先把 fastMode 关了，再去 env 编辑器加了个无关变量：
    // fastMode 必须保持关闭，不能被 env 里的旧值倒推回来。
    const prev = updateConfigField({}, "fastMode", false);
    const next = updateConfigField(prev, "env", { ...prev.env, DEBUG: "1" });
    expect(next.fastMode).toBe(false);
    expect(next.env.DEBUG).toBe("1");
  });

  it("未配对的字段行为不变：false 保留，空串视为删除", () => {
    const kept = updateConfigField({}, "includeCoAuthoredBy", false);
    expect(kept.includeCoAuthoredBy).toBe(false);
    expect(kept.env).toBeUndefined();

    const dropped = updateConfigField({ outputStyle: "Concise" }, "outputStyle", "");
    expect(dropped.outputStyle).toBeUndefined();
  });

  it("不改入参", () => {
    const prev = { fastMode: true, env: { DEBUG: "1" } };
    const snapshot = JSON.stringify(prev);
    updateConfigField(prev, "fastMode", false);
    expect(JSON.stringify(prev)).toBe(snapshot);
  });
});
