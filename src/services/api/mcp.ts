// mcp.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import type { McpAgentSlug, McpServerInfo, McpScanItem, McpImportPayload, McpImportReport, McpUpdatePayload } from "./types";

export const mcpApi = {
  /** 列出 DB 中所有 MCP（env/headers 脱敏）。 */
  list: () => invoke<McpServerInfo[]>("mcp_list"),
  /** 扫描 Claude Code + Codex 配置去重合并。 */
  scan: () => invoke<McpScanItem[]>("mcp_scan"),
  /** 批量导入（enabled = source agent）。 */
  import: (items: McpImportPayload[]) =>
    invoke<McpImportReport>("mcp_import", { items }),
  /** 粘贴 JSON 导入（claude.json 协议；enabled 空，同名跳过）。 */
  importJson: (json: string) =>
    invoke<McpImportReport>("mcp_import_json", { json }),
  /** per-agent 启用/禁用（同步写/删 agent 配置）。 */
  setAgent: (name: string, agent: McpAgentSlug, enabled: boolean) =>
    invoke<void>("mcp_set_agent", { name, agent, enabled }),
  /** 编辑（全字段 + 改名 + transport 切换，同步 agent 配置）。 */
  update: (oldName: string, payload: McpUpdatePayload) =>
    invoke<McpServerInfo>("mcp_update", { oldName, payload }),
  /** 手动添加（enabled 空，不写 agent 配置；后续 setAgent 启用）。 */
  add: (payload: McpUpdatePayload) =>
    invoke<McpServerInfo>("mcp_add", { payload }),
  /** 删除（DB + 所有 enabled agent 配置，破坏性）。 */
  delete: (name: string) => invoke<void>("mcp_delete", { name }),
  /** 重新同步全部：从 DB 全量重写所有 enabled agent 配置（修复外部污染如 env:null）。 */
  resync: () => invoke<number>("mcp_resync"),
  /** 导出单 MCP 可分享对象（claude.json 协议 {mcpServers:{name:entry}}，明文 env/headers）。 */
  shareExport: (name: string) =>
    invoke<Record<string, unknown>>("mcp_share_export", { name }),
};

// ─── 导入导出子系统 ───────────────────────────────────────

