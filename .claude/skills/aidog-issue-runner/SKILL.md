---
name: aidog-issue-runner
description: |
  批量处理 GitHub issue 的执行循环：清点开放 issue → 按优先顺序逐个提取 → 每个 issue 走「独立 worktree + PR」闭环 → 合并后清理 → 循环直到清零。固化 origin/master 基点、增量测试/审查、合并前主仓全量门禁、push 预检与 SHA 绑定、假红归因、中断续做 checklist。触发词：处理 issue、跑 issue、清 issue、issue 队列、下一个 issue、批量处理 issue、issue 清零。不在一般性代码问答上自动触发。
when_to_use: 用户要求处理一个或全部 GitHub issue；继续 issue 处理队列；某 issue 开发到一半要续做
---

# aidog issue 处理队列

你是每张票的交付负责人：**完成 = 合并进 master + worktree 清理 + 票关闭**，代码写完只是中间站。

总览 = 一个循环：**清点 → 提取一个 issue → 执行单票闭环 → 合并清理 → 未清零回到提取**。#67（出站协议转换总任务）留到最后关单。2~3 张票轮转、各自走完整闭环，同一时刻 2~3 个 worktree 并行写，cargo 测试全局串行。

## 提取顺序

取票前 `gh issue list --repo lazygophers/aidog --state open` 核对当前清单，票可能已被其他会话处理。

1. 测试基线红最先：#217（vitest 10 红）、#211（gateway flaky）。基线红不修，后续每个 PR 都要区分「新引入」与「预存」。
2. 家族基座：#82~#90（平台适配的公共层）。
3. 平台适配：#91~#155，按家族分组批量取。
4. 独立 issue：#66 / #57 / #58，可与第 3 批穿插。
5. #67 总任务：全部子票 resolved 后验收关单。

## 单个 issue 闭环

1. **建 worktree**：先 `git fetch origin`；分支 `issue-<编号>-<短slug>` **必须从远端 `origin/master` 开**：`/usr/bin/git worktree add <仓库根>/.worktrees/issue-<编号>-<短slug> -b <分支> origin/master`。fetch 失败不是回落本地 master 的许可，未取得 `origin/master` 时不建分支、不开工。建完 `rev-parse --show-toplevel` 验一次，基点 sha 写进回报。细则：`.claude/rules/shared-worktree.md`「issue 开发：独立 worktree + PR」节。
2. **实现**：worktree 内改码；每个可验证单元（编译过 + 相关测试绿）commit 一次，回报前最后一步是 commit。commit 前自问：「这段 diff 的每一段，我都能说出它属于哪张票吗？」说不出的那段就是别人的。开工先 `git status --short` 记脏文件、`diff --cached --stat` 核索引；locale 文件一律按脏文件走 patch 过滤。
3. **增量测试**：只跑改动面相关子集。worktree 内 `check:docs` EXIT 2、`yarn test:statusline-golden` ENOENT 属已知红，不修。假红（extern 丢失 / E0463 / 代理 env / 夹具漂移）归因见 [references/gate-and-dispatch.md](references/gate-and-dispatch.md)。
4. **审查**：对照 issue 验收标准查行为与边界，发现与裁决记进 worktree 内 `review-notes.md`，修复回环以它为准；失败在原 worktree 内回到「修复 → 增量测试 → 再审查」。同一问题连续三轮仍失败 → 停，ask-me。
5. **push + PR**：验收标准全部满足后 push 并建 PR（永久授权）。push 前自问：「GitHub 端 head SHA 还是我验证过的那个吗？」push 预检、失败处置、READY 的 SHA 绑定见 [references/merge-and-interrupt.md](references/merge-and-interrupt.md)。
6. **合并**：🔴 合并前停一拍——核对 GitHub 端 head SHA 仍是验证过的那个（`git ls-remote origin <分支>` 对比验证时的 sha，或 `gh pr merge <PR> --match-head-commit <完整SHA>`）。合并回 master 在主仓做，只挪指针、不动在途脏文件。**合并前在主仓跑全量门禁一次**（`cargo test --workspace --no-fail-fast` + `make lint` 整条）——agent 侧增量子集不作数。多 PR 同动 Cargo.lock / 同 crate：对合并态全量测试后再清理。
7. **清理**：先把门禁证据日志移到主仓 `.tmp/pr-<PR号>/`，再删 worktree 与分支；删前 `git -C <worktree> status --short` 非空则停手上报。

## 常见误读（遇事先对表）

| 误读 | 事实 | 出处 |
|---|---|---|
| agent 报绿 = 门禁真绿 | 命令面与 crate 面两个盲区，采信前主会话复跑 | parallel-implement.md §7 |
| check:docs 在 worktree 红 = 改坏了 | worktree 恒 EXIT 2 属已知，合并时主仓补跑 | parallel-implement.md §12 |
| push 失败 = 换本地基线重开 | fetch 失败禁回落；正解 = merge origin/master 进分支 | shared-worktree.md「push 预检条件设计」节 |
| 门禁中途红 = 我的回归 | 先查夹具近期提交与他人脏文件集 | parallel-implement.md 派发前置条 |
| timer 断言偶红 = 回归 | 先单跑一次定性，绿 = 负载抖动记遗留 | parallel-implement.md §17 |

## 并行约束

- cargo 全局串行：共用 `<项目根>/.worktrees/.target`，同一时刻只允许一个 cargo 跑；取锁走 `/tmp/aidog-cargo.lock` node 轮询；删 target 目录也要持锁。
- 后台 agent 同时 ≤ 3 个；同一工作区一个写 agent。
- 开跑前 `df -h .`，余量 <10G 不开跑；满盘先报主会话，不擅自删 target。
- 分支保护：master 禁强推，secret-scan 是必需检查，PR/push 只扫新增提交区间。

## 派发与收尾

- 派 agent 的固定要素（规则原文+file:line、状态词三行、前台运行句、清单计数）见 [references/gate-and-dispatch.md](references/gate-and-dispatch.md)。
- 中断续做五查与半成品采信三前提见 [references/merge-and-interrupt.md](references/merge-and-interrupt.md)。
- 收尾：session 状态文件记一行（issue 号、PR 号、门禁 EXIT）；PR 正文写 `Closes #<编号>`；含真机行为的 PR 在回报单列「人工验收 N 条未执行」。
