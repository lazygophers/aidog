# 并行 implementer：基线红名单前置、追加任务不可靠

2026-09-30 jev-decision-proxy 轮（PR #46：一个 spec 拆 7 步、5 个 implementer
agent 各自 worktree 并行、逐个合并回 PR 分支）踩出的两条流程缺口。适用于一切
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

## 2. SendMessage 追加任务给运行中的 agent 不可靠

主会话给运行中的 impl-reg 追加补充修复任务，agent 未采纳，主会话最后自己修了。
运行中 agent 的注意力在当前票上，追加消息可能被忽略或做一半丢掉。

- 任务与边界一次性写进派发 prompt（同模式先例：wayfinder-session.md 第 2 条
  「状态迁移指令要贴进派发 prompt」）。追加需求宁可等 agent 回报后再派一轮，
  或主会话直接自己修。
- 确要追加：发完在 checkpoint 记一行「已追加 X、待核对」，agent 回报时核对该
  任务是否被执行，未执行即主会话兜底——不把「已发消息」当成「已安排」。
