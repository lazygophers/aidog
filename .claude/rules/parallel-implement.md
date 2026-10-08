# 并行 implementer：基线红名单前置、追加任务不可靠

2026-09-30 jev-decision-proxy 轮（PR #46：一个 spec 拆 7 步、5 个 implementer
agent 各自 worktree 并行、逐个合并回 PR 分支）踩出的流程缺口。适用于一切
「主会话拆票 + 多 agent 并行实现 + 主会话合并验收」的形态。

## 1. 基线红测名单开工前跑一次，写进每个派发 prompt

master 基线本身带红（该轮：vitest 1 红、flutter 4 红、aidog_db 2 红）。结果：
每个 implementer 都各自 stash 复跑一遍「证明是基线预存」，五个 agent 浪费十余
轮；且 impl-reg 与主会话各自修了同一个红测（index.json 排序 + slug 白名单），
同题双解只能择一合并、另一份作废。

- 主会话开工前在干净 master 跑一次全量门禁，把红名单（测试名 + 现象）原样贴进
  **每个**派发 prompt：「这些是基线红，不修、不复跑验证，报红照常交付」。
- agent 发现 prompt 未列的红测只上报不动手；基线红只由主会话修一次。
- 基线红 = 门禁信号丢失（红名单越长，「新引入 vs 预存」的判定越贵），发现
  当场开票，不修不等于不开票（该轮 Flutter 4 红 + vitest 1 红即此遗留）。
- 修门禁红派 agent 时，修复范围预先写死文件清单（2026-10-08 passthrough-platform
  轮：限 6 文件，agent 只改了 ws.rs，未漂移）——与「基线红只由主会话修一次」同精神：
  修复 agent 只动清单内文件，清单外的红上报不动手。

## 2. SendMessage 追加任务给运行中的 agent 不可靠

主会话给运行中的 impl-reg 追加补充修复任务，agent 未采纳，主会话最后自己修了。
运行中 agent 的注意力在当前票上，追加消息可能被忽略或做一半丢掉。

- 任务与边界一次性写进派发 prompt（同模式先例：wayfinder-session.md 第 2 条
  「状态迁移指令要贴进派发 prompt」）。追加需求宁可等 agent 回报后再派一轮，
  或主会话直接自己修。
- 确要追加：发完在 checkpoint 记一行「已追加 X、待核对」，agent 回报时核对该
  任务是否被执行，未执行即主会话兜底——不把「已发消息」当成「已安排」。
- **给已回报完 / 空闲的 agent 追加任务更不可靠**（2026-10-01 perf-backend 轮
  bench agent 实例：消息送达、agent 做一半停住，主会话接手收尾）：空闲 agent
  被唤醒后不保证把新任务跑完。有分量的新活重派一个新 agent 或主会话自己干；
  只把「顺手捎带」级别的小事交给唤醒。

## 3. 派发 prompt 里的路径先验存在，agent 侧留自检指令

派发 prompt 本身也会产出乱码（生成流劣化的一种，见 surgical-refactor.md §3）：
2026-10-08 tab-redesign 轮 2 次污染派发 prompt——路径写坏、引用不存在的文件名，
肉眼读像合理的。agent 靠 prompt 里预置的一句自我修正提示才接上。

1. 派发前把 prompt 里出现的每个路径 / 文件名 `ls` 核一遍存在性（相对主 checkout
   用绝对路径）；
2. 派发 prompt 尾部固定加一行自检指令：「若本 prompt 引用的路径 / 文件名在仓库
   中不存在，先自行核实真实路径再动手，不按 prompt 原文照搬」。

## 4. harness worktree 隔离的 agent：开工前核对基点（2026-10-08 passthrough-platform 轮）

两个 implementer 用 harness `isolation: "worktree"` 启动，各自 worktree HEAD 落在
9650d99a——比主 checkout 当时 HEAD（596eb8cc）落后 7 个提交。agent 在旧代码上写
补丁、门禁全绿；主会话合并时 `git apply --check` 一个过一个失败（PlatformEditForm.tsx
上下文漂移），`git apply -3` 落出一个错位 hunk（BreakerSection 加在 tab 改版前的
旧位置），被迫手工按 HEAD 新结构重解 + 全量重跑门禁。

- **agent 在 worktree 跑的门禁 ≠ 主 HEAD 上的门禁**，门禁结论不随补丁迁移。
- 派活后开工前核对基点：`git -C <worktree> rev-parse HEAD` 对比主 checkout HEAD；
  落后就先 rebase / 重建 worktree，再让 agent 动手。agent 运行期间主 checkout 前进
  （别的会话提交）同样造成漂移——合并前再对一次，不等开工时那一次管全程。
- 基点已落后且补丁已产出：按 shared-worktree.md「采信半成品」三前提处理——逐 hunk
  重审 + 主会话全量重跑门禁，不采信 agent 侧门禁结论。

## 5. 多补丁流水线：`git apply --check` 通过 ≠ 已应用（2026-10-08）

主会话对补丁 A 只跑了 `--check` 就转向补丁 B，直到 cargo 测试报
「registry passthrough 不在 Protocol 枚举」才发现补丁 A 从未 apply。

- 「检查」和「应用」走同一条命令：`git apply` 本身先检查后应用，失败即停；不单独
  跑 `--check` 把应用留到之后。
- 必须先查后做别的：应用完立即 `git status --short` 点数确认改动落盘，再转向下一个
  补丁——「检查过」不占「应用过」的位置。
