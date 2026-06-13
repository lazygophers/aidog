# SubagentStatusLine name 段 fallback 对齐

## Goal
DEFAULT_SUBAGENT_SEGMENTS 第二段(name, `sa-name`)的 jq expr 从 `.agent.name // .session_name // "subagent"` 改为 `.label // .name // .id // "?"`，对齐 ccplugin `subagent_statusline.py` render_row(228) 的 name 兜底链。

## Requirements
- R1 sa-name expr = `.label // .name // .id // "?"`。

## Acceptance Criteria
- [ ] yarn build 通过。
- [ ] 子代理生成脚本 name 段按 label→name→id→"?" 兜底（与 D2 per-task 归一化兼容）。

## Definition of Done
- 变更提交 + worktree 合并归档。

## Out of Scope
- 其余段不变。

## Technical Notes
- 单行改 src/components/settings/editors.tsx DEFAULT_SUBAGENT_SEGMENTS sa-name。
- 接 06-13-subagent-statusline-dynamic(已归档) 的补充。
