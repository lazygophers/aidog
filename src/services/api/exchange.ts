// exchange.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import type { ImportExportScope, ConflictDecision, ImportPreview, ImportReport, CcswitchDetection, CcswitchReadResult, Sub2ApiReadResult, BackupSettings, BackupResult } from "./types";

export const importExportApi = {
  /**
   * 导出勾选范围到用户选择的文件。
   * @param selection 逐项白名单（[scope, key] 对列表）；省略/null = 导出全部（向后兼容）。
   */
  exportToFile: (scopes: ImportExportScope[], path: string, selection?: [string, string][] | null) =>
    invoke<void>("export_to_file", { scopes, path, selection: selection ?? null }),
  /** 导出前预览：collect 全量 → 列出可勾选条目（conflicts 恒空）。 */
  exportPreview: (scopes: ImportExportScope[]) =>
    invoke<ImportPreview>("export_preview", { scopes }),
  /** 读文件 → 解密 → 冲突预览。 */
  readPreview: (path: string) =>
    invoke<ImportPreview>("import_read_file", { path }),
  /**
   * 按决策应用导入。
   * @param selection 选中条目白名单（[scope, key] 对列表）；省略 = 导入全部（旧行为）。
   */
  apply: (path: string, decisions: ConflictDecision[], selection?: [string, string][]) =>
    invoke<ImportReport>("import_apply", { path, decisions, selection }),
};

// ─── cc-switch 导入（异源单向，仅 claude + codex provider）───

/** codex provider config.toml 解析后字段（后端已解析，前端直接消费）。 */


export const ccswitchApi = {
  /** 探测 cc-switch 配置存在性 + 路径。 */
  detect: (overridePath?: string) =>
    invoke<CcswitchDetection>("ccswitch_detect", { overridePath }),
  /** 读取 providers（仅 claude + codex）。 */
  read: (path?: string) =>
    invoke<CcswitchReadResult>("ccswitch_read", { path }),
  /**
   * 接收前端转换好的 Platform JSON + 决策，走 apply::apply 写入。
   * autoGroup=true 时导入后建/加入 `cc-switch` 分组（toggle 默认开）。
   */
  import: (platformPayload: unknown[], decisions: ConflictDecision[], autoGroup: boolean) =>
    invoke<ImportReport>("ccswitch_import", { platformPayload, decisions, autoGroup }),
};

/** sub2api 账号 DTO（后端解析结果，camelCase）。 */


export const sub2apiApi = {
  /** 解析用户提供的 sub2api-data JSON 文本，返回账号 DTO 列表。 */
  parse: (jsonText: string) =>
    invoke<Sub2ApiReadResult>("sub2api_parse", { jsonText }),
  /** 后端读取用户选择的 JSON 文件文本（避开前端 fs scope 限制）。 */
  readFile: (path: string) =>
    invoke<string>("sub2api_read_file", { path }),
  /**
   * 接收前端转换好的 Platform JSON + 决策，走 apply::apply 写入。
   * autoGroup=true 时导入后建/加入 `sub2api` 分组（toggle 默认开）。
   */
  import: (platformPayload: unknown[], decisions: ConflictDecision[], autoGroup: boolean) =>
    invoke<ImportReport>("sub2api_import", { platformPayload, decisions, autoGroup }),
};

// ─── 定时备份 ───────────────────────────────────────────────

/** 定时备份设置（字段 snake_case，与后端 BackupSettings 对齐）。 */


export const backupApi = {
  /** 读取定时备份设置（缺省/解析失败 → 后端默认）。 */
  get: () => invoke<BackupSettings>("backup_settings_get"),
  /** 写入设置（后端会 clamp 非法值，返回规范化后的值）。 */
  set: (settings: BackupSettings) =>
    invoke<BackupSettings>("backup_settings_set", { settings }),
  /** 立即触发一次备份（忽略 throttle）。 */
  runNow: () => invoke<BackupResult>("backup_run_now"),
};

// ─── About / 版本信息 ───────────────────────────────────────

/** 关于页版本信息（字段 snake_case，与后端 AboutInfo 对齐）。 */

