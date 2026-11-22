# 先验快照（2026-11-22，非盲写：skill 初版由本会话刚写）

## 现有 skill 覆盖的（不再算增量）

- 队列循环：清点 → 提取 → 单票闭环 → 清理 → 循环，#67 最后。
- 提取顺序：基线红 → 家族基座 → 平台适配 → 独立票 → 总任务。
- 单票七步：fetch + origin/master 开 worktree → 实现阶段性 commit → 增量测试 → 审查回环 → push+PR → 合并前全量门禁 → 存档清理。
- 并行：cargo 全局串行、agent ≤3、一工作区一写 agent。
- 收尾：session 文件记账 + Closes #N。

## 我认为还缺的（待语料证实）

1. PR 合并纪律（SHA 绑定、push 失败处置、合并态复测）只字未提。
2. 中断续做 checklist 与半成品采信三前提未提——issue 轮断连是常态。
3. 派发 prompt 的固定要素（规则原文+file:line、状态词、前台运行句）未提。
4. 假红归因（并发重建、代理 env、夹具漂移）未提，实现 agent 会把假红当回归。
5. commit 细节（$ 展开、locale patch 过滤、索引核对）未提。
6. 磁盘前置检查（df -h <10G 不开跑）未提。
7. ticket 状态词与 GitHub issue / 本地票两套轨制的关系未理清。

## 大众（新会话）最常见的错误印象

- 「门禁绿 = 能合并」——实际 agent 子集绿不作数，主仓全量才算。
- 「worktree 隔离 = 随便并行」——cargo 一次锁整个 workspace。
- 「fetch 失败 = 用本地 master 凑合」——实锤禁止。
