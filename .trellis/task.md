# Trellis 任务看板

> 由 trellisx-workspace 维护 (经 trellisx-taskmd.py); task 生命周期节点后及时更新。

| ID | 名称 | 描述 | 状态 | worktree |
| --- | --- | --- | --- | --- |
| 06-20-test-coverage-80 | 单测覆盖率≥80% | 真实覆盖面全补: vitest 全量统计 + Rust 3 缺口分支 + 前端 39 test (pages/settings/platforms) | 规划中 | — |
| 06-30-group-env-vars | 分组配置支持环境变量设置 | 分组维度支持自定义环境变量注入 (sync 强写 ANTHROPIC_BASE_URL/AUTH_TOKEN 保护) | 已完成 | — |
| platform-last-error-msg | 平台最近错误展示提取error.message | DB 残留旧值 Migration 039 重提 + extract_error_message 已正确 | 已完成 | — |
| 06-30-export-ux-i18n | 导出 UX + i18n | 去「预览导出项」按钮改 debounce 自动展开 + 条目级展示 + setting label 本地化 (app:theme→主题) | 已完成 | — |
| 07-01-platform-429-no-autodisable | 429 不触发自动禁用 | 移除 429-配额从 auto_disable 触发条件 (non_success.rs:68) + spec C1/C3 修订 | 已完成 | — |
| 07-01-test-isolation-fix | 测试隔离治理 | 删真实环境 spawn + HomeGuard 收拢 4→1 + ENV_LOCK 集中 + grep lint 守卫 | 已完成 | — |
| 07-01-07-01-cli-integration-tab | CLI 集成 tab 改名 + 语言设置 | tab「编程工具」→「CLI 集成」(8 locale key+value) + 新增语言设置项 (复用 claude-settings language sync) | 实施中 | /Users/luoxin/persons/lyxamour/aidog/.worktrees/07-01-07-01-cli-integration-tab |
| 07-01-07-01-sensenova-platform | 商汤 SenseNova 平台支持 | 加商汤日日新平台 (Protocol/adapter/preset/粘贴识别/quota token plan) | 规划中 | — |
| 07-01-arch-redesign | 全仓架构重设计 | 分包分文件消大文件: 前端 4 巨型 (editors 4609/Platforms 3568/Groups 2195/api 2072) 拆 + 目录重组 + Rust 局部拆 | 规划中 (排队) | — |

## Worktree ↔ Task 映射

> 每个活跃 worktree 登记映射到的 task (一对多: 同 task 拆多 subagent 各占一行);
> 无映射的 worktree 由 WorktreeCreate hook 提醒补登。

| worktree | task | 创建源 |
| --- | --- | --- |
| /Users/luoxin/persons/lyxamour/aidog/.worktrees/07-01-07-01-cli-integration-tab | 07-01-07-01-cli-integration-tab | trellisx-start |
