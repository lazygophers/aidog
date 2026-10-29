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
   中不存在，先自行核实真实路径再动手，不按 prompt 原文照搬」；
3. **多 worktree 并存时 prompt 全文内联，或写明唯一绝对路径并明令「禁入其它
   /tmp/aidog-* 目录」**（2026-10-20 db-sqlcipher-full 票 02：prompt 经 /tmp 下
   文件间接引用，agent 顺着同名 PROMPT.md 走进 obs 旧 worktree，白跑一轮。
   /tmp 下的 `aidog-*` 是多轮共用的命名约定，同名文件撞车是常态不是意外）。

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

## 6. 多 agent 并行审计：结论互相冲突时主会话实证定论（2026-10-18）

整仓审计按域派 4 个 agent（gateway / 数据层 / 前端 / 脚本构建），交叉验证时撞上
直接矛盾：脚本 agent 说 `converter::registry` 是活路径、gateway agent 说是死码。
主会话 grep 调用点定论 gateway agent 对，随后删 63 个死模块 + converter registry
共 -4284 行（115f82d79）。

- **agent 对同一事实给出矛盾结论时，不按多数票、不按先到先得采信**——主会话自己
  grep / 编译验证一手定论，再动手。agent 分域审计时每个 agent 只看得到自己的域，
  跨域事实（谁调用谁、死码判定）本来就是它们的盲区，矛盾是常态不是意外。
- **「死码」这类影响删除类改动的断言，删后编译 + 全量门禁绿才算证据**（比「agent
  说没人用」强一级）；删除量大时（本例 4000+ 行）主会话逐 hunk 抽查 diff 归属。
- 交叉验证是必做步骤，不是可省的收尾：本例若无交叉验证，脚本 agent 的错误结论会
  保住 4000 行死码。合并多 agent 报告时显式找「两份报告对同一符号/文件的不同说法」
  逐条对账。

## 7. agent 门禁结论的假绿盲区（2026-10-20 obs 轮实锤）

obs implementer 回报 clippy exit 0，主会话 `--all-targets -D warnings` 复跑当场
红（items_after_test_module 在 lib test 目标下才 warn，agent 只跑了部分 target 或
读了缓存）；同轮另一条：obs 只跑 aidog_db+aidog_logs 门禁，aidog_core 的
mitm_bypass 测试读加密列断言明文——子集门禁全绿，合并到 master 才炸
（e75cd454e 修）。**「agent 报绿」和「门禁真绿」之间隔着命令面和 crate 面两个
盲区，口头回报不构成证据。**

- 派发 prompt 的门禁命令写全文不写缩写：clippy 一律 `cargo clippy --all-targets
  -- -D warnings`，要求 agent 回报贴命令行原文 + warning/failed 计数；修完必须
  重跑同一条命令再报。
- **子集门禁的范围按数据流向定，不按改动 crate 定**：spec 改共享层（写入层——
  列加密、列改格式、写入语义变更）时，验收范围 = grep 该列/表名找所有 select
  它的测试所在 crate（`grep -rn '<列名>' src-tauri/crates/*/src/ --include='*test*'`），
  grep 一圈比事后跑全量门禁便宜。
- 主会话采信 agent 门禁前自己复跑一次（合并轮的「全量重跑门禁」三前提已含此条，
  无 PR / 直合路径同样成立）。
- 门禁输出禁纯 grep 过滤 test result：连续三轮（db-field-encryption ×2 +
  passthrough-log-fixes ×1）「首跑 1 红复跑绿、失败名被 grep 过滤丢失」无法归因。
  命令模板改 `2>&1 | tee /tmp/gate.log | grep -E 'test result|error'`——全量落
  文件，屏幕只留摘要，flaky 复现时失败名还在。
- **`&&` 门禁链上禁裸 grep 计数**（2026-10-24 routing-viz 轮两次实锤）：
  `cargo clippy 2>&1 | grep -cE '^warning|^error' && cargo test ...` 零 warning
  时 grep 退出码 1，`&&` 后面的 cargo test / tsc / vitest 全部静默不跑——输出
  看似有结果实为残缺（该轮靠链尾 MARK 行才发现）。计数类 grep 补 `|| true`，
  且链尾固定 `echo GATE_CHAIN_DONE`：回看后台输出先找 MARK，不在场 = 链没跑完，
  中间结果再多也不算门禁跑过。
- **裸 `cargo test` 只跑根包，是 workspace 子集假绿**（2026-10-28 直接模型转发轮实锤）：
  本仓 src-tauri 是 cargo workspace，`cargo test` 不带 `--workspace` 时 crates/* 成员
  （aidog_core / aidog_db 等）的测试一个不跑。票 01 门禁照写根包版，根包 14 passed
  全绿，成员 crate 的直通测试实际没跑——agent 自行补跑 `--workspace` 才暴露。票面 /
  派发 prompt 的 Rust 门禁一律写 `cargo test --workspace`；agent 拿到短命令自行补全
  并在回报注明。

## 7b. 门禁红先问「是不是我顺手改的」（2026-10-22 入站密钥轮）

门禁 agent 报 `test_connect.rs:639` 失败，根因是主会话顺手给未匹配 404 补了
`source_protocol`（请求没提）——范围外改动打破既有断言，回退后全绿。

- 门禁出现新红，先 `git diff` 找请求范围之外的 hunk，回退它再重跑；
  不先改测试断言去迁就。

## 8. 派 agent 前先数在途数（2026-10-21）

后台 agent 同时不超过 2（全局约束），三个并行 spec 各要 code-review 时必然串行
排队——这是可接受代价，不改规则；但**派之前先数在途 agent 数**，排队会饿住关键
路径（如 review 挡合并）时，把顺序重排或将轻票收给主会话自己干，而不是派完才发现
卡在队尾。
