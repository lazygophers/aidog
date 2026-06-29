# Trellis 任务看板

> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。
> 已完成任务归档于 `.trellis/tasks/`，历史可查 git log；本表只列当前活跃任务。

| ID | 名称 | 描述 | 状态 | 阶段 | 进度 | worktree |
| --- | --- | --- | --- | --- | --- | --- |
| 06-20-test-coverage-80 | 单测覆盖率≥80% | 完善整体单元测试覆盖率至少80% | 规划中 | 规划 | 0% | — |
| 06-26-skills-install-ux-recovery | SkillInstallView 改动恢复 (worktree 未 commit 丢失) | — | 进行中 | 实施 | — | — |
| proxy-models-404 | fix GET /proxy/models 返回 404 | — | 已完成 | 完成 | 100% | /Users/luoxin/persons/lyxamour/aidog/.worktrees/proxy-models-404 |

## Worktree ↔ Task 映射

> 每个活跃 worktree 登记映射到的 task (一对多: 同 task 拆多 subagent 各占一行);
> 无映射的 worktree 由 WorktreeCreate hook 提醒补登。

| worktree | task | 创建源 |
| --- | --- | --- |
