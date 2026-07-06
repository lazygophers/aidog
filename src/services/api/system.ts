// system.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import type { AppLogSettings, ScriptExecutor, DbCompactResult, AboutInfo } from "./types";

export const scriptExecutorApi = {
  /** 检测 uv 是否可用（true=已安装）。 */
  checkUv: () => invoke<boolean>("check_uv"),
  /**
   * 自动安装 uv（官方安装脚本）。成功返回 true 并持久化执行器为 "uv"。
   * 仅 Unix 支持自动安装；其他平台抛错由前端引导手动安装。
   */
  installUv: () => invoke<boolean>("install_uv"),
  /** 持久化脚本执行器选择，避免每次注入时重复询问。 */
  setExecutor: (executor: ScriptExecutor) =>
    invoke<void>("set_script_executor", { executor }),
};

// ─── Codex Config API ─────────────────────────────────────



// ─── App Log Settings API ─────────────────────────────────

export const appLogApi = {
  get: () => invoke<AppLogSettings>("app_log_settings_get"),
  set: (settings: AppLogSettings) =>
    invoke<void>("app_log_settings_set", { settings }),
};

// ─── DB Maintenance (Tier 1: VACUUM reclaim) ──────────────


export const dbApi = {
  /** 全量 VACUUM 压缩数据库，返回 before/after 字节。锁库期间请求排队。 */
  compact: () => invoke<DbCompactResult>("db_compact"),
};

// ─── CLI 集成联动开关 ──────────────────────────


export const aboutApi = {
  /** 读取应用 / 运行时 / 系统 / 构建版本信息。 */
  info: () => invoke<AboutInfo>("about_info"),
};

// ─── Auto-update toggle (gates 启动 daily check；手动按钮不 gate) ──────────

export const autoUpdateApi = {
  /** 读 auto_update_enabled；缺失默认 true（不打扰存量用户）。 */
  get: () => invoke<boolean>("get_auto_update_enabled"),
  /** 持久化 auto_update_enabled（settings KV scope=app key=auto_update_enabled）。 */
  set: (enabled: boolean) => invoke<void>("set_auto_update_enabled", { enabled }),
};

