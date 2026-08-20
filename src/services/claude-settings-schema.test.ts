import { describe, it, expect } from "vitest";
import {
  SECTIONS,
  ENV_VAR_DEFS,
  ENV_VAR_GROUP_ORDER,
  ENV_VAR_GROUP_LABEL_KEYS,
  ALL_SETTING_KEYS,
} from "./claude-settings-schema";

describe("claude-settings-schema", () => {
  it("每个 settings 键只出现一次", () => {
    const dupes = ALL_SETTING_KEYS.filter((k, i) => ALL_SETTING_KEYS.indexOf(k) !== i);
    expect(dupes).toEqual([]);
  });

  it("每个 section 的 id 唯一且有 labelKey", () => {
    const ids = SECTIONS.map((s) => s.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const s of SECTIONS) expect(s.labelKey).toMatch(/^settings\.section/);
  });

  it("object 字段必须声明 objectFields，kv-select 必须声明 valueOptions", () => {
    for (const f of SECTIONS.flatMap((s) => s.fields)) {
      if (f.type === "object") expect(f.objectFields?.length, f.key).toBeGreaterThan(0);
      if (f.type === "kv-select") expect(f.valueOptions?.length, f.key).toBeGreaterThan(0);
      if (f.type === "select") expect(f.options?.length, f.key).toBeGreaterThan(0);
    }
  });

  it("每个环境变量只出现一次", () => {
    const keys = ENV_VAR_DEFS.map((d) => d.key);
    const dupes = keys.filter((k, i) => keys.indexOf(k) !== i);
    expect(dupes).toEqual([]);
  });

  it("环境变量的 group 都在 ENV_VAR_GROUP_ORDER 内且有 i18n labelKey", () => {
    for (const d of ENV_VAR_DEFS) {
      expect(ENV_VAR_GROUP_ORDER as readonly string[], d.key).toContain(d.group);
      expect(ENV_VAR_GROUP_LABEL_KEYS[d.group], d.group).toBeTruthy();
    }
  });

  it("select 类型的环境变量必须带 options", () => {
    for (const d of ENV_VAR_DEFS.filter((d) => d.type === "select")) {
      expect(d.options?.length, d.key).toBeGreaterThan(0);
    }
  });

  // 上游文档对齐回归护栏：这些键 2026-08 从官方 settings / env-vars 文档补入，
  // 误删会让可视化面板重新出现覆盖缺口。
  it("覆盖官方文档新增的关键 settings 键", () => {
    for (const k of [
      "advisorModel", "availableModels", "fallbackModel", "skillOverrides",
      "skillListingMaxDescChars", "autoCompactEnabled", "fileCheckpointingEnabled",
      "askUserQuestionTimeout", "crossSessionInbound", "theme", "voice",
      "spinnerVerbs", "diffTool", "enableAllProjectMcpServers", "disableRemoteControl",
    ]) {
      expect(ALL_SETTING_KEYS, k).toContain(k);
    }
  });

  it("maxSkillDescriptionChars 已替换为上游的 skillListingMaxDescChars", () => {
    expect(ALL_SETTING_KEYS).not.toContain("maxSkillDescriptionChars");
    expect(ALL_SETTING_KEYS).toContain("skillListingMaxDescChars");
  });

  it("覆盖官方 env-vars 文档的全部 332 个变量（含额外 OTel 标准变量）", () => {
    expect(ENV_VAR_DEFS.length).toBeGreaterThanOrEqual(332);
    for (const k of [
      "MCP_TIMEOUT", "OTEL_LOG_USER_PROMPTS", "CLAUDE_CONFIG_DIR",
      "VERTEX_REGION_CLAUDE_5_OPUS", "CLAUDE_CODE_SYNC_SKILLS", "DO_NOT_TRACK",
    ]) {
      expect(ENV_VAR_DEFS.map((d) => d.key), k).toContain(k);
    }
  });
});
