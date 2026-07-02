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

