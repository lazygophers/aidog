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
};
