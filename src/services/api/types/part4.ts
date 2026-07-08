// types/part4.ts — 类型分片 4/4（arch-redesign），纯移动。
// 由 types.ts barrel 统一 re-export；外部应 `import type { X } from "../types"`，
// 不直接 import 本文件（分片边界为实现细节）。

export interface Sub2ApiAccount {
  name: string;
  /** sub2api 原始 platform 值（小写），前端做 Protocol 映射。 */
  platform: string;
  apiKey?: string;
  baseUrl?: string;
}


export interface Sub2ApiReadResult {
  accounts: Sub2ApiAccount[];
}


export interface BackupSettings {
  enabled: boolean;
  /** 间隔小时，≥1。 */
  interval_hours: number;
  /** 保留天数，1..=90。 */
  retention_days: number;
  /** 上次成功备份 epoch 毫秒（0=从未），后端写。 */
  last_backup_at: number;
  /** 上次错误信息（空=成功），后端写。 */
  last_backup_error: string;
}

/** 立即备份结果。 */


export interface BackupResult {
  ok: boolean;
  path?: string;
  error?: string;
  timestamp: number;
}


export interface AboutInfo {
  app_version: string;
  tauri_version: string;
  os: string;
  arch: string;
  family: string;
  profile: string;
  /** git 短 commit（无 git 时 "unknown"）。 */
  git_commit: string;
  /** 构建时间 epoch 秒字符串（前端格式化）。 */
  build_time: string;
}

// ─── CLI 工具环境（Claude Code / Codex）─────────────────────
// 与后端 `commands::cli_env` 数据结构对齐，snake_case。

/** 单处安装（`which -a` / `where` 枚举 + canonicalize 去重 + source 推断）。 */
export interface CliInstallation {
  path: string;
  version: string | null;
  runnable: boolean;
  /** 安装来源：nvm / homebrew / volta / fnm / mise / bun / pnpm / scoop / pip / native / npm-global / system。 */
  source: string;
  /** 是否为 PATH 默认命中的那处（`which` / `where` 第一行）。 */
  is_path_default: boolean;
}

/** 工具状态（claude / codex）。 */
export interface CliToolStatus {
  name: string;
  installed: boolean;
  version: string | null;
  path: string | null;
  /** 装了但 `--version` 跑不起来（平台二进制损坏等）。 */
  broken: boolean;
  /** 多处安装且版本分歧或运行态混合（严阈值）。 */
  conflict: boolean;
  /** npm registry 最新版本（检测失败/离线时为 undefined）。 */
  latest_version?: string;
  /** 是否有更新可用（undefined=检测失败/离线，true=有更新，false=已是最新）。 */
  has_update?: boolean;
}

/** 冲突诊断结果。 */
export interface CliConflict {
  tool: string;
  installations: CliInstallation[];
  is_conflicting: boolean;
  /** 仅报告 + 建议，不自动卸载（破坏性操作禁主动执行）。 */
  suggestion: string;
}

