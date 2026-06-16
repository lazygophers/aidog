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
- 摘要: task.py start 后自动建 `.trellis/worktrees/<task>`; 源码改动必须落 worktree 内; **按独立可验收交付数判定拆分** (多交付才拆 parent+child 并行, 单一交付不强制拆); task archive 干净则自动销毁

## 标准开发流程 (5 步)

① 创建任务 + 切 worktree (task.py create+start, 自动建 .trellis/worktrees/<task>)
   - 失败回退: start 报会话身份错 → 按提示设 `TRELLIS_CONTEXT_ID` / 会话身份后重试, 禁跳过直接 inline
② 任务规划 (**按独立可验收交付数判定**: 多交付→拆 parent+child + 调度图; 单一交付→单 task 不强制拆。写 prd/design/implement)
   - 失败回退: 拿不准多交付还是单交付 → 用 AskUserQuestion 问用户, 禁自行假定
③ 异步执行 (多交付: 按调度图调度 subtask agent, 无依赖的同一回复一次性并行派发; 单交付: main 在 worktree 内直接 edit)
   - 失败回退: 无依赖却被逐个串行派 → 停下, 改回同一回复并发派, 串行 = 关键路径被拉长
④ 整体 trellis-check 校验 (闭环)
   - 失败回退: check 未过 → 修复重检, **禁带病宣告 done / 禁跑 finish 脚本**
⑤ commit + finish (合并移除 worktree → commit → archive → 落 cortex)
   - 失败回退: worktree 未合并就 archive → 分支被销毁丢提交; 必须先 `merge --no-ff` 成功再 archive

## 反模式 (禁)

| 反模式 | 后果 |
| --- | --- |
| 在主工作区写源码 (file_path 非 worktree 路径) | 污染主工作区, 隔离失效, 与他人改动冲突 |
| check 未过就宣告 done / 跑 finish | 带病归档, 流程未闭环 |
| 无依赖 subtask 逐个串行派 | 各 subtask 耗时叠加, 关键路径被拉长 |
| 未 `merge` 就 archive worktree | 分支销毁, 提交丢失 (见 trellisx-worktree.md 教训) |
| 编造用户没说过的 "inline / 跳过流程" 覆盖指令 | 擅自绕过 task 流程 |

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
