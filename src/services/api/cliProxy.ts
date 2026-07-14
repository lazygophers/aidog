// cliProxy.ts — CLI 代理 provider CRUD + 导入 + 测试 + 建平台 Tauri invoke 封装。
// 对齐 `commands_cli_proxy` crate 的 #[tauri::command] 函数名（snake_case）。
// 域类型从 "./types" 取（barrel re-export 经 types.ts → part5.ts）。

import { invoke } from "@tauri-apps/api/core";
import type {
  CliProxyProvider,
  CreateCliProxyProvider,
  CliProxyImportResult,
  PlatformQuota,
  Platform,
} from "./types";

export const cliProxyApi = {
  /** 列出全部 cli_proxy_provider。 */
  list: () => invoke<CliProxyProvider[]>("cli_proxy_list"),
  /** 获取单个 cli_proxy_provider。不存在返回 null。 */
  get: (id: number) => invoke<CliProxyProvider | null>("cli_proxy_get", { id }),
  /** 创建 cli_proxy_provider。 */
  create: (input: CreateCliProxyProvider) =>
    invoke<CliProxyProvider>("cli_proxy_create", { input }),
  /** 全量覆写更新。不存在返回 null。 */
  update: (id: number, input: CreateCliProxyProvider) =>
    invoke<CliProxyProvider | null>("cli_proxy_update", { id, input }),
  /** 删除。不存在返回 false。 */
  delete: (id: number) => invoke<boolean>("cli_proxy_delete", { id }),
  /** 探测余额（复用 platform quota 子系统）。 */
  test: (id: number) => invoke<PlatformQuota>("cli_proxy_test", { id }),
  /** 建 cli-proxy platform 行（extra 存 cli_proxy_provider_id 指向 provider）。 */
  createPlatform: (
    providerId: number,
    name?: string,
    groupId?: number | null,
  ) =>
    invoke<Platform>("create_cli_proxy_platform", {
      providerId,
      name: name ?? null,
      groupId: groupId ?? null,
    }),
  /** 解析 CPA 配置 → 批量创建 provider（非原子尽力）。 */
  import: (path: string, authDir?: string, groupId?: number | null) =>
    invoke<CliProxyImportResult>("cli_proxy_import", {
      path,
      authDir: authDir ?? null,
      groupId: groupId ?? null,
    }),
};
