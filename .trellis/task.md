# Trellis 任务看板

> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。

| ID | 名称 | 描述 | 状态 | worktree |
| --- | --- | --- | --- | --- |
| 06-20-test-coverage-80 | 单测覆盖率≥80% | 完善整体单元测试覆盖率至少80% | 规划中 | — |
| 06-30-group-env-vars | 分组配置支持环境变量设置 | 分组维度支持自定义环境变量注入 | 实施中 | /Users/luoxin/persons/lyxamour/aidog/.worktrees/06-30-group-env-vars |
| 06-30-platform-402-autodisable-error-status | 402上游自动禁用免purge + proxy错误记入平台状态 | — | 实施中 | /Users/luoxin/persons/lyxamour/aidog/.worktrees/06-30-platform-402-autodisable-error-status |

## Worktree ↔ Task 映射

> 每个活跃 worktree 登记映射到的 task (一对多: 同 task 拆多 subagent 各占一行);
> 无映射的 worktree 由 WorktreeCreate hook 提醒补登。

| worktree | task | 创建源 |
| --- | --- | --- |
| /Users/luoxin/persons/lyxamour/aidog/.worktrees/06-30-group-env-vars | 06-30-group-env-vars | trellisx-start |
| platform-402-autodisable-error-status | 402上游自动禁用免purge + proxy错误记入平台状态 | — | 已完成 | — |
