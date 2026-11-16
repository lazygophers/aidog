// subagents.ts — 扩展 → 子代理（Claude Code / Codex agents 目录写出）。

import { invoke } from "../transport";

export type SubagentAgent = "claude-code" | "codex";
/** on = aidog 写出且未改；modified = 用户改过（不删）；conflict = 同名外来文件（不覆盖）。 */
export type SubagentFileStatus = "on" | "off" | "modified" | "conflict";

export interface Subagent {
  name: string;
  description: string;
  prompt: string;
  agents: SubagentAgent[];
  codex_model?: string;
  from?: string;
  from_hash?: string;
  written?: Record<string, string>;
}

export interface SubagentView extends Subagent {
  status: Record<SubagentAgent, SubagentFileStatus>;
}

export interface SubagentImportReport {
  added: string[];
  updated: string[];
  have: string[];
  edited: string[];
  /** 同源内被浅层同名压掉的深层文件（相对路径）。 */
  conflicts: string[];
}

export interface SubagentImportCandidate {
  rel: string;
  name: string;
  description: string;
  prompt: string;
}

export const subagentsApi = {
  list: () => invoke<SubagentView[]>("subagents_list"),
  /** oldName 空 = 新建；否则编辑（改名先撤回旧名文件）。 */
  save: (oldName: string | null, subagent: Subagent) =>
    invoke<SubagentView[]>("subagents_save", { oldName, subagent }),
  /** 返回因被用户修改而保留的文件路径。 */
  delete: (name: string) => invoke<string[]>("subagents_delete", { name }),
  /** 导入前预扫（只读），返回候选清单。 */
  preview: (source: string) => invoke<SubagentImportCandidate[]>("subagents_import_preview", { source }),
  /** source = 本地文件夹或 git 地址；selected = 只导入这些相对路径，缺省全部。 */
  import: (source: string, agents: SubagentAgent[], selected?: string[]) =>
    invoke<SubagentImportReport>("subagents_import", { source, agents, selected }),
};
