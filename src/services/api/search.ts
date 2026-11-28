// search.ts — 搜索设置页的管理命令（手动刷新 searx.space 公共实例）。

import { invoke } from "../transport";

/** searx.space 健康实例；seconds = 实测搜索耗时（未知为 null）。 */
export type SearxInstance = { url: string; seconds: number | null };

/** 搜索来源（后端 `search_sources` 唯一真值源；显示名走 i18n `tools.source.<id>`）。 */
export type SearchSource = {
  id: string;
  kind: "keyed" | "engine";
  needs_key: boolean;
  default_on: boolean;
  billing: "per_request" | "free";
};

export const searchApi = {
  /** 全部搜索来源：key 服务在前，免 key 引擎在后。 */
  sources: () => invoke<SearchSource[]>("search_sources"),
  /** 手动拉取一次 searx.space 公共实例列表（用户点按钮才调用，不自动）。 */
  searxInstances: () => invoke<SearxInstance[]>("search_searx_instances"),
  /** 单实例可用性探测：GET <url>/search?q=aidog&format=json，可用回耗时秒（用户点行内「测试」才调用）。 */
  searxTest: (url: string) => invoke<number>("search_searx_test", { url }),
};

/** 单个 key 服务的配置（Rust `aidog_search::ServiceConfig`；keys 经管理接口按后缀脱敏）。 */
export type SearchServiceConfig = { keys: string[] };

/** `mcp_tools_settings_get` 返回（Rust `McpToolsSettings`，已迁移 + 已默认化，永不报错缺记录回默认）。 */
export type McpToolsSettings = {
  search: {
    enabled: boolean;
    sources: string[];
    mode: "sequential" | "parallel";
    services: Record<string, SearchServiceConfig>;
    searxng_base_urls: string[];
  };
  fetch: { enabled: boolean; format: "markdown" | "html" | "text" };
};

export const mcpToolsApi = {
  /** 读 Tools 页设置：后端读时迁移旧形状（票 57-01），前端不再自带平行解析。 */
  settings: () => invoke<McpToolsSettings>("mcp_tools_settings_get"),
};
