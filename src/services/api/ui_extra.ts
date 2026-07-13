// ui_extra.ts — UI 态持久化 invoke 封装。
// 对应 Rust `commands_platform::ui_extra::set_ui_extra`（读改写 platform/group extra JSON 单键）。
// key 推荐 `_ui_` 前缀（与业务键 peak_hours/breaker/...共存无副作用；导出时 strip）。

import { invoke } from "@tauri-apps/api/core";

/** extra 写入目标表。group 待 s7 schema migration 后开放。 */
export type UiExtraTarget = "group" | "platform";

/** 现有 UI 态键（bool）。新增键沿用 `_ui_` 前缀并在此 union 扩展。 */
export type UiExtraKey =
  | "_ui_collapsed"
  | "_ui_expand_plat"
  | "_ui_expand_grp";

/** 写 UI 态单键到 extra JSON（读改写，保留其余键）。
 *  - target="platform": 现支持；target="group": 待 s7 group 表加 extra 列后开放。
 *  - value 经 serde_json::Value 透传，现键全为 bool。 */
export function setUiExtra(
  target: UiExtraTarget,
  id: number,
  key: UiExtraKey,
  value: boolean,
): Promise<void> {
  return invoke<void>("set_ui_extra", { target, id, key, value });
}
