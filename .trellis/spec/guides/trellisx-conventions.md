---
updated: 2026-06-10
authored-by: trellisx-apply
---

# trellisx 任务编排约定

何时被读: trellis task 实施 / 检查时 (sub-agent dispatch 注入)
谁读: main / trellis-implement / trellis-check / 任何执行者
不遵守的代价: worktree 污染主工作区 / task 无隔离 / 流程跳步

## worktree 隔离 (trellisx 补 trellis 缺失)

- task.py start 后自动建 `.trellis/worktrees/<task>` (平台 hook trellisx-worktree.py)
- 全部源码改动**必须**落 worktree 内, 主工作区保持干净
- main 可直接写源码 (trellis inline), 但目标路径必须在 worktree
- 复杂 / 并行 subtask → 派 sub-agent (isolation:worktree) 或 agent-team 成员
- task archive 时: worktree 干净 → 自动销毁; 脏 → 警告先合并

## 标准开发流程 (5 步)

① 创建任务 + 切 worktree (task.py create+start, 自动建 .trellis/worktrees/<task>)
② 任务规划 (拆 ≥ 2 subtask, 写 prd/design/implement + subtask 文件 + 调度图)
③ 异步执行 (按调度图调度 subtask agent, 无依赖的并行派发提效)
④ 整体 trellis-check 校验 (闭环)
⑤ commit + finish (合并移除 worktree → commit → archive → 落 cortex)

## subtask 拆分

- task 必须拆 ≥ 2 subtask (按 实施/验证/文档 维度)
- 每 subtask 独立文件 `.trellis/tasks/<task>/subtask/<id>-<slug>.md` (见 trellisx-orchestrate skill)
- PRD 含 mermaid 调度图 (依赖 + 并行)
- parent-child 用 trellis 原生 `task.py add-subtask`

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
