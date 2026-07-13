// types/part4.ts — 类型分片 4/4（arch-redesign），纯移动。
// 由 types.ts barrel 统一 re-export；外部应 `import type { X } from "../types"`，
// 不直接 import 本文件（分片边界为实现细节）。

import type { Protocol, Platform } from "./part1";

// ─── CPA(CLIProxyAPI) 配置导入 ─────────────────────────────
// 镜像 Rust `commands_platform/src/cpa_import.rs` + `aidog_core/src/gateway/cpa_import/mapper.rs`
// serde 字段名一一对应（snake_case）。MappedPlatform 同时是 apply 的输入。

/** 映射后的 aidog 平台（cpa_import_parse 输出 / cpa_import_apply 输入）。 */
export interface MappedPlatform {
  /** 映射后的协议（platform_type），含 4 cpa-* 变体 */
  protocol: Protocol;
  /** 平台名称（openai-compat 从 name；OAuth 从 email；api-key 段由 protocol + host 派生） */
  name: string;
  /** 上游 base URL（OAuth 段可能为空，前端预览回填） */
  base_url: string;
  /** API key（OAuth = access_token；预览由前端掩码展示） */
  api_key: string;
  /** 可用模型列表（来自 cpa models[].name，alias 丢） */
  models: string[];
  /** 序列化后的 extra JSON（含 prefix/headers/cpa 源信息） */
  extra: string;
  /** 是否禁用（来自 CPA `disabled=true`，apply 时 post-create 置 status=disabled） */
  disabled: boolean;
  /** 来源标签（UI 展示用，如 "openai-compatibility / glm"） */
  source_label: string;
}

/** 解析时被跳过的文件（rar/7z、解析失败、无 cpa 段等）。 */
export interface CpaSkipReason {
  /** 文件路径 */
  path: string;
  /** 跳过原因类型 */
  reason: string;
}

/** cpa_import_parse 返回（providers 已是 MappedPlatform，纯读，不建平台）。 */
export interface CpaImportParseResult {
  platforms: MappedPlatform[];
  skipped: CpaSkipReason[];
  source_files: string[];
}

/** apply 失败项（非原子：成功的入库，失败的收集原因）。 */
export interface CpaBatchFailure {
  name: string;
  error: string;
}

/** cpa_import_apply 返回：created 入库平台列表 + failed 失败原因列表。 */
export interface CpaBatchReport {
  created: Platform[];
  failed: CpaBatchFailure[];
}

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

