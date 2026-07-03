// skills.ts — 从 services/api.ts 拆出（arch-redesign）；纯移动，零逻辑变更。

import { invoke } from "@tauri-apps/api/core";
import type { SkillAgent, SkillScope, SkillsEnv, CatalogEntry, SkillsOpResult, SkillDetail, SkillFileContent, CachedSkills } from "./types";

export const skillsApi = {
  /** 探测 npx / node 环境。 */
  checkEnv: () => invoke<SkillsEnv>("skills_check_env"),
  /** 浏览 catalog（skills.sh HTTP 端点当前 404，恒返回空；前端用 search）。 */
  browseCatalog: () => invoke<CatalogEntry[]>("skills_browse_catalog"),
  /** 搜索 catalog（`npx skills find <kw>`，id = `owner/repo@skill`）。 */
  search: (keyword: string) =>
    invoke<CatalogEntry[]>("skills_search", { keyword }),
  /**
   * 从 catalog 安装 skill 到多个 agent（`npx skills add <id> -a <slug> [-g] -y`）。
   * id = CatalogEntry.id（`owner/repo@skill`，含子 skill 选取，无需 -s）。
   */
  install: (id: string, agents: SkillAgent[], scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_install", { id, agents, scope }),
  /** 列已装 skill 目录文件树（详情视图，只读）。 */
  detail: (installedPath: string) =>
    invoke<SkillDetail>("skill_detail", { installedPath }),
  /** 读 skill 内单文件（只读，带路径遍历防护）。 */
  readFile: (installedPath: string, rel: string) =>
    invoke<SkillFileContent>("skill_read_file", { installedPath, rel }),
  /**
   * 列指定 scope 下已装 skills —— **立即返回缓存**（命中即 0 子进程）。
   * 冷启动返回 `{ items: [], stale: true }`，调用方据此显加载态 + 触发 refresh。
   */
  listInstalled: (scope: SkillScope) =>
    invoke<CachedSkills>("skills_list_installed", { scope }),
  /** 强制跑 npx 刷新缓存并返回 fresh（SWR revalidate 半）。 */
  listRefresh: (scope: SkillScope) =>
    invoke<CachedSkills>("skills_list_refresh", { scope }),
  /** 为某 agent 启用 skill（npx add，用 skill 本地 path 作 add package）。 */
  enable: (name: string, path: string, agent: SkillAgent, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_enable", { name, path, agent, scope }),
  /** 为某 agent 关闭 skill（npx remove）。 */
  disable: (name: string, agent: SkillAgent, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_disable", { name, agent, scope }),
  /** 更新已装 skills。 */
  update: (scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_update", { scope }),
  /** 一键卸载当前 scope 所有平台所有 skills（破坏性）。 */
  uninstallAll: (scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_uninstall_all", { scope }),
  /** 卸载单一 skill（破坏性）：删规范存储 + 所有 agent 启用配置。 */
  uninstall: (name: string, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_uninstall", { name, scope }),
  /** 对齐两 agent 的 skills 启用配置（使 to 与 from 完全一致）。 */
  alignAgents: (from: SkillAgent, to: SkillAgent, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_align_agents", { from, to, scope }),
  /** 为某 agent 启用当前 scope 全部已装 skills（只增不减）。 */
  enableAll: (agent: SkillAgent, scope: SkillScope) =>
    invoke<SkillsOpResult>("skills_enable_all", { agent, scope }),
};

// ─── MCP 管理 ─────────────────────────────────────────────

/** 受管 agent slug（对齐后端 mcp.rs::McpAgent，注意 claude-code 非 "claude"）。 */

