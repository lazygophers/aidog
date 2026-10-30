import { describe, expect, it } from "vitest";
import codexOss from "../../src-tauri/defaults/codex/oss.json";
import piOss from "../../src-tauri/defaults/pi/oss.json";
import { renderPresetPlaceholders } from "./preset-select";

// 期望值取自迁移前 master (c998452) 的 buildCodexRecommendedConfig(port) / PI_RECOMMENDED_CONFIG 字面值。
const legacyCodex = (port: number) => ({
  model: "gpt-5.5",
  model_provider: "aidog",
  model_reasoning_effort: "medium",
  approval_policy: "on-request",
  sandbox_mode: "workspace-write",
  sandbox_workspace_write: { network_access: false },
  web_search: "cached",
  personality: "pragmatic",
  analytics: { enabled: false },
  model_providers: {
    aidog: {
      name: "aidog proxy",
      base_url: `http://127.0.0.1:${port}/proxy`,
      wire_api: "responses",
      env_key: "AIDOG_KEY",
    },
  },
});

describe("oss 预设 = 迁移前常量", () => {
  it("codex 渲染后与旧 buildCodexRecommendedConfig 深相等（9890 / 19890）", () => {
    expect(renderPresetPlaceholders(codexOss, { port: 9890 })).toEqual(legacyCodex(9890));
    expect(renderPresetPlaceholders(codexOss, { port: 19890 })).toEqual(legacyCodex(19890));
  });
  it("pi 与旧 PI_RECOMMENDED_CONFIG 深相等", () => {
    expect(piOss).toEqual({
      defaultThinkingLevel: "medium",
      defaultProjectTrust: "ask",
      theme: "dark",
      quietStartup: false,
      hideThinkingBlock: false,
      enableInstallTelemetry: false,
      enableAnalytics: false,
    });
  });
});
