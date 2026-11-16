// plugins.ts — 扩展 → 插件管理（Claude Code 原生插件 + Codex 插件 CLI）。

import { invoke } from "../transport";

export interface ClaudePluginView {
  /** <plugin>@<marketplace> */
  id: string;
  version: string;
  installPath: string;
  lastUpdated: string;
  /** enabledPlugins 缺键 = 启用（Claude Code 缺省语义）。 */
  enabled: boolean;
  /** enabledPlugins 里有无该键。 */
  configured: boolean;
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
  claudeList: () => invoke<ClaudePluginView[]>("plugins_claude_list"),
  /** 启停 Claude Code 插件：写全局 claude_code.enabledPlugins 并触发分组同步。 */
  claudeSet: (pluginId: string, enabled: boolean) =>
    invoke<ClaudePluginView[]>("plugins_claude_set", { pluginId, enabled }),
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
