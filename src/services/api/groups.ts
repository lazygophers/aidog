// groups.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import type { RoutingMode, Group, GroupPlatformDetail, ModelMapping, EnvVar, GroupDetail, PlatformUsageStats } from "./types";

export const groupUsageApi = {
  stats: (groupKey: string) =>
    invoke<PlatformUsageStats>("group_usage_stats", { groupKey }),
  // 批量：单次 invoke 返回所有 group → 聚合 map（group_key → stats），消除前端逐 group N+1 往返。
  // 后端 GROUP BY group_key，共享平台不重复计入。
  statsAll: () =>
    invoke<Record<string, PlatformUsageStats>>("all_group_usage_stats"),
};

// ─── Tray Config API ───────────────────────────────────────
// 字段名与 Rust serde（src-tauri/src/gateway/models.rs TrayConfig/TrayItem/TrayColor）保持 snake_case 一致。

/** 单项颜色（三态）。
 * - mode="follow": 跟随系统（labelColor，自适应明暗），value 忽略
 * - mode="preset": value ∈ "red" | "green" | "orange"（systemRed/Green/Orange 自适应）
 * - mode="custom": value = hex（如 "#RRGGBB"），固定色，某些菜单栏主题下可读性差
 */



// ─── Group API ─────────────────────────────────────────────

export const groupApi = {
  create: (input: {
    name: string;
    /** 分组密钥；省略/空 → 后端自动生成 gk_<32hex>。创建后锁定不可改。 */
    group_key?: string;
    routing_mode: RoutingMode;
  }) => invoke<Group>("group_create", { input }),

  list: () => invoke<Group[]>("group_list"),

  get: (id: number) => invoke<Group | null>("group_get", { id }),

  update: (input: {
    id: number;
    name?: string;
    routing_mode?: RoutingMode;
    request_timeout_secs?: number;
    connect_timeout_secs?: number;
    source_protocol?: string;
    /** 分组级最大重试次数（0 = 不重试） */
    max_retries?: number;
    model_mappings?: ModelMapping[];
    /** 用户自定义环境变量（整体替换；同名 ANTHROPIC_BASE_URL/ANTHROPIC_AUTH_TOKEN 后端跳过） */
    env_vars?: EnvVar[];
  }) => invoke<Group>("group_update", { input }),

  delete: (id: number) => invoke<void>("group_delete", { id }),

  /** 设置默认分组（单选）。传 id=null 取消默认（无默认组）。
   * 设置后该组 config merge 写入 ~/.claude/settings.json + ~/.codex/config.toml。 */
  setDefault: (id: number | null) =>
    invoke<void>("group_set_default", { id }),

  /** 拖拽排序：传入按新顺序排列的 group id 列表 */
  reorder: (orderedIds: number[]) =>
    invoke<void>("group_reorder", { orderedIds }),

  setPlatforms: (
    groupId: number,
    platforms: {
      platform_id: number;
      priority?: number;
      weight?: number;
      /** per-group 平台优先级（1~10，省略 → 默认 5；后端 clamp） */
      level_priority?: number;
    }[]
  ) =>
    invoke<void>("group_set_platforms", {
      input: { group_id: groupId, platforms },
    }),

  getPlatforms: (groupId: number) =>
    invoke<GroupPlatformDetail[]>("group_get_platforms", { groupId }),
};

// ─── Aggregate API ─────────────────────────────────────────



// ─── Aggregate API ─────────────────────────────────────────

export const groupDetailApi = {
  get: (id: number) =>
    invoke<GroupDetail | null>("group_detail", { id }),

  list: () => invoke<GroupDetail[]>("group_detail_list"),

  /** 分页取分组详情（触底加载）：offset/limit 页窗，越界返回空数组（前端据此停止）。
   *  后端无 JOIN（单表 group_platform + 内存补 platform），按 sort_order 排序。 */
  listPaged: (offset: number, limit: number) =>
    invoke<GroupDetail[]>("group_detail_list_paged", { offset, limit }),

  /** 分组内平台拖拽排序：orderedIds 按序赋 priority 1,2,3… */
  reorderPlatforms: (groupId: number, orderedIds: number[]) =>
    invoke<void>("group_platform_reorder", { groupId, orderedIds }),

  /** 设置某 group×platform 的 level_priority（1~10，后端 clamp 到 [1,10]） */
  setPlatformLevelPriority: (
    groupId: number,
    platformId: number,
    levelPriority: number
  ) =>
    invoke<void>("group_platform_set_level_priority", {
      groupId,
      platformId,
      levelPriority,
    }),

  /** 跨分组移动平台：从 from 组移除、加入 to 组 */
  movePlatform: (platformId: number, fromGroupId: number, toGroupId: number) =>
    invoke<void>("group_platform_move", { platformId, fromGroupId, toGroupId }),
};

// ─── Proxy API ─────────────────────────────────────────────

