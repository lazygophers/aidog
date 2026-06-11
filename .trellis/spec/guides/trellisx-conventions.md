---
updated: 2026-06-11
rewrite-version: 2
authored-by: trellisx-spec
mode: optimize
---

# trellisx 任务编排约定

何时被读: trellis task 实施 / 检查时 (sub-agent dispatch 注入)
谁读: main / trellis-implement / trellis-check / 任何执行者
不遵守的代价: worktree 污染主工作区 / task 无隔离 / 流程跳步

## worktree 隔离 + subtask 拆分 (单一事实源)

- worktree 隔离 + subtask 异步并行的**完整强制约定**见 [trellisx-worktree.md](./trellisx-worktree.md), 本文件禁重复其条款
- 摘要: task.py start 后自动建 `.trellis/worktrees/<task>`; 源码改动必须落 worktree 内; subtask 拆 ≥ 2 + 异步并行派发; task archive 干净则自动销毁

## 标准开发流程 (5 步)

① 创建任务 + 切 worktree (task.py create+start, 自动建 .trellis/worktrees/<task>)
② 任务规划 (拆 ≥ 2 subtask, 写 prd/design/implement + subtask 文件 + 调度图)
③ 异步执行 (按调度图调度 subtask agent, 无依赖的并行派发提效)
④ 整体 trellis-check 校验 (闭环)
⑤ commit + finish (合并移除 worktree → commit → archive → 落 cortex)

## trellis-check 闭环 (强制)

- task 完成前**必经** `trellis-check` 综合验证
- check 未过禁宣告 done

## 分工 (融合 trellis 原生)

| 能力 | 用谁 |
| --- | --- |
| 建 task / start / archive / add-subtask | trellis 原生 `task.py` |
| 实施 / 检查闭环 | trellis 原生 `trellis-implement` / `trellis-check` |
| 增量 spec 捕获 | trellis 原生 `trellis-update-spec` |
| 破坏式 spec 重写 | trellisx `trellisx-spec` skill |
| planning 文档编排 (PRD/design/implement/subtask 文件 + 调度图) | trellisx `trellisx-orchestrate` skill |
| worktree 隔离 | trellisx (本约定 + 平台 hook) |
