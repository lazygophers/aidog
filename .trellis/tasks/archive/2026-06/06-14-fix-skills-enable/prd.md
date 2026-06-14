# 修复 skills enable 失败 (path 替代锁文件 source + 前端弹错)

## Goal

修复 Skills 页点击「启用」对锁文件未记录的 skill 无效的 bug：enable 改用 skill 的 `installed_path` 作为 `npx skills add` 的 package（绕过锁文件 source 依赖），并让 enable/disable 失败在前端可见。

## Bug 现象 / 根因（已定位+验证）

- 现象：点击 skill `aihot` 启用，刷新后仍未启用；日志只见 `skills_enable command invoked name=aihot`，无可见错误。
- 根因 1：`aihot` 是 OpenCode 独有 skill（`~/.config/opencode/skills/aihot`，`agents:["OpenCode"]`），**非 npx skills 装的 → 锁文件 `.agents/.skill-lock.json` 无该条目**。
- `enable()` 现逻辑（skills.rs:436+）：`read_skill_source(name)` 从锁文件取 source，缺失 → 直接返回 `success:false, stderr:"cannot resolve source..."`。→ 对所有锁文件未记录的 skill（OpenCode/Gemini 等其他 agent 独有、手动放置、第三方 symlink 如 .cc-switch）enable 全失败。
- 根因 2：后端返回了 `success:false`，但**前端没把错误弹给用户**（用户只看到"还是未启用"）。

## 已验证的修复机制

- `npx skills add <skill 的本地路径> --agent <slug> -g -y` **可行**（实测 aihot → claude-code 成功，copy 到 `~/.claude/skills/aihot`，agents 变 `["Claude Code","OpenCode"]`）。
- skill 的 path 在 `npx skills list --json` 每条都有（`path` 字段，已映射到 SkillInfo.installed_path）。→ 用 path 作 add package 对**所有** skill 通用，不依赖锁文件。
- path 方式用本地已有副本（无网络、内容一致），优于 source 重新 GitHub 拉取。

## Requirements

### R1 — enable 改用 path 作 add package
- `skills_enable` 入参增加 skill 的 `path`（前端从 SkillInfo.installed_path 传入），或后端调 `npx skills list --json` 查该 skill path。**推荐前端传 path**（已有数据，省一次 npx 调用）。
- `enable()` 命令改为：`npx skills add <path> -a <slug> [-g] -y`（去掉 `-s <name>`，实测单 skill 目录 add 无需 -s）。
- **删除** `read_skill_source` / 锁文件 source 依赖（或保留 source 仅作展示，但 enable 不再用它）。
- path 为空/无效 → 返回明确错误。

### R2 — 前端弹出 enable/disable 错误
- toggle 点击后，若 `SkillsOpResult.success === false`，前端**显示错误消息**（toast/消息条，含 stderr 摘要），不静默。
- 成功也给轻反馈（可选）+ 刷新列表。

## Acceptance Criteria

- [ ] 点击启用锁文件未记录的 skill（如 aihot）能成功启用，刷新后显示已启用
- [ ] enable 用 `npx skills add <path> -a <slug>`，不再依赖锁文件 source
- [ ] enable/disable 失败时前端显示错误消息
- [ ] 契约同步：skills_enable 签名加 path（前后端一致）
- [ ] cargo clippy 无新 warning + cargo test 绿（更新 enable_args 测试为 path 形式）；yarn build 绿；check-i18n 零缺失

## Definition of Done

- cargo clippy 无 warning + cargo test 绿；yarn build 绿；check-i18n 零缺失
- 改动落 worktree，闭环 check→commit(merge)→archive
- 更新 [[skills-management-module]] / [[npx-skills-cli]]（enable 改 path 机制）

## Technical Approach

- 后端 `gateway/skills.rs`：`enable(name, path, agent, scope)` → `enable_args` 改 `["add", <path>, "-a", slug, (-g), "-y"]`。删 `read_skill_source` + 相关测试，改 `enable_args` 测试断言 path 形式。`SkillInfo.source` 可保留（展示用）或一并删。lib.rs `skills_enable` 加 path 参。
- 前端 `pages/Skills.tsx`：toggle enable 调用传 `skill.installed_path`；enable/disable 后判 success，失败 setMessage(错误)。api.ts `skillsApi.enable` 签名加 path。
- ⚠️ 实施安全：worktree 内**禁对用户真实 skill 跑写操作**测试；正确性靠 `enable_args` 单测（断言 path 形式 args）+ 代码审查。修复机制已由 coordinator 实机验证（aihot 成功）。

## Out of Scope

- disable 逻辑（`remove -s <name> -a <slug>` 不变，已正常）
- 锁文件 source 的其他用途
- 装新 skill / catalog

## Technical Notes

- 现状：`enable()` skills.rs:436；`read_skill_source` :239；`enable_args` :406；`SkillInfo.installed_path`(list path)
- 实测命令：`npx skills add /Users/luoxin/.config/opencode/skills/aihot --agent claude-code -g -y` → 成功 copy 到 ~/.claude/skills/aihot
- list --json path 字段 = skill 本地路径
- 参考 [[skills-management-module]] / [[npx-skills-cli]]
