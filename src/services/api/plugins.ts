// plugins.ts — 扩展 → 插件管理（Claude 插件捆绑包 + Codex 插件 CLI）。

import { invoke } from "../transport";

/** aidog 库中安装的 Claude 插件（部件以普通技能 / 子代理 / 钩子 / MCP 分发）。 */
export interface ClaudePluginView {
  name: string;
  source: string;
  pick: string | null;
  agents: string[];
  on: boolean;
  skills: string[];
  subagents: string[];
  hooks: string[];
  servers: string[];
  /** 未携带部件（slash commands、LSP servers 等）。 */
  left: string[];
}

/** 操作结果含插件清单 + 用户改过未撤回的部件 warning。 */
export interface ClaudePluginViewList {
  plugins: ClaudePluginView[];
  warnings: string[];
}

export interface ClaudePluginCandidate {
  name: string;
  description: string;
  version: string;
  rel: string;
  installed: boolean;
}

export interface CodexPlugin {
  pluginId: string;
  name: string;
  marketplaceName: string;
  version: string;
  installed: boolean;
  enabled: boolean;
}

export interface CodexMarketplace {
  name: string;
  source: string;
  root: string;
}

export interface CodexPluginsView {
  installed: CodexPlugin[];
  marketplaces: CodexMarketplace[];
}

export const pluginsApi = {
  claudeList: () => invoke<ClaudePluginViewList>("plugins_bundle_list"),
  claudeProbe: (source: string) => invoke<ClaudePluginCandidate[]>("plugins_bundle_probe", { source }),
  claudeInstall: (source: string, agents: string[], selected?: string[]) =>
    invoke<ClaudePluginViewList>("plugins_bundle_install", { source, agents, selected }),
  claudeSet: (name: string, on: boolean) => invoke<ClaudePluginViewList>("plugins_bundle_set", { name, on }),
  claudeSetAgents: (name: string, agents: string[]) =>
    invoke<ClaudePluginViewList>("plugins_bundle_set_agents", { name, agents }),
  claudeRemove: (name: string) => invoke<ClaudePluginViewList>("plugins_bundle_remove", { name }),
  claudeUpdate: (name: string) => invoke<ClaudePluginViewList>("plugins_bundle_update", { name }),
  codexList: () => invoke<CodexPluginsView>("plugins_codex_list"),
  /** pluginId 形如 name@marketplace；返回变更后的列表。 */
  codexAdd: (pluginId: string) => invoke<CodexPluginsView>("plugins_codex_add", { pluginId }),
  codexRemove: (pluginId: string) => invoke<CodexPluginsView>("plugins_codex_remove", { pluginId }),
  /** source 为本地路径或 git 地址；返回变更后的列表。 */
  codexMarketplaceAdd: (source: string) =>
    invoke<CodexPluginsView>("plugins_codex_marketplace_add", { source }),
  codexMarketplaceRemove: (name: string) =>
    invoke<CodexPluginsView>("plugins_codex_marketplace_remove", { name }),
};
