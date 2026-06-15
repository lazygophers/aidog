# subagent statusline 显示问题修复

## Goal

用户报 subagent statusline 行渲染为 Claude Code 兜底格式：
```
◯ feature-dev:code-architect  S3 tokenusage package implementation   19s
```
（来自 cortex `subagent-statusline-native`：「子代理行变 `◯ <type> <desc> <dur>` 无徽章」= Claude Code 脚本零输出/失败时的默认渲染）。

诊断结果：`~/.aidog/scripts/aidog-subagent-statusline.py` **内容是主 statusline 脚本**（末尾 `main()` 调 `render(payload, ROWS, gi)` + 含主 ROWS base64），不是 subagent 版本（应调 `render_subagent` + 每任务 JSONL）。送 subagent stdin 给主 render → 输出非 JSON 文本 → Claude Code 解析失败回退默认渲染。

## 根因（已定位）

`src/components/settings/editors.tsx::StatuslineEditor` L1967：
```ts
const scriptPreview = generateStatusLineScript(segments);
```
**写死调主脚本生成器**，不区分 `scriptType`。后续 `handleSave` (L1975) 和 auto-sync effect (L1998) 都用这个 `scriptPreview` 通过 `statuslineApi.generate(scriptType, scriptPreview)` 写盘——`scriptType="subagent"` 时**写到 subagent 文件但内容是主脚本**。

后端 `generate_statusline_script` (lib.rs L2251) 仅按 `scriptType` 选文件名，不验证 content；纯透写。

## Decision (ADR-lite)

- 改 editors.tsx L1967：按 `scriptType` 分流调对应生成器：`scriptType === "subagent" ? generateSubagentStatusLineScript(segments) : generateStatusLineScript(segments)`。
- 导入 `generateSubagentStatusLineScript` from `./statusline-gen`。
- 不动后端（透写正确）。
- 用户保存设置 / 触发 auto-sync effect 后，subagent 脚本即被正确覆写。

## Requirements

- editors.tsx StatuslineEditor scriptPreview 按 scriptType 分流。
- import 补 `generateSubagentStatusLineScript`。
- 验证：保存 subagent statusline → 重读 `~/.aidog/scripts/aidog-subagent-statusline.py` → 末尾应是 `for line in render_subagent(payload, SEGS, now)` + `SEGS` base64。
- yarn build / check-i18n / cargo (no rust change) 全绿。

## Acceptance Criteria

- [ ] editors.tsx 同一处 `scriptPreview` 按 scriptType 分流，导入 generateSubagentStatusLineScript。
- [ ] 重新保存或触发 auto-sync 后，subagent 脚本末尾调 `render_subagent`（不是 `render`）。
- [ ] 模拟 subagent stdin → 脚本输出每行 `{"id","content"}` JSON（不是裸 ANSI）。
- [ ] yarn build 成功，check-i18n 零缺失。

## Definition of Done

- worktree commit + merge + archive
- cortex memory 更新 [[subagent-statusline-native]] 加 bug 修复记录

## Out of Scope

- 后端 generate_statusline_script 加 content 校验（防御性，本次不做）
- statusline 渲染引擎变更
- 用户已落盘错脚本的迁移命令（保存触发自动覆写即可）

## Files

- `src/components/settings/editors.tsx` — scriptPreview 分流 + import
