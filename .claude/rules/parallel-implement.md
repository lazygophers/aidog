# 并行 implementer：基线红名单前置、追加任务不可靠

2026-09-30 jev-decision-proxy 轮（PR #46：一个 spec 拆 7 步、5 个 implementer
agent 各自 worktree 并行、逐个合并回 PR 分支）踩出的流程缺口。适用于一切
「主会话拆票 + 多 agent 并行实现 + 主会话合并验收」的形态。

**默认不并行、不开 worktree**（2026-11-04 用户：「尽可能的不要使用worktree开发，除非有必要」）：
implementer 在主 checkout 当前分支串行跑，「有必要」的三种情形见 shared-worktree.md 文首。
下面涉及 worktree / 基点 / 合并的节，只在确实开了 worktree 并行时适用。

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
- **派发 prompt 里的需求词有歧义就先定义**（2026-11-06 statusline「官方协议」：agent 列出 3 种解读后停下，用户插话澄清
  时 agent 已停，需 SendMessage 唤醒续做）：prompt 写明该词在本仓的所指（如「官方协议 = 平台 protocol 为 anthropic 的官方直连」）
  与回退口径；仍有多解的，让 agent 回传解读由主会话 ask-me，用户的澄清等 agent 回报后连同答复一次性发新 prompt。

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
- **数据落库与代码票并行时，基点漂移的是数据**（2026-10-29 计费线票 19）：agent 基点
  da764097d 下写断言「kimi/qianfan 无 tools_pricing → est_cost 0」，主会话随后落库
  bc6628dee 给两家上了价，合并必红。采信前对 agent 测试里每条「某数据缺失 / 为空 /
  为 0」断言，`git log <agent 基点>..HEAD -- src-tauri/defaults/registry/` 查基点后
  有无数据提交；有就改按真值断言，或让断言自带 stub 数据不依赖 registry 现状。

## 5. 多补丁流水线：`git apply --check` 通过 ≠ 已应用（2026-10-08）

主会话对补丁 A 只跑了 `--check` 就转向补丁 B，直到 cargo 测试报
「registry passthrough 不在 Protocol 枚举」才发现补丁 A 从未 apply。

- 「检查」和「应用」走同一条命令：`git apply` 本身先检查后应用，失败即停；不单独
  跑 `--check` 把应用留到之后。
- 必须先查后做别的：应用完立即 `git status --short` 点数确认改动落盘，再转向下一个
  补丁——「检查过」不占「应用过」的位置。
- **补丁里的新文件**（2026-10-29）：agent worktree 里新建的未跟踪文件不进 `git diff`，
  导补丁前先 `git add -N <新文件>`（或让 agent 回报新文件清单）；主会话 apply 后
  commit 前，新文件也要 `git add`，否则 `git commit -- <路径>` 报 pathspec 未跟踪而失败。

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
- **reviewer agent 的发现同样先实证**（2026-10-29 计费线）：reviewer 报「tools 真值源
  双源不对称」，主会话 grep 见 `default_tools_endpoints` 与 `is_tools_endpoint` 同走
  `effective_presets`（billing.rs:281-283），属误报；成因是只读了纯函数测试直读
  `presets()` 的路径。「双源 / 不对称 / 死码」类发现，采信前 grep 两侧调用链到同一个
  源头再决定修不修。

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
  命令模板改 `2>&1 | tee .tmp/<任务名>/gate.log | grep -E 'test result|error'`——全量落
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
- **MARK 在场 ≠ 链跑完**（2026-10-29 计费线实锤）：`grep ... | head; echo MARK` 结构把
  cargo 中途失败吞掉，MARK 照打。`grep -cE 'test result: ok'` 得 4、期望 <N>（cargo
  编译期失败中断），差点当绿。门禁链除 MARK 外再核一个数：`test result: ok` 计数 =
  suite 总数；开工前在干净 HEAD 数一次期望值写进派发 prompt（<N> 例：2026-10-29 为 34，随
  crate 增减变，不当常量）。
- **门禁脚本名从 `package.json` scripts 抄，不凭记忆写**（2026-10-29）：派单与票面模板
  曾写 `yarn check-i18n`（不存在），真名 `yarn check:i18n`（同族：`check:registry` /
  `check:modal` / `check:tokens` / `check:rpc` / `check:docs`），首链因此在尾步红。

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

## 9. 整体 revert 旧提交撤功能：逐 hunk 归属复核（2026-10-29 去抽屉轮）

去抽屉用 revert 15f85b503 + 91571e2b8 压成一提交（cc2fa3d87）。整体 revert 会连同旧提交里
非目标改动一起删（该轮风险点：`duplicating` 复制平台、appbuilder 侧栏条目）。

- 合并前派复核：对 revert diff 逐 hunk 标「属目标功能 / 属别的功能」，后者即误删，修回。
- 以可达性证据收口：被删符号的其它入口仍在（该轮 `PlatformEditForm.tsx:287`、
  `usePlatformsState.ts:704`），不凭「diff 看着没问题」。

## 10. 门禁红先跑基线对照再归因；worktree 内 build:web 的限制（2026-10-29）

- 升级依赖后 `yarn build:web` 红（`document is not defined at useThemeMode.ts:33`），agent 停下问
  「跑基线还是认定无关」。只有一条路：直接 detach worktree 到升级前 commit、独立 `yarn install`
  跑同一命令（perf-testing.md §7 双版本对照）；基线同红 = 预存，记遗留，不算升级回归。
- `build:web`（next/turbopack）在 harness worktree 里因 `node_modules` 软链找不到 next
  （`web/next.config.ts` 的 `turbopack.root`）。验证在主仓跑；非要在 worktree 跑，临时改
  `turbopack.root` 并在提交前还原（该轮已还原、未入提交）。

## 11. 等别人的门禁日志：禁 `tail -F | grep -m1`，用 node 等待脚本（2026-10-30）

派发 prompt 让 agent 用 `tail -n +1 -F <log> | grep -m1 GATE_CHAIN_DONE` 等别人的门禁：
grep 命中退出后，日志已写完、tail 不再写入就收不到 SIGPIPE，管道永不结束，agent 的
Bash 一直挂到超时；一轮下来残留 9 个 tail（最老 2h13m），多个 agent 卡死。改用 node
`fs.watchFile` 等待脚本后首用即成功（票 06 合并）。

- 等待脚本在命中判据后自己 `process.exit(0)`，不留后台子进程；派发 prompt 直接贴脚本，
  不贴 shell 管道写法。
- 命中判据三条同时满足：行数 > 5、含 `make lint` 的 `EXIT` 行、末行为 `GATE_CHAIN_DONE`。
  日志里残留的单行 `GATE_CHAIN_DONE`（上一轮遗留）会让「日志含标记」误判。
- 清理残留 tail 按 PID 核对命令行（`tail … .tmp/*/*.log`）再 kill，不用 pkill
  （perf-testing.md §1）。

## 12. harness worktree 里 `check:docs` 恒红：主仓补跑（2026-10-30）

worktree 缺 docs 子包依赖（`typescript-eslint`），`yarn check:docs` 恒 EXIT 2，与改动无关。
本轮每张票的实现回报都带这一条红。

- 派发 prompt 写明：worktree 内 `check:docs` EXIT 2 属已知，不修、不复跑；其余步骤全 0 即交付。
- `yarn test:statusline-golden` 同理：`scripts/statusline-golden/build.mjs` 不读 `CARGO_TARGET_DIR`，
  在设了独立 `CARGO_TARGET_DIR`（`.worktrees/.target`）的 worktree 里 ENOENT，只在主仓跑。
- harness 隔离 worktree 里的 agent 对主仓的 git 操作可能被 harness 拒绝；合并一律派单独的合并 agent
  在主仓做。基点落后 master 时用 `merge --no-ff`，并在主仓重跑完整门禁（agent 侧测试数不作数）。
- 合并 agent 在主仓补跑 `check:docs`，改了 command 签名 / 删 command 的票还要重新生成
  docs（先例 bf3ba6cd8）。
- 新增 / 改签名 `tauri_command!` 的票，其门禁必含 `yarn check:api-catalog`（已在 `make lint`，无 CI、无 pre-commit，
  不手跑就没人跑）；红则 `yarn gen:api-catalog` 后一并提交（2026-11-05 票 01 漏 regen，4c185f112 补）。

## 13. 已回报的 agent：回报转述后当场 TaskStop（2026-10-30）

completed 的后台 subagent 不会从列表消失，界面一直显示最后一步，用户两次要求
「已完成的后台 agent 及时关闭」。

- 回报转述完当场对该 agent `TaskStop`；返回 `not running` 或 `No task found` 都说明该 agent 已结束，无需再试。
- 空闲 teammate（已回报的 SendMessage 型）发 `shutdown_request`。
- 派发前数在途数（§8）时，只算 running，不把 completed 算进去。

## 14. 渲染类 UI 票：门禁全绿不等于视觉验收（2026-10-30 流向图）

FlowChart 泛化票（a692fe57f / 5e27d1b36）门禁全绿合入，用户看真机截图后否掉
（「页面过于丑陋且没有动效」）：节点等高不成比例、连线压标签、0 值节点照显、
空标签、颜色不可读、无动效共 8 处缺陷，重做一整轮（6600c646d）。

- 图表 / 画布 / 动效类组件票，验收标准含「Playwright 真渲染深浅两 mode 截图 + 缺陷清单
  逐条核」（取证法见 ui-color-bug-verify.md 规则 2），截图落 `.scratch/<feature>/`；
  票面没有截图验收行不算完成。
- 主会话合并前读一遍截图，按比例、遮挡、空值、对比度、动效五项自查；真机 WebKit /
  reduced-motion 未覆盖的列入回报「人工验收 N 条未执行」（pr-merge-discipline.md §2）。

## 15. 门禁退出码不进管道；agent 中断后的续做 checklist（2026-10-30 ~ 11-02 平台-客户端白名单线）

- **门禁命令禁管到 `tail` / `head` 再取退出码**：合并 agent 把 `yarn test:cov` 管到 `tail -15`，
  退出码与 `Tests` 行一并丢失（票 03 合并），靠票 08 合并补证。命令写 `cmd > .tmp/<任务名>/x.log 2>&1; echo EXIT=$?`，
  再 `grep -E 'Tests |Test Files' .tmp/<任务名>/x.log`；派发 prompt 把这条写死。
- **会话中途退出会让后台 agent 中断**（本线 2 次：规划 agent、票 05）。续做按此顺序核对现场，再派续做 agent：
  1. 票 `Status:` 是否停在 `claimed`（未写 `## Answer` = 没结票）；
  2. `git -C <worktree> status --short` / `diff --stat` 半成品范围，最后一次编译错误原文；
  3. 主仓 `status --short` 有无误写副本（与 worktree `cmp`）；
  4. 锁残留：`/tmp/aidog-cargo.lock`（见 shared-worktree.md）、`.git/index.lock`；
  5. 半成品含 JSON / 数据文件先跑语法门禁（`yarn check:defaults` 覆盖 `src-tauri/defaults/` 全部 JSON）
     再续做——2026-11-02 preset 权限轮：进程退出杀掉 agent，preset 留下非法 JSON（编辑脚本劣化垃圾），
     编译与既有门禁全绿，无门禁能拦。半成品采信走 shared-worktree.md「implementer 挂掉后的半成品采信」三前提。
- **派数据 / JSON 批量编辑给 agent 时，prompt 附确定性脚本全文**（2026-11-02 preset 权限轮实锤）：
  prompt 只写「精确文本插入」描述、让 agent 自己拼编辑脚本，agent 侧同样发生生成流劣化
  （surgical-refactor.md §3 同模式）；agent 被中断后脚本垃圾直接落进数据文件。改法：把
  删垃圾 + 锚点插入的 python/node 脚本全文写进 prompt，agent 只负责跑与回报校验输出。
- **harness 并发上限是环境变量 `CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS`（默认 2）**：用户说「不要管限制」
  只解除本项目的 ≤2 约定，不改 harness；第 3 个派发报 `Concurrent subagent limit reached` 时不重试，
  任务排队，任一完成再派。
- **主仓 `node_modules` 缺失**（票 07 observed，原因不明）：agent 在主仓补 `yarn install --immutable`，
  装完确认 `git status --short` 无 `yarn.lock` 变动；再出现时记当日账本遗留，累计两次再查成因。
- **TDD 与截图豁免要写进票面**：票 07 未先红后绿、票 08 以「渲染未变」跳过截图。修 bug / 加校验的票，
  agent 回报须贴先红的失败输出；豁免截图须在回报写明「渲染未变」依据（diff 不含 JSX / 样式），
  否则按 §14 列人工验收。

## 16. 先红输出全量落文件，禁 `head -N` 截断

新增测试的「先红」证据：`cmd > .tmp/<任务名>/red.log 2>&1; echo EXIT=$?`，再对每个新测试名各 grep 一次
`FAILED|failed|<测试名>`；管到 `head` / `tail` 会截掉新测试的失败行，无法确认它先红过。

## 17. 门禁里的计时断言偶发红：`--no-fail-fast` 取全量，重跑定性

`cargo test --workspace` 遇一个 suite 失败就停，后面的 suite 不跑，拿不到全量结果。
门禁命令写 `cargo test --workspace --no-fail-fast > .tmp/<任务名>/x.log 2>&1; echo EXIT=$?`。
计时断言（如 `aidog_stats::test_usage_stats::flow_breakdown_7d_scale_guard`，并发编译后首轮曾超时）
首轮红先单跑该测试一次：绿 = 负载抖动，记账本遗留；仍红才按回归处理。

## 18. 网络断连中断、先红严格性、票状态词、带密钥写命令登记（2026-11-04 ~ 11-05）

- **SSL / 断网中断是常态**：2026-11-04.md:216/222 API 票 01 两个 agent 连续 `SSL certificate hostname mismatch`
  中止，2026-11-05.md:300 又一次。续做走 §15 的核对现场清单；采信半成品的 commit message 注明
  `adopted from interrupted agent`，门禁整条重跑（11-05.md:300 即此写法）。派续做 agent 时 prompt 写明
  「前一 agent 因断网中止，先 `git diff` 逐 hunk 核对再接着做」，不让它从头重写。
- **先红要严格**：2026-11-04.md:24 策略库 4 条「测试与组件同写、未拍到真先红」，2026-11-05.md:92 生成脚本
  「非严格先红，事后改坏三处反证」，11-05.md:27 review 因「先红输出未见」无法核实 Spec。
  派发 prompt 要求：先只写测试跑出红（`> .tmp/<任务名>/red.log 2>&1; echo EXIT=$?`），回报**贴 EXIT 值与失败测试名**，
  再写实现；确实做不到（测试与实现不可分）写明原因并改用「改坏实现→红→按行号还原」反证，回报标「非严格先红」。
  回报只写「先红已验」而无 EXIT 值和失败名，按没验处理。
  **红必须是断言红，不是编译红**（2026-11-05 模型-自定义槽位：4 张票 3 张非严格——符号缺失编译红、夹具缺项红、写完实现再换回）：
  先加桩（新符号只给签名 + 默认返回值，夹具补全）使测试编译通过，再跑出断言失败；票 03 桩先行即得真先红。
  review 修复补的测试同样适用（2026-11-06 38ca3d577 仍非严格先红，靠改坏 decision.rs 反证）：派 review-fix agent 时 prompt 照贴本条。
- **`make lint` 整条跑**（2026-11-06 额度线遗留「make lint 未整条跑，仅分项跑」）：收尾门禁以 `make lint > .tmp/<任务名>/lint.log 2>&1; echo EXIT=$?` 为准，
  分项绿不算；收尾 agent 回报贴 EXIT 值，没跑写「做了没验证」。
- **绕开脏文件另建模块只是权宜**（2026-11-06 statusline 回退放进 fallback.py 因 engine.py 被他线占用）：占用解除后并回原模块，
  账本遗留写明并入条件；不把「文件被占」当拆文件的设计理由。
- **票 `Status:` 只有 `claimed` → `resolved`**：2026-11-05.md:101 遗留「票状态跳过 claimed」。
  与 wayfinder-session.md §2 同口径，派发 prompt 三行词表照贴；合并后当场 grep `Status:`。
- **新增带密钥的写命令，同提交登记 `RESTORERS`**（`aidog_rpc/src/secret_fields.rs:114`）：2026-11-05.md:105
  审查补了 `kernel_settings_set` / `proxy_client_set_settings` / `settings_set` 三处漏登记；裁决「不修」Shotgun
  （2026-11-05.md:106）的复活条件是「再新增带密钥写命令」。派发含新增写命令且涉及 `SECRET_PATHS` 字段的票时，
  prompt 写明此登记要求，review 时对照检查。建议做成机械检查：「`SECRET_PATHS` 涉及的写命令 ⊆ `RESTORERS`」测试（未做，属代码改动）。

## 18. 门禁日志与构建目录收回仓库（2026-11-05）

用户原话：「Debug 的产出或者是这些前端的产出，能不能都限制在当前目录，而不是每次都去 temp 目录下面的临时文件里面创建，让它们每次都在同一个地方？这样更便于清理和管理。」

- 上文各节的 `/tmp/<名>.log` 一律落仓库 `.tmp/<任务名>/<名>.log`（`.gitignore` 已忽略），收工删该目录；
  命令前先 `mkdir -p .tmp/<任务名>`。
- 上文 `CARGO_TARGET_DIR=/tmp/aidog-agent-target` 作废：主 checkout 用默认 `src-tauri/target`，仅 worktree 用 `<项目根>/.worktrees/.target`（见 shared-worktree.md「构建目录」）。
