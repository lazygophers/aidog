// search.ts — 搜索设置页的管理命令（手动刷新 searx.space 公共实例）。

import { invoke } from "../transport";

/** searx.space 健康实例；seconds = 实测搜索耗时（未知为 null）。 */
export type SearxInstance = { url: string; seconds: number | null };

export const searchApi = {
  /** 手动拉取一次 searx.space 公共实例列表（用户点按钮才调用，不自动）。 */
  searxInstances: () => invoke<SearxInstance[]>("search_searx_instances"),
};
