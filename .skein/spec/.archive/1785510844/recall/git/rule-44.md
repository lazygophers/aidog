---
title: 并行 subtask commit 竞态防护
layer: recall
category: git
keywords: [git,并行,subtask,commit,竞态,staged,worktree]
source: -
authored-by: skein-spec
created: 1784730592
status: active
related: []
updated: 1784730592
---

## 触发场景
同一 worktree 并行跑多个 subtask 时，不同 agent 可能对同一文件产生变更，导致 git index 竞态。

## 陷阱-正解
❌ **陷阱**：多个并行 subtask 各自 commit，兄弟 staged 文件可能被误入彼此的 commit（如 agent A 提交时带了 agent B 的 staged 文件）。
✅ **正解**：commit 前用 `git diff --cached --name-only` 核验落点，只提交本 agent 涉及的文件；误入则 reset --soft + restore --staged + 重提。

## 处理流程
```bash
# commit 前检查 staged 文件
git diff --cached --name-only | grep -v "^本-agent-路径前缀/"

# 如发现误入兄弟 agent 的文件
git reset --soft HEAD~1          # 撤 commit（保留 staged）
git restore --staged <误入文件> # 从 staged 区移除
# 重新 git add 本 agent 文件 + commit
```

## 适用
- 同 worktree 并行 subtask（skein parallel 模式）
- 多 agent 同时改同一文件的不同区域

## 关联
git-worktree-parallel-isolation

## 案例
- shadcn-pages task 并行 m-groups/m-logs/m-stats 等子任务，需 commit 前核验 staged 文件归属
