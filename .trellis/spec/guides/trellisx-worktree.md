---
updated: 2026-06-11
rewrite-version: 1
authored-by: trellisx-spec
mode: optimize
---

# trellisx worktree + subtask 约定

何时被读: trellis task 实施时 (sub-agent dispatch 注入)
谁读: main / 执行者 agent
不遵守的代价: worktree 污染主工作区 / subtask 无隔离 / 串行派降低提效

> **本文件是 worktree 隔离 + subtask 异步并行的单一事实源。** trellisx-conventions.md 只引用本文件, 禁重复其条款。

## worktree 隔离

- task.py create/start 后由平台 hook (.claude/hooks/trellisx-worktree.py) 自动建 worktree 于 `.trellis/worktrees/<task>`, branch `trellisx-<task>`
- 全部源码改动**必须**落 worktree 内, 主工作区保持干净
- main 可直接写源码 (trellis inline), 但目标路径必须在 worktree (写绝对路径或 EnterWorktree 切入)
- 复杂 / 并行 subtask → 派 sub-agent (isolation:worktree) 或 agent-team 成员
- task archive 时 worktree 干净 → hook 自动销毁 (worktree remove + branch -D); 脏 → 警告先合并

## subtask 拆分 + 异步并行

- task 拆 >= 2 subtask, 每 subtask 独立文件 `.trellis/tasks/<task>/subtask/<id>-<slug>.md`
- PRD mermaid 调度图显式标并行组 (无依赖 subtask 同批同时跑), 有依赖标依赖箭头
- 拆分目标 = 最大化可并行 subtask 数, 缩短关键路径
- 执行硬规: 无依赖 subtask **同一条回复一次性发多个 sub-agent 调用** (Claude Code 同消息多 Agent tool = 真并行); **禁逐个串行派** (串行 = 各 subtask 耗时叠加)
- 有依赖 subtask 等上游 done 再派下游; 收到各 agent 返回立即回传用户进度
- parent-child 用 trellis 原生 `task.py add-subtask`
