// types/part5.ts — 类型分片 5/5（cpa-standalone-module s5），cli-proxy 子系统。
// 由 types.ts barrel 统一 re-export；外部应 `import type { X } from "../types"`。
// 镜像 Rust `aidog_core::gateway::models::cli_proxy.rs` + `commands_cli_proxy::import.rs`
// serde 字段名一一对应（snake_case）。

/** CLI 代理 provider 主行（对应 Rust `CliProxyProvider`）。 */
export interface CliProxyProvider {
  id: number;
  name: string;
  /** 入站协议标识（anthropic/openai/glm_coding 等，对应 Protocol serde 形式） */
  wire_protocol: string;
  base_url: string;
  api_key: string;
  /** 模型列表（DB 存 JSON 数组字符串；Rust 序列化时已 parse 为数组） */
  models: string[];
  /** 原始 JSON 串（空串视作 "{}"，仿 platform.extra） */
  extra: string;
  /** active / disabled */
  status: string;
  /** 归属分组 id；null = 未分配 */
  group_id?: number | null;
  created_at: number;
  updated_at: number;
}

/** 创建/更新入参（全量覆写，对齐 Rust `CreateCliProxyProvider` / `UpdateCliProxyProvider`）。 */
export interface CreateCliProxyProvider {
  name: string;
  wire_protocol: string;
  base_url: string;
  api_key?: string;
  models?: string[];
  extra?: string;
  status?: string;
  group_id?: number | null;
}

/** `cli_proxy_import` 单条失败原因（非原子：成功入库，失败收集）。 */
export interface CliProxyImportFailure {
  name: string;
  error: string;
}

/** `cli_proxy_import` 跳过项（rar/7z、解析失败、无 cpa 段等）。 */
export interface CliProxyImportSkipReason {
  path: string;
  reason: string;
}

/** `cli_proxy_import` 返回。 */
export interface CliProxyImportResult {
  created: CliProxyProvider[];
  failed: CliProxyImportFailure[];
  skipped: CliProxyImportSkipReason[];
  source_files: string[];
}
