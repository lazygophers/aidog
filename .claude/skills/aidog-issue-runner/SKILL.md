---
name: aidog-issue-runner
description: |
  批量处理 GitHub issue 的执行流程：清点开放 issue → 按优先顺序逐个提取 → 每个 issue 走「独立 worktree + PR」闭环 → 合并后清理 → 循环直到清零。固化基线红优先、增量测试/审查、合并前主仓全量门禁一次、cargo 全局串行、push/PR 永久授权的边界。触发词：处理 issue、跑 issue、清 issue、issue 队列、下一个 issue、批量处理 issue、issue 清零。
when_to_use: 用户要求处理一个或全部 GitHub issue；继续 issue 处理队列；某 issue 开发到一半要续做
---

# aidog issue 处理队列

总览 = 一个循环：**清点 → 提取一个 issue → 执行单票闭环 → 合并清理 → 未清零回到提取**。#67（出站协议转换总任务）留到最后关单。

## 提取顺序

1. 测试基线红最先：#217（vitest 10 红）、#211（gateway flaky）。基线红不修，后续每个 PR 都要区分「新引入」与「预存」。
2. 家族基座：#82~#90（平台适配的公共层）。
3. 平台适配：#91~#155，按家族分组批量取。
4. 独立 issue：#66 / #57 / #58，可与第 3 批穿插。
5. #67 总任务：全部子票 resolved 后验收关单。

取票前 `gh issue list --repo lazygophers/aidog --state open` 核对当前清单，票可能已被其他会话处理。

## 单个 issue 闭环

1. **建 worktree**：先 `git fetch origin`，确认拿到最新 `origin/master`；分支 `issue-<编号>-<短slug>` **必须从远端的 `origin/master` 开**（`git worktree add <仓库根>/.worktrees/issue-<编号>-<短slug> -b <分支> origin/master`），禁从本地 HEAD 或本地 master 开——fetch 失败不是回落本地 master 的许可，未能取得 `origin/master` 时不建分支、不开工（shared-worktree.md「推送只走 origin」节实锤条）。建/删/查走 `/usr/bin/git` 直跑，建完 `rev-parse --show-toplevel` 验一次，并记下基点 sha 写进回报。细则见 `.claude/rules/shared-worktree.md`「issue 开发：独立 worktree + PR」节。
2. **实现**：worktree 内改码；每完成一个可验证单元（编译过 + 相关测试绿）commit 一次，implementer 回报前最后一步是 commit。开工先 `git status --short` 记脏文件；locale 文件一律按脏文件处理走 patch 过滤（shared-worktree.md「硬规则」节）。
3. **增量测试**：只跑改动面相关子集（相关 crate 测试 + lint 子集）。worktree 内 `check:docs` EXIT 2、`yarn test:statusline-golden` ENOENT 属已知红，不修（parallel-implement.md §12）。
4. **审查**：对照 issue 验收标准查行为与边界；发现失败回到「修复 → 增量测试 → 再审查」，不重开 worktree。审查失败、门禁红都要贴命令原文 + EXIT 值，退出码不进管道：`{ cmd; rc=$?; echo EXIT=$rc; } > .tmp/<任务名>/x.log 2>&1`。
5. **push + PR**：验收标准全部满足后 push 该分支并建 PR（永久授权）。push 前核 `git rev-list --count origin/master..<分支>` 只含本 issue 提交。PR 评论按 `.claude/rules/pr-merge-discipline.md` §4 自动发送，合并评论带门禁证据（§4「合并时评论必须自带证据」）。
6. **合并**：合并回 master 在主仓做，照 shared-worktree.md「共享 checkout 合回 master」节只挪指针、不动在途脏文件。**合并前在主仓跑全量门禁一次**（`cargo test --workspace --no-fail-fast` + `make lint` 整条）——增量子集不作数（parallel-implement.md §7）。多个 PR 同动 Cargo.lock / 同 crate 时，合并后对合并态跑一次全量测试再清理（pr-merge-discipline.md §5）。
7. **清理**：先把评审与门禁证据日志从 worktree 移到主仓 `.tmp/pr-<PR号>/`，再删 worktree 与分支；删前 `git -C <worktree> status --short` 非空则停手上报。

## 并行约束

- 同一时刻多个 worktree 可并行写，但 cargo 全局串行：共用 `<项目根>/.worktrees/.target`，同一时刻只允许一个 cargo 跑（shared-worktree.md「构建目录」节）。
- 后台 agent 同时 ≤ 3 个；同一工作区一个写 agent（parallel-implement.md §8）。
- 分支保护：master 禁强推，secret-scan 是必需检查；PR/push 只扫新增提交区间（CLAUDE.md 分支保护条）。

## 收尾登记

每票完成后在 `.scratch/session/<会话编号>.md` 记一行（issue 号、PR 号、门禁 EXIT）；合并的 PR 在 GitHub 侧关闭对应 issue（PR 正文写 `Closes #<编号>`）。
