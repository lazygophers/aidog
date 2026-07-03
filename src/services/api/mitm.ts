// mitm.ts — MITM (P3) 假 CA + 白名单配置 API。
// sync master 07-03-proxy-relay-mitm：从 monolithic api.ts 迁入 arch 新结构。

import { invoke } from "@tauri-apps/api/core";

// ─── MITM (P3) — 假 CA + 白名单配置 ───────────────────────

/** 白名单行（与后端 WhitelistEntryDto 对齐，snake_case）。 */
export interface WhitelistEntry {
  host_pattern: string;
  enabled: boolean;
  /** "default"（系统预填）/ "user"（用户加）。 */
  source: "default" | "user";
  /** 规则类型（与后端 rule_type 对齐）：domain / suffix / keyword / ipcidr。 */
  rule_type: "domain" | "suffix" | "keyword" | "ipcidr";
}

/** MITM 综合状态（与后端 MitmStatus 对齐）。 */
export interface MitmStatus {
  enabled: boolean;
  /** CA 已生成（DB 有 mitm_ca 行）。 */
  ca_present: boolean;
  /** CA 已装到系统信任库（装命令 exit=0 后回写）。 */
  ca_installed: boolean;
  /** SHA-256 fingerprint（hex colon-separated，装/卸信任库定位用）。 */
  ca_fingerprint: string;
  whitelist: WhitelistEntry[];
}

/** CA 安装命令 spec（前端 `Command.create(name, args).execute()`）。 */
export interface CaCommandSpec {
  /** capability `mitm-ca.json` 的命名命令 key（按 OS）。 */
  name: string;
  /** 命令参数（含 ca_pem_path）。 */
  args: string[];
  /** 落盘后的 CA PEM 绝对路径（失败兜底手动装命令展示用）。 */
  ca_pem_path: string;
  /** 兜底手动装展示的真实 sudo 终端命令（提权失败时弹窗给用户复制执行）。 */
  manual_display: string;
}

/** CA 卸载命令 spec（ST9 用）。 */
export interface CaUninstallSpec {
  name: string;
  args: string[];
  /** 兜底手动卸展示的真实 sudo 终端命令。 */
  manual_display: string;
}

/**
 * CA 安装失败分类（与后端 `TrustErrorKind` enum 对齐，snake_case）。
 * 后端 `classify_trust_error` 真源，前端 invoke 后返 union string。
 */
export type TrustErrorKind = "cancel" | "auth_fail" | "no_agent" | "cmd_fail";

export const mitmApi = {
  /** 读 MITM 综合状态（CA + 白名单）。 */
  status: () => invoke<MitmStatus>("mitm_status"),
  /** 启用 MITM（D7：首次调 ensure_root_ca 生成 CA）。 */
  enable: () => invoke<void>("mitm_enable"),
  /** 禁用 MITM（保留 CA，仅 enabled=false）。 */
  disable: () => invoke<void>("mitm_disable"),
  /** 准备装信任库：写 ca.pem + 返命名命令 spec。 */
  installCaPrepare: () => invoke<CaCommandSpec>("mitm_install_ca_prepare"),
  /** 准备卸信任库（ST9 用）。 */
  uninstallCaPrepare: () => invoke<CaUninstallSpec>("mitm_uninstall_ca_prepare"),
  /** shell execute 完成后回写 CA 安装状态。 */
  setCaInstalled: (installed: boolean) =>
    invoke<void>("mitm_set_ca_installed", { installed }),
  /**
   * 分类 CA 安装失败原因（阶段 B 后端化真源）。
   * 入参 (name, code, stderr) 走后端 `classify_trust_error`（三 OS 分支纯函数 + None 兜底）。
   * `code` 显式 null 兜底（Tauri shell plugin reject/signal kill 路径 code 可能为 null）。
   */
  classifyTrustError: (name: string, code: number | null, stderr: string) =>
    invoke<TrustErrorKind>("mitm_classify_trust_error", { name, code, stderr }),
  /** 加白名单条目。 */
  whitelistAdd: (hostPattern: string) =>
    invoke<void>("mitm_whitelist_add", { input: { host_pattern: hostPattern } }),
  /** 删白名单条目。 */
  whitelistRemove: (hostPattern: string) =>
    invoke<void>("mitm_whitelist_remove", { hostPattern }),
  /** 切换白名单条目启用态。 */
  whitelistToggle: (hostPattern: string, enabled: boolean) =>
    invoke<void>("mitm_whitelist_toggle", { hostPattern, enabled }),
};
