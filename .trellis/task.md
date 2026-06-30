# Trellis 任务看板

> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。

| ID | 名称 | 描述 | 状态 | worktree |
| --- | --- | --- | --- | --- |
| 06-20-test-coverage-80 | 单测覆盖率≥80% | 完善整体单元测试覆盖率至少80% | 规划中 | — |
| 06-30-group-env-vars | 分组配置支持环境变量设置 | 分组维度支持自定义环境变量注入 (sync 强写 ANTHROPIC_BASE_URL/AUTH_TOKEN 保护) | 已完成 | — |
| platform-last-error-msg | 平台最近错误展示提取error.message | DB 残留旧值 Migration 039 重提 + extract_error_message 已正确 | 已完成 | — |
| 06-30-export-ux-i18n | 导出 UX + i18n | 去「预览导出项」按钮改 debounce 自动展开 + 条目级展示 + setting label 本地化 (app:theme→主题) | 已完成 | — |
| 07-01-test-isolation-fix | 测试隔离治理 | 删真实环境 spawn + HomeGuard 收拢 4→1 + ENV_LOCK 集中 + grep lint 守卫 | 实施中 | /Users/luoxin/persons/lyxamour/aidog/.worktrees/07-01-test-isolation-fix |

## Worktree ↔ Task 映射

> 每个活跃 worktree 登记映射到的 task (一对多: 同 task 拆多 subagent 各占一行);
> 无映射的 worktree 由 WorktreeCreate hook 提醒补登。

| worktree | task | 创建源 |
| --- | --- | --- |
| /Users/luoxin/persons/lyxamour/aidog/.worktrees/07-01-test-isolation-fix | 07-01-test-isolation-fix | trellisx-start |
