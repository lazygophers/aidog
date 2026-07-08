// system.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import type { AppLogSettings, ScriptExecutor, DbCompactResult, AboutInfo, CliToolStatus, CliConflict } from "./types";

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

// ─── CLI 工具环境（Claude Code / Codex）─────────────────────
// 后端 spawn 检测 / 安装 / 升级 / 冲突诊断（抄 install_uv 后端 spawn 模式，零 capability 改动）。

export const cliEnvApi = {
  /** 检查 claude / codex 版本 + 路径 + 状态（installed / broken / conflict）。 */
  checkVersions: () => invoke<CliToolStatus[]>("cli_check_versions"),
  /** 检查更新可用性（latest version + has_update）。 */
  checkUpdates: () => invoke<CliToolStatus[]>("cli_check_updates"),
  /** 安装（claude POSIX 走 native installer + npm 兜底；codex 走 npm）。 */
  install: (tool: "claude" | "codex") => invoke<void>("cli_install", { tool }),
  /** 升级（claude 走 `claude update` + npm 兜底；codex 走 uninstall + install 自愈）。 */
  upgrade: (tool: "claude" | "codex") => invoke<void>("cli_upgrade", { tool }),
  /** 诊断冲突：`which -a` / `where` 枚举 + canonicalize 去重 + source 推断。 */
  diagnoseConflicts: () => invoke<CliConflict[]>("cli_diagnose_conflicts"),
};

// ─── Auto-update toggle (gates 启动 daily check；手动按钮不 gate) ──────────

export const autoUpdateApi = {
  /** 读 auto_update_enabled；缺失默认 true（不打扰存量用户）。 */
  get: () => invoke<boolean>("get_auto_update_enabled"),
  /** 持久化 auto_update_enabled（settings KV scope=app key=auto_update_enabled）。 */
  set: (enabled: boolean) => invoke<void>("set_auto_update_enabled", { enabled }),
};

