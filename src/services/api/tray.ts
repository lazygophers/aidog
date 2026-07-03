// tray.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import type { TrayConfig, TodayStats, PopoverConfig, TodayPlatformStat } from "./types";

export const trayApi = {
  /** 设选定平台为唯一托盘展示平台，display: "balance" | "coding" */
  set: (platformId: number, display: string) =>
    invoke<void>("platform_set_tray", {
      platformId,
      trayDisplay: display,
      enabled: true,
    }),
  /** 关闭托盘 quota 展示（清空所有平台） */
  clear: () =>
    invoke<void>("platform_set_tray", {
      platformId: 0,
      trayDisplay: "balance",
      enabled: false,
    }),
};


export const trayConfigApi = {
  /** 读取托盘配置（无配置时后端迁移旧 show_in_tray 平台生成默认）。 */
  get: () => invoke<TrayConfig>("tray_config_get"),
  /** 保存托盘配置并刷新托盘渲染。 */
  set: (config: TrayConfig) => invoke<void>("tray_config_set", { config }),
  /** 获取今日统计摘要（tokens / cache_rate / cost / requests）。 */
  todayStats: () => invoke<TodayStats>("tray_today_stats"),
};

// ─── Popover Config API ────────────────────────────────────
// 字段名与 Rust serde（src-tauri/src/gateway/models.rs PopoverConfig/PopoverItem）保持 snake_case 一致。

/** Popover 浮窗预定义指标集 item type。
 * - "today_cost"       今日已用金额
 * - "today_cache_rate" 今日缓存率
 * - "today_tokens"     今日 token 总量
 * - "platform_today"   各平台当日使用（只含已用，列表）
 * - "proxy_status"     代理状态行
 * - "platform_balance" 平台余额 / coding 列（来自 tray 配置）
 * - "cost_trend"       消费趋势曲线（按 scope / time_window 维度）
 * - "platform_metric"  指定平台某时间窗的金额 + token 数值卡（多实例）
 * - "group_cost"       指定分组某时间窗的金额数值卡（多实例）
 * - "group_tokens"     指定分组今日 Token 数值卡（input+output，多实例）
 * - "group_requests"   指定分组今日请求数数值卡（多实例）
 * - "group_balance"    指定分组余额（组内平台 est_balance_remaining 求和，多实例）
 */


export const popoverConfigApi = {
  /** 读取 popover 配置（无配置 → 默认）。 */
  get: () => invoke<PopoverConfig>("popover_config_get"),
  /** 保存 popover 配置。 */
  set: (config: PopoverConfig) => invoke<void>("popover_config_set", { config }),
  /** 各平台当日使用（设置页预览）。 */
  platformToday: () => invoke<TodayPlatformStat[]>("popover_platform_today"),
};

// ─── Group API ─────────────────────────────────────────────

