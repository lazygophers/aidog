// profile.ts — agent 配置方案（Profile）快照：保存 / 一键切回 / 列表 / 删除（magpie 票 06）。
// 类型与 Rust `ai_tools_cmd::profile` 的 serde 形状逐字对齐。

import { invoke } from "../transport";

/** 单文件差异 / 应用结果（与 Rust ProfileDiffItem 对齐）。 */
export interface ProfileDiffItem {
  /** 文件 key 或 `db:groups`。 */
  target: string;
  /** create / update / delete。 */
  op: "create" | "update" | "delete";
  /** 人读说明。 */
  detail: string;
  /** 执行阶段单面失败原因（best-effort）。 */
  error?: string;
}

export interface ProfileMeta {
  name: string;
  /** Unix 秒。 */
  created_at: number;
}

export const profileApi = {
  /** 保存当前 agent 配置 + 分组映射为方案；同名已存在须 overwrite=true。 */
  save: (name: string, overwrite: boolean) =>
    invoke<ProfileMeta>("profile_save", { name, overwrite }),
  /** 一键切回方案；dry_run=true 只返回差异清单不落盘。 */
  apply: (name: string, dryRun: boolean) =>
    invoke<ProfileDiffItem[]>("profile_apply", { name, dryRun }),
  list: () => invoke<ProfileMeta[]>("profile_list"),
  /** 只删快照文件，不碰当前配置。 */
  remove: (name: string) => invoke<void>("profile_delete", { name }),
};
