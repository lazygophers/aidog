# Trellis 任务看板

> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。

| ID | 名称 | 描述 | 状态 | worktree |
| --- | --- | --- | --- | --- |
| 06-20-test-coverage-80 | 单测覆盖率≥80% | 完善整体单元测试覆盖率至少80% | 规划中 | — |
| 06-26-skills-install-ux-recovery | SkillInstallView 改动恢复 (worktree 未 commit 丢失) | — | 实施中 | — |
| 06-30-import-export-modules-ux | 导入导出补全模块覆盖+对齐菜单IA+导出逐项细粒度 | — | 已完成 | — |
| proxy-models-404 | fix GET /proxy/models 返回 404 | — | 已完成 | /Users/luoxin/persons/lyxamour/aidog/.worktrees/proxy-models-404 |

## Worktree ↔ Task 映射

> 每个活跃 worktree 登记映射到的 task (一对多: 同 task 拆多 subagent 各占一行);
> 无映射的 worktree 由 WorktreeCreate hook 提醒补登。

| worktree | task | 创建源 |
| --- | --- | --- |
