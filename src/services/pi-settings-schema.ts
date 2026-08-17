// pi 全局设置 schema（~/.pi/agent/settings.json）— 对照 codex-settings-schema.ts。
// 权威来源：pi `packages/coding-agent/docs/settings.md` 的全局设置表。
// 只收用户真正会调的键；schema 未覆盖的键由「整份读-整份写」保留，不会丢。
//
// 注意：`defaultProvider` 与 `httpProxy` 也由分组同步写入（默认分组 / 出站代理）。
// 用户在本页手改后，下一次分组同步会按 aidog 侧的值覆盖回去 —— 这两项以 aidog 为准。

import type { SettingField, SettingSection } from "./claude-settings-schema";

export const PI_SECTIONS: SettingSection[] = [
  {
    id: "core",
    labelKey: "pi.sectionCore",
    fields: [
      { key: "defaultProvider", label: "Default Provider", type: "string", placeholder: "aidog-<group>", description: "标记默认分组后由 aidog 写入" },
      { key: "defaultModel", label: "Default Model", type: "string" },
      { key: "defaultProjectTrust", label: "Default Project Trust", type: "select", options: ["ask", "always", "never"] },
    ],
  },
  {
    id: "behavior",
    labelKey: "pi.sectionThinking",
    fields: [
      { key: "defaultThinkingLevel", label: "Default Thinking Level", type: "select", options: ["off", "minimal", "low", "medium", "high", "xhigh", "max"] },
      { key: "thinkingBudgets", label: "Thinking Budgets", type: "json", description: "每个思考等级的 token 预算" },
      { key: "hideThinkingBlock", label: "Hide Thinking Block", type: "boolean" },
    ],
  },
  {
    id: "ui",
    labelKey: "pi.sectionUi",
    fields: [
      { key: "theme", label: "Theme", type: "string", placeholder: "dark" },
      { key: "externalEditor", label: "External Editor", type: "string", placeholder: "code -w" },
      { key: "quietStartup", label: "Quiet Startup", type: "boolean" },
      { key: "showCacheMissNotices", label: "Show Cache Miss Notices", type: "boolean" },
    ],
  },
  {
    id: "network",
    labelKey: "pi.sectionNetwork",
    fields: [
      { key: "httpProxy", label: "HTTP Proxy", type: "string", placeholder: "http://127.0.0.1:7890", description: "同时作用于 HTTP_PROXY 与 HTTPS_PROXY" },
    ],
  },
  {
    id: "advanced",
    labelKey: "pi.sectionPrivacy",
    fields: [
      { key: "enableInstallTelemetry", label: "Install Telemetry", type: "boolean" },
      { key: "enableAnalytics", label: "Analytics", type: "boolean" },
    ],
  },
];

/** 推荐配置：pi 官方默认 + aidog 的隐私偏好（两项遥测关掉，与 Codex 页同调）。 */
export const PI_RECOMMENDED_CONFIG: Record<string, unknown> = {
  defaultThinkingLevel: "medium",
  defaultProjectTrust: "ask",
  theme: "dark",
  quietStartup: false,
  hideThinkingBlock: false,
  enableInstallTelemetry: false,
  enableAnalytics: false,
};

export type { SettingField, SettingSection };
