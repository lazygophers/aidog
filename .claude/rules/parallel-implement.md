# 并行 implementer：基线红名单前置、追加任务不可靠

适用于一切「主会话拆票 + 多 agent 并行实现 + 主会话合并验收」的形态。**默认不并行、不开 worktree**：implementer 在主 checkout 当前分支串行跑，「有必要」的情形见 shared-worktree.md 文首。下面涉及 worktree / 基点 / 合并的节，只在确实开了 worktree 并行时适用。

## 1. 基线红测名单开工前跑一次，写进每个派发 prompt

master 基线本身可能带红。若每个 implementer 各自复跑一遍「证明是基线预存」，会浪费大量轮次；主会话与某 agent 各自修同一个红测，同题双解只能择一合并、另一份作废。

- 主会话开工前在干净 master 跑一次全量门禁，把红名单（测试名 + 现象）原样贴进**每个**派发 prompt：「这些是基线红，不修、不复跑验证，报红照常交付」。
- agent 发现 prompt 未列的红测只上报不动手；基线红只由主会话修一次。
- 基线红 = 门禁信号丢失（红名单越长，「新引入 vs 预存」的判定越贵），发现当场开票，不修不等于不开票。
- 修门禁红派 agent 时，修复范围预先写死文件清单——与「基线红只由主会话修一次」同精神：修复 agent 只动清单内文件，清单外的红上报不动手。

## 2. SendMessage 追加任务给运行中的 agent 不可靠

运行中 agent 的注意力在当前票上，追加消息可能被忽略或做一半丢掉。

- 任务与边界一次性写进派发 prompt（同模式先例：wayfinder-session.md 第 2 条「状态迁移指令要贴进派发 prompt」）。追加需求宁可等 agent 回报后再派一轮，或主会话直接自己修。
- 确要追加：发完在 checkpoint 记一行「已追加 X、待核对」，agent 回报时核对是否被执行，未执行即主会话兜底——不把「已发消息」当成「已安排」。
- **改范围（扩量 / 收窄 / 停做某部分）不用 SendMessage，改用 TaskStop + 重派**：范围更新「以本条为准」送不进运行中 agent 的注意力。§30「给出完整新边界」管措辞。新范围会让在途产出作废时：`TaskStop` 原 agent → 新 prompt 写明「已有产物在 <目录>，保留不删，在其基础上按新范围做」→ 重派；新范围只是在原产出上加一项、且原产出仍有用时，才可等原 agent 回报后再派一轮。
- **给已回报完 / 空闲的 agent 追加任务更不可靠**（消息可能送达但 agent 做一半停住）。有分量的新活重派一个新 agent 或主会话自己干；只把「顺手捎带」级别的小事交给唤醒。
- **派发 prompt 里的需求词有歧义就先定义**：prompt 写明该词在本仓的所指（如「官方协议 = 平台 protocol 为 anthropic 的官方直连」）与回退口径；仍有多解的，让 agent 回传解读由主会话 ask-me，用户的澄清等 agent 回报后连同答复一次性发新 prompt。
- **落盘约定（路径 + 文件名规范）也要贴进派发 prompt**：agent 不读 `scratch-规范.md` 等全局规则文件，prompt 写死的目标路径 agent 会原样照抄。要求 agent 落盘新文件时，prompt 里直接给最终中文名（research 类：中文文件名、词间 `-` 连），不给将来要改的过渡名。
- **prompt 复述规则或闸门时，引用票面 / CLAUDE.md 原文加 file:line，不凭记忆概括**：凭记忆写的「唯一 / 只有 / 一律」类断言，最容易和票面里后定的口径冲突（prompt 与票面各写一个闸门名时，agent 取两者交集）。agent 侧：prompt 与票面冲突时按票面执行；涉及安全或隐私闸门时取更严的交集，并在回报里写出冲突点，不自行放宽。

## 3. 派发 prompt 里的路径先验存在，agent 侧留自检指令

派发 prompt 本身也会产出乱码（生成流劣化的一种，见 surgical-refactor.md §3）：路径写坏、引用不存在的文件名，肉眼读像合理的。agent 靠 prompt 里预置的一句自我修正提示才接上。

1. 派发前把 prompt 里出现的每个路径 / 文件名 `ls` 核一遍存在性（相对主 checkout 用绝对路径）；
2. 派发 prompt 尾部固定加一行自检指令：「若本 prompt 引用的路径 / 文件名在仓库中不存在，先自行核实真实路径再动手，不按 prompt 原文照搬」；
3. **多 worktree 并存时 prompt 全文内联，或写明唯一绝对路径并明令「禁入其它 /tmp/aidog-* 目录」**（/tmp 下的 `aidog-*` 是多轮共用的命名约定，同名文件撞车是常态不是意外，agent 会顺着同名文件走进别的 worktree）；
4. **prompt 里的清单会被生成流劣化截断**（条目名被截后肉眼读起来像完整）：写 prompt 时数一遍条目数、把 N 写进 prompt（「清单共 N 项」）；agent 侧自检指令加一句：「清单条数与磁盘实际（`git status` / 目录列表）不符时以磁盘为准，不按 prompt 原文照搬」。

## 4. harness worktree 隔离的 agent：开工前核对基点

worktree 基点落后主 checkout 时，agent 在旧代码上写补丁、门禁全绿；合并时 `git apply --check` 上下文漂移失败，`git apply -3` 落出错位 hunk，被迫手工按 HEAD 新结构重解 + 全量重跑门禁。

- **agent 在 worktree 跑的门禁 ≠ 主 HEAD 上的门禁**，门禁结论不随补丁迁移。
- 派活后开工前核对基点：`git -C <worktree> rev-parse HEAD` 对比主 checkout HEAD；落后就先 rebase / 重建 worktree，再让 agent 动手。agent 运行期间主 checkout 前进（别的会话提交）同样造成漂移——合并前再对一次，不等开工时那一次管全程。
- **基点落后会让 agent 停下提问**：派发 prompt 写明目标基点 sha，并要求「开工先 `git -C <worktree> rev-parse HEAD`，不等于该 sha 就先 `git merge --ff-only <sha>` 或 rebase，再动手，不停下提问」。
- 基点已落后且补丁已产出：按 shared-worktree.md「采信半成品」三前提处理——逐 hunk 重审 + 主会话全量重跑门禁，不采信 agent 侧门禁结论。
- **数据落库与代码票并行时，基点漂移的是数据**：agent 基点下写的「某数据缺失 / 为 0」类断言，可能被主会话随后的落库提交推翻，合并必红。采信前对每条这类断言，`git log <agent 基点>..HEAD -- src-tauri/defaults/registry/` 查基点后有无数据提交；有就改按真值断言，或让断言自带 stub 数据不依赖 registry 现状。

## 5. 多补丁流水线：`git apply --check` 通过 ≠ 已应用

- 「检查」和「应用」走同一条命令：`git apply` 本身先检查后应用，失败即停；不单独跑 `--check` 把应用留到之后。
- 应用完立即 `git status --short` 点数确认改动落盘，再转向下一个补丁——「检查过」不占「应用过」的位置。
- **补丁里的新文件**：agent worktree 里新建的未跟踪文件不进 `git diff`，导补丁前先 `git add -N <新文件>`（或让 agent 回报新文件清单）；主会话 apply 后 commit 前，新文件也要 `git add`，否则 `git commit -- <路径>` 报 pathspec 未跟踪而失败。

## 6. 多 agent 并行审计：结论互相冲突时主会话实证定论

多 agent 分域审计可能对同一事实给出直接矛盾。主会话 grep 调用点后才能定论。

- **agent 对同一事实给出矛盾结论时，不按多数票、不按先到先得采信**——主会话自己 grep / 编译验证一手定论，再动手。agent 分域审计时每个 agent 只看得到自己的域，跨域事实（谁调用谁、死码判定）本来就是它们的盲区，矛盾是常态不是意外。
- **「死码」这类影响删除类改动的断言，删后编译 + 全量门禁绿才算证据**；删除量大时主会话逐 hunk 抽查 diff 归属。
- 交叉验证是必做步骤，不是可省的收尾。合并多 agent 报告时显式找「两份报告对同一符号/文件的不同说法」逐条对账。
- **reviewer agent 的发现同样先实证**：「双源 / 不对称 / 死码」类发现，采信前 grep 两侧调用链到同一个源头再决定修不修。
- **reviewer 报的「数据缺失」类断言，动手补之前先对官方/一手原文**：数据对错的一手真值源是官方原文，审阅 agent 的比对结论是下游，冲突时以官方原文裁决。

## 7. agent 门禁结论的假绿盲区

agent 可能只跑部分 target 或 crate，导致子集门禁全绿、完整门禁失败。**「agent 报绿」和「门禁真绿」之间隔着命令面和 crate 面两个盲区，口头回报不构成证据。**

- **新增 `test_*.rs` 文件必须接线 `mod` 声明**。新增测试文件的回报加一条证据：`grep -rn 'mod <文件名去.rs>' <crate>/src/` 命中挂载处；机械门禁已落地：`scripts/check-orphan-tests.mjs`（walk 全部 test_*.rs 对照 mod 声明，已进 `make lint`）。
- 派发 prompt 的门禁命令写全文不写缩写：clippy 一律 `cargo clippy --all-targets -- -D warnings`，要求 agent 回报贴命令行原文 + warning/failed 计数；修完必须重跑同一条命令再报。
- **子集门禁的范围按数据流向定，不按改动 crate 定**：spec 改共享层（写入层——列加密、列改格式、写入语义变更）时，验收范围 = grep 该列/表名找所有 select 它的测试所在 crate（`grep -rn '<列名>' src-tauri/crates/*/src/ --include='*test*'`），grep 一圈比事后跑全量门禁便宜。
- 主会话采信 agent 门禁前自己复跑一次（合并轮的「全量重跑门禁」三前提已含此条，无 PR / 直合路径同样成立）。
- 门禁输出禁纯 grep 过滤 test result：命令模板改 `2>&1 | tee .tmp/<任务名>/gate.log | grep -E 'test result|error'`——全量落文件，屏幕只留摘要，flaky 复现时失败名还在。
- **`&&` 门禁链上禁裸 grep 计数**：`cargo clippy 2>&1 | grep -cE '^warning|^error' && cargo test ...` 零 warning 时 grep 退出码 1，`&&` 后面的 cargo test / tsc / vitest 全部静默不跑——输出看似有结果实为残缺。计数类 grep 补 `|| true`，且链尾固定 `echo GATE_CHAIN_DONE`：回看后台输出先找 MARK，不在场 = 链没跑完，中间结果再多也不算门禁跑过。
- **裸 `cargo test` 只跑根包，是 workspace 子集假绿**：本仓 src-tauri 是 cargo workspace，`cargo test` 不带 `--workspace` 时 crates/* 成员（aidog_core / aidog_db 等）的测试一个不跑。票 01 门禁照写根包版，根包 14 passed 全绿，成员 crate 的直通测试实际没跑——agent 自行补跑 `--workspace` 才暴露。票面 / 派发 prompt 的 Rust 门禁一律写 `cargo test --workspace`；agent 拿到短命令自行补全并在回报注明。
- **执行段可用 nextest 提速（实测 138.6s→64s，`.tmp/test-speed/bin/cargo-nextest run --workspace`）；cargo test 语义仍是真值源**——nextest 不跑 doctests，配置在 `src-tauri/.config/nextest.toml`（retries 0 不变，slow-timeout 提前到 30s 仅标记）。make lint 等门禁链不动。
- **MARK 在场 ≠ 链跑完**：`grep ... | head; echo MARK` 结构把 cargo 中途失败吞掉，MARK 照打，cargo 编译期失败中断时也照打。门禁链除 MARK 外再核一个数：`test result: ok` 计数 = suite 总数；开工前在干净 HEAD 数一次期望值写进派发 prompt（<N> 随 crate 增减变，不当常量）。
- **门禁脚本名从 `package.json` scripts 抄，不凭记忆写**：同族脚本名如 `check:registry` / `check:modal` / `check:tokens` / `check:rpc` / `check:docs`。

## 7b. 门禁红先问「是不是我顺手改的」

范围外改动可能打破既有断言。门禁出现新红，先 `git diff` 找请求范围之外的 hunk，回退它再重跑；不先改测试断言去迁就。

## 8. 派 agent 前先数在途数

后台 agent 同时不超过 2（全局约束），多个并行 spec 各要 code-review 时会串行排队；**派之前先数在途 agent 数**，排队会饿住关键路径（如 review 挡合并）时，把顺序重排或将轻票收给主会话自己干。

- **在途账本与实际占槽同步**：agent 回报并 TaskStop 后，主会话当场把任务状态文件里该 agent 的占槽条目标成已释放——TaskStop 不清账本，派发 hook 按自己的账本计数且条目约 4 小时才过期。被「并发 subagent 上限 N 已满」拦时先核对账本：条目对应 agent 已回报 / 已结束却未释放的，释放后立即重派，不干等自动过期，也不硬重试。
- **用户催促 / 说「继续」时同样先数在途**：先 ListAgents 核对在途，同任务只留一个收口方，收口 agent 还在跑就等它，不为催促新开重复代理。
- **同一工作区同一时刻只派一个写 agent**：派发前按工作区（主 checkout / 各 worktree）记归属账：一个工作区一个在途写入 agent；要并行就开新 worktree（shared-worktree.md 文首「有必要」三情形），不往同一工作区塞第二个。

## 9. 整体 revert 旧提交撤功能：逐 hunk 归属复核

整体 revert 会连同旧提交里非目标改动一起删（如复制平台、侧栏条目）。

- 合并前派复核：对 revert diff 逐 hunk 标「属目标功能 / 属别的功能」，后者即误删，修回。
- 以可达性证据收口：被删符号的其它入口仍在（该轮 `PlatformEditForm.tsx:287`、`usePlatformsState.ts:704`），不凭「diff 看着没问题」。

## 10. 门禁红先跑基线对照再归因；worktree 内 build:web 的限制

- 升级依赖后 `yarn build:web` 红时，直接 detach worktree 到升级前 commit、独立 `yarn install` 跑同一命令（perf-testing.md §7 双版本对照）；基线同红 = 预存，记遗留，不算升级回归。
- `build:web`（next/turbopack）在 harness worktree 里可能因 `node_modules` 软链找不到 next（`web/next.config.ts` 的 `turbopack.root`）。验证在主仓跑；非要在 worktree 跑，临时改 `turbopack.root` 并在提交前还原。

## 11. 等别人的门禁日志：禁 `tail -F | grep -m1`，用 node 等待脚本

`tail -n +1 -F <log> | grep -m1 GATE_CHAIN_DONE` 命中后，日志已写完、tail 不再写入就收不到 SIGPIPE，管道永不结束，agent 的 Bash 一直挂到超时。改用 node `fs.watchFile` 等待脚本，命中判据后自己 `process.exit(0)`、不留后台子进程；派发 prompt 直接贴脚本，不贴 shell 管道写法。

- 命中判据三条同时满足：行数 > 5、含 `make lint` 的 `EXIT` 行、末行为 `GATE_CHAIN_DONE`。日志里残留的单行 `GATE_CHAIN_DONE` 会让「日志含标记」误判。
- 清理残留 tail 按 PID 核对命令行（`tail … .tmp/*/*.log`）再 kill，不用 pkill（perf-testing.md §1）。

## 12. harness worktree 里 `check:docs` 恒红：主仓补跑

worktree 缺 docs 子包依赖（`typescript-eslint`）时，`yarn check:docs` 恒 EXIT 2，与改动无关。

- 派发 prompt 写明：worktree 内 `check:docs` EXIT 2 属已知，不修、不复跑；其余步骤全 0 即交付。
- `yarn test:statusline-golden` 同理：`scripts/statusline-golden/build.mjs` 不读 `CARGO_TARGET_DIR`，在设了独立 `CARGO_TARGET_DIR`（`.worktrees/.target`）的 worktree 里 ENOENT，只在主仓跑。
- harness 隔离 worktree 里的 agent 对主仓的 git 操作可能被 harness 拒绝；合并一律派单独的合并 agent 在主仓做。基点落后 master 时用 `merge --no-ff`，并在主仓重跑完整门禁（agent 侧测试数不作数）。
- 合并 agent 在主仓补跑 `check:docs`，改了 command 签名 / 删 command 的票还要重新生成 docs（先例 bf3ba6cd8）。
- 新增 / 改签名 `tauri_command!` 的票，其门禁必含 `yarn check:api-catalog`（已在 `make lint`，无 CI、无 pre-commit，不手跑就没人跑）；红则 `yarn gen:api-catalog` 后一并提交。

## 13. 已回报的 agent：回报转述后当场 TaskStop

completed 的后台 subagent 不会从列表消失，界面一直显示最后一步。

- 回报转述完当场对该 agent `TaskStop`；返回 `not running` 或 `No task found` 都说明该 agent 已结束，无需再试。
- 空闲 teammate（已回报的 SendMessage 型）发 `shutdown_request`。
- 派发前数在途数（§8）时，只算 running，不把 completed 算进去。
- **idle 但报告未达：先查产出已落盘再催收，不重派**：agent 长时间 idle 且无回报时，先 `ls` 它应落盘的产出文件（报告路径在派发 prompt 里写死），文件在 = 工作已完成只是回报链路丢了，SendMessage 催交；不在才走 §15 中断 checklist 续做。重派会重复取证 / 实现。
- **停止后通知仍会来，忽略即可**：TaskStop 掉的 agent 可能送来重复回报 / idle 通知，忽略，不回信、不重派、不再 TaskStop。

## 14. 渲染类 UI 票：门禁全绿不等于视觉验收

图表、画布和动效类改动可能有布局比例、遮挡、空值、颜色可读性与动效缺陷，门禁无法替代截图验收。

- 图表 / 画布 / 动效类组件票，验收标准含「Playwright 真渲染深浅两 mode 截图 + 缺陷清单逐条核」（取证法见 ui-color-bug-verify.md 规则 2），截图落 `.scratch/<feature>/`；票面没有截图验收行不算完成。
- 主会话合并前读一遍截图，按比例、遮挡、空值、对比度、动效五项自查；真机 WebKit / reduced-motion 未覆盖的列入回报「人工验收 N 条未执行」（pr-merge-discipline.md §2）。

## 15. 门禁退出码不进管道；agent 中断后的续做 checklist

- **门禁命令禁管到 `tail` / `head` 再取退出码**（退出码与 `Tests` 行一并丢失）。命令写 `{ cmd; rc=$?; echo EXIT=$rc; } > .tmp/<任务名>/x.log 2>&1`，再 `grep -E 'Tests |Test Files' .tmp/<任务名>/x.log`；派发 prompt 把这条写死。
- **会话中途退出会让后台 agent 中断**。续做按此顺序核对现场，再派续做 agent：
  1. 票 `Status:` 是否停在 `claimed`（未写 `## Answer` = 没结票）；
  2. `git -C <worktree> status --short` / `diff --stat` 半成品范围，最后一次编译错误原文；
  3. 主仓 `status --short` 有无误写副本（与 worktree `cmp`）；
  4. 锁残留：`/tmp/aidog-cargo.lock`（见 shared-worktree.md）、`.git/index.lock`；
  5. 半成品含 JSON / 数据文件先跑语法门禁（`yarn check:defaults` 覆盖 `src-tauri/defaults/` 全部 JSON）再续做——非法 JSON 可以躲过编译与既有门禁。半成品采信走 shared-worktree.md「implementer 挂掉后的半成品采信」三前提。
- **派数据 / JSON 批量编辑给 agent 时，prompt 附确定性脚本全文**：prompt 只写「精确文本插入」描述、让 agent 自己拼编辑脚本时，agent 侧同样发生生成流劣化（surgical-refactor.md §3 同模式），agent 被中断后脚本垃圾直接落进数据文件。改法：把删垃圾 + 锚点插入的 python/node 脚本全文写进 prompt，agent 只负责跑与回报校验输出。
- **harness 并发上限是环境变量 `CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS`（官方默认 20，需 Claude Code ≥ v2.1.217；ultracode / resume 已结束 agent / /subtask / workflow / teammate 不受此限）**：用户说「不要管限制」只解除本项目的 ≤2 约定，不改 harness；第 3 个派发报 `Concurrent subagent limit reached` 时不重试，任务排队，任一完成再派。
- **主仓 `node_modules` 缺失**（成因不明）：agent 在主仓补 `yarn install --immutable`，装完确认 `git status --short` 无 `yarn.lock` 变动；再出现时记当日账本遗留，累计两次再查成因。
- **TDD 与截图豁免要写进票面**：修 bug / 加校验的票，agent 回报须贴先红的失败输出；豁免截图须在回报写明「渲染未变」依据（diff 不含 JSX / 样式），否则按 §14 列人工验收。

## 16. 先红输出全量落文件，禁 `head -N` 截断

新增测试的「先红」证据：`{ cmd; rc=$?; echo EXIT=$rc; } > .tmp/<任务名>/red.log 2>&1`，再对每个新测试名各 grep 一次 `FAILED|failed|<测试名>`；管到 `head` / `tail` 会截掉新测试的失败行，无法确认它先红过。

## 17. 门禁里的计时断言偶发红：`--no-fail-fast` 取全量，重跑定性

`cargo test --workspace` 遇一个 suite 失败就停，后面的 suite 不跑，拿不到全量结果。门禁命令写 `{ cargo test --workspace --no-fail-fast; rc=$?; echo EXIT=$rc; } > .tmp/<任务名>/x.log 2>&1`。计时断言（如 `aidog_stats::test_usage_stats::flow_breakdown_7d_scale_guard`）首轮红先单跑该测试一次：绿 = 负载抖动，记账本遗留；仍红才按回归处理。同族挂钟断言：`aidog_core::gateway::proxy::test_integration::mock_platform_ttft_and_inter_chunk_split`（`test_integration.rs:1366`，ttft 阈值 40–160ms、chunk 间隔 ≤60ms，已加预热请求兜底）。再红先查机器负载 / 内存压力，不当回归。

## 18. 网络断连中断、先红严格性、票状态词、带密钥写命令登记

- **SSL / 断网中断是常态**：续做走 §15 的核对现场清单；采信半成品的 commit message 注明 `adopted from interrupted agent`，门禁整条重跑。派续做 agent 时 prompt 写明「前一 agent 因断网中止，先 `git diff` 逐 hunk 核对再接着做」，不让它从头重写。
- **先红要严格**：先只写测试跑出红（`{ cmd; rc=$?; echo EXIT=$rc; } > .tmp/<任务名>/red.log 2>&1`），回报**贴 EXIT 值与失败测试名**，再写实现；确实做不到（测试与实现不可分）写明原因并改用「改坏实现→红→按行号还原」反证，回报标「非严格先红」。回报只写「先红已验」而无 EXIT 值和失败名，按没验处理。**红必须是断言红，不是编译红**：先加桩（新符号只给签名 + 默认返回值，夹具补全）使测试编译通过，再跑出断言失败。review 修复补的测试同样适用：派 review-fix agent 时 prompt 照贴本条。
- **`make lint` 整条跑**：收尾门禁以 `{ make lint; rc=$?; echo EXIT=$rc; } > .tmp/<任务名>/lint.log 2>&1` 为准，分项绿不算；收尾 agent 回报贴 EXIT 值，没跑写「做了没验证」。**agent 级派发同样适用**：改动含前端文件（tsx / css / locale）的修复类派发，prompt 门禁清单写 `make lint` 整条，不枚举 cargo 子集——字号下限 / tsc / biome 都不在 cargo 子集里，cargo 全绿证不了前端门禁。
- **绕开脏文件另建模块只是权宜**：占用解除后并回原模块，账本遗留写明并入条件；不把「文件被占」当拆文件的设计理由。
- **票 `Status:` 只有 `claimed` → `resolved`**：与 wayfinder-session.md §2 同口径，派发 prompt 三行词表照贴；合并后当场 grep `Status:`。
- **新增带密钥的写命令，同提交登记 `RESTORERS`**（`aidog_rpc/src/secret_fields.rs:114`）：派发含新增写命令且涉及 `SECRET_PATHS` 字段的票时，prompt 写明此登记要求，review 时对照检查。建议做成机械检查：「`SECRET_PATHS` 涉及的写命令 ⊆ `RESTORERS`」测试（未做，属代码改动；重审触发：再新增带密钥写命令）。

## 18. 门禁日志与构建目录收回仓库

- 上文各节的 `/tmp/<名>.log` 一律落仓库 `.tmp/<任务名>/<名>.log`（`.gitignore` 已忽略），收工删该目录；命令前先 `mkdir -p .tmp/<任务名>`。**先红（`red*.log`）与门禁日志例外：实现 agent 不删**——留到 review 核对与合并完成后由收尾 agent / 主会话清；派发 prompt 交代清场边界时带上这句。
- 上文 `CARGO_TARGET_DIR=/tmp/aidog-agent-target` 作废：主 checkout 用默认 `src-tauri/target`，仅 worktree 用 `<项目根>/.worktrees/.target`（见 shared-worktree.md「构建目录」）。

## 19. implementer 的门禁链一律前台同步跑

agent 把门禁链挂后台 Bash 自己 idle 等「完成通知」时，链早已跑完（`.tmp/<任务名>/gate.log` 里 `GATE_CHAIN_DONE` 在场）却没人唤醒，白白挂起。

- **执行 agent 的依赖性命令前台运行**：门禁结果、编译产物或锁获取是下一步输入时，Bash 前台运行并设足 timeout；若 harness 自动转后台，主会话接收完成通知并读取日志，再通知 agent 续做。执行 agent 不得自行挂后台后 idle 等通知。
- 主会话发现执行 agent idle 且 `.tmp/<任务名>/gate.log` 已有完成标记或退出码时，读取日志并 SendMessage 唤醒续做，不重派。
- **主会话侧只读门禁优先主对话后台 Bash 自跑**：门禁是纯验证、红了也不由它修时，主对话直接后台 Bash 跑（`{ cmd; rc=$?; echo EXIT=$rc; } > .tmp/<任务名>/gate.log 2>&1`，退出码与命令输出写入同一日志；若自动转后台，主会话接收完成通知后读日志并通知续做），比派专职门禁 agent 少一层唤醒 / 回报链路，EXIT 与日志直接可读；门禁红需要改代码才派 implementer。这是对全局「主对话只调度」的项目级收窄，仅限只读验证类命令。

## 20. 本机代理测试假红与下载故障记录

**测试方向**：`cargo test --workspace` 在设了 `HTTP_PROXY` 的本机上会出与本改动无关的假红，去掉代理变量后全绿。门禁命令前缀 `env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy`，派发 prompt 照贴；全量跑偶发红、单跑绿的，按 §17 先单跑定性，仍未归因的记账本遗留。

**下载方向**：直连包管理源偶发 `ENOTFOUND` 属当轮环境记录，不构成 DNS 故障的普遍归因或代理、镜像配置建议。git push 的 SSL 失败与 GitHub 合并结果分别核验，规则见 pr-merge-discipline.md §5。

## 21. 「加功能」票派发前先审计存在性；外部协议字面量的来源分级

先例：派发 prompt 照 CLAUDE.md 旧文写「新增」，实际 `passthrough.rs::group_model_ids` 早已按分组返回、`ENV_VAR_DEFS`（`src/services/claude-settings-schema.ts:340`）早有该条目，重复加条目只有 `yarn test` 拦下（CLAUDE.md 对应条目已过时）。

- 与 wayfinder-session.md §14 同源：派「新增 X」的实现票前，主会话先 grep 代码确认 X 不存在（枚举 / 表 / 常量表 / 路由）；已存在就把票改成「补差额」，prompt 写明现状 file:line。
- agent 开工第一步同样 grep，发现已存在按「补差额」做，回报写「已有 / 本次补」；CLAUDE.md 文字与代码不符时顺手改文并在回报列出。
- 往常量表（`ENV_VAR_DEFS`、registry 清单这类）加条目前先 grep 键名；门禁必含 `yarn test`，它是重复键的唯一拦截点。
- 官方文档未列的协议字面量（头名等）若取自逆向 Claude Code 二进制：账本 / 回报标「逆向，版本 X」，代码常量处注明版本，遗留写复活条件（官方文档补列或改名时回头核对）。逆向值不得冒充第 1 类出处。
- clippy 易踩两处：`collapsible_if`、`MutexGuard` 跨 `.await`（先取值后释放锁再 await）；门禁照旧 `--all-targets -D warnings`。
- **「给 X 加开关」票派发前 grep 开关字段的 Rust 消费者**：零消费者 = 假开关，UI 只是摆设。此时先拍板执行语义（超限挡不挡、挡在哪一层）再动 UI；反方向同样成立——给已有 UI 开关接执行逻辑的票，prompt 写明现状 file:line。
- **修数据类字段后 grep 消费端；grep 证据直接写进派发 prompt**：零消费者 = 假修复，修完数据 grep 读取端取 file:line；主会话把证据原文写进修复票派发 prompt，agent 从证据开工、不重复取证。
- **「入口 / 控件加在某组件上」票派发前 grep 该组件的生产渲染点**：存在性审计面从后端符号（枚举 / 表 / 常量表 / 路由）扩到 UI 挂载点：票面点名的每个组件先 grep `<组件名` 生产渲染点，仅测试引用的组件不算挂载面（按钮落在零渲染点的组件上，应用内不可达）；落点组件有渲染但操作排已满（固定宽列 / grid 溢出）时，票面写明布局约束与替代落点，不硬塞。

## 22. 先红只算新行为测试；断言「AiDog 写了 X 文件」先 grep 写入点

- 在旧代码上已绿的新增测试只是回归护栏，不算新行为证据。回报把新测试分两列：「先红（贴 EXIT 与失败名）」/「回归护栏（旧码已绿，不算新行为证据）」；护栏不计入「先红已验」。非严格先红按本文 §18 标注。
- 断言「AiDog 会写 / 会改某用户目录文件」前，先 grep 写入点（`sync_settings.rs` 等的路径常量）取到 file:line，取不到标 `推测:`。

## 23. 前端先红：先渲染 testid 空桩；渲染类票截图不得「为省时」跳过；非英文 locale 机翻要复核

- 前端票的桩 = 先在组件里渲染带 `data-testid` 的空壳元素（无行为）使测试能取到元素，再跑出断言失败；派发 prompt 照贴这句，回报贴失败信息须是 `expect(...)` 断言而非 `null` / 找不到元素（与 §18「红必须是断言红」同口径）。
- 渲染类票（新增 UI 元素 / 提示 / 开关）截图验收不可因时间豁免，只允许 §15 的「渲染未变」依据豁免；否则回报列「人工验收」。
- 新增文案票回报列出新增 key 清单，主会话抽核至少 zh-Hans / en-US 原文与一种 RTL（ar-SA）显示，其余机翻在账本遗留标「未复核」。

## 24. 网关代执行类票：逆向值核当前版本；「接管 / 跳过模型」spec 先列混合形态；worktree 合并前看盘

- 逆向值随版本漂移（ADR 0012 记的版本开工时已过期）：派逆向值相关票前 `claude --version` 对一次，prompt 写当前版本；版本变了就重核字段再用（补充 §21 逆向条）。
- 「命中即不调模型 / 整体接管」的 spec 要列混合形态并拍板：spec 阶段逐条问「请求同时带别的工具 / 别的块时是接管、拆分还是放行」（同 wayfinder-session.md §11 等效退化形态）。
- 先红覆盖要逐类核：改坏反证须每个断言类别各改坏一次，仍绿的类别补断言再交付。
- 合并前先 `df -h .`，余量不足先报主会话（删缓存要授权，shared-worktree.md），不带着满盘跑全量门禁；合并后当场删 worktree 与其 target。

## 25. 设计稿类 agent 不开隔离；并发账本对失败 agent 不释放；自检零命中不算缺失

- **只产 `.scratch/` 设计稿 / 报告、不改代码的 agent 派发时不带 `isolation`**：隔离 agent 的 Write 工具被限在自己的 worktree，写主仓 `.scratch/<feature>/design-demos/` 只能先写 worktree 再 `cat` 拼回主仓，多一步且易留副本。隔离只用于 shared-worktree.md 文首「有必要」三种写码情形。
- **并发上限账本（`src-tauri/crates/aidog_hooks/scripts/aidog-subagent-cap.mjs`，状态在 `~/.aidog/subagent-cap/<session_id>.json`）对 failed / 被 kill 的 agent 同样不释放**，与 §8 的 TaskStop 情形同病。被拦且确认在途 agent 已全部结束时，跑 `node src-tauri/crates/aidog_hooks/scripts/aidog-subagent-cap.mjs --reset <session_id>` 清零后重派。`--reset` 清的是该会话全部登记，仍有 running agent 时不跑（会让在途数少算）。
- **agent 用 grep 字面词自检「某内容没保留」得零命中，不构成缺失结论**：主会话采信「缺失」类自报前，按内容本身（具体数据值、节点名）再 grep 一次（同 registry-data-edit.md「grep 零命中只说明 pattern 不匹配」）。

## 26. 规划 / 调研 agent 防断连丢结论；探索报告转交；前序数字先复核

断连常发生；结论攒在上下文里、零落盘时断连即全丢。§18 管实现 agent 的半成品，本条管规划 / 调研 agent。

- **派发 prompt 写落盘节奏**：「开工 5 次工具调用内写出 spec / 票骨架到目标路径，之后每得一条结论当场写进文件」。断连后对同一 agent `SendMessage` 续做，已落盘部分保留；不重派从头来（长 HTML 同理，见 surgical-refactor.md §3.5）。
- **同一 agent 连断 3 次**：按「同一问题连续三轮」停手，用 ask-me 问用户续做 / 重派 / 换拆法，不自行硬重试。
- **探索 agent 的报告走 subagent JSONL 文件传递**：下游 agent 要用时，把 JSONL 复制到 `.tmp/<任务名>/`，让下游自己写脚本（落盘成文件再跑）提取最后一条 assistant 消息；报告全文不经主会话转贴。
- **前序探索报告的数字是假设**：规划 agent 的 prompt 写明「探索报告的计数、调用点、死码判断先 grep 复核，再写进 spec」（§6 同口径）。

## 27. 状态码 / 错误码规范化票：派发 prompt 带语义表与 RFC 依据；`&&` 门禁链不丢 cargo 退出码

- 派发 prompt 列「失败原因 → 状态码 + RFC 9110 依据」表（限频 429、审核 451、无实现 501、无该类资源 404、白名单 403、内部读库失败 500），写明可用 4xx/5xx 之外的码；表由主会话先拍板，agent 只实现。
- 改状态码必同轮改：全部断言该码的测试名（旧名含 `400` 的会误导）、CLAUDE.md 对应描述、locale 文案；回报列出未同步项。
- 门禁写 `{ cargo test --workspace --no-fail-fast; rc=$?; echo EXIT=$rc; } > .tmp/<任务>/t.log 2>&1`，回报必须带 EXIT 值；只贴 `passed` 计数不算。
- 跑到一半 `No space left on device`：先报主会话，不删 `target`（别的会话已整体清过一次）。

## 28. 子 agent 保持单层；中断续做先核现场；收口前独立审查

- **被派出的 agent 自己完成获派工作**：不再调用 Agent 派子 agent。发现任务拆分或并行能省时，先把建议、依赖关系和当前现场回报主会话，由主会话决定是否另派。派发 prompt 写明「你是执行 agent；直接完成任务，不派生子 agent；需要拆分时回报主会话」。
- **429 或会话中断后的续做先验现场**：接手 prompt 指明中断原因，并要求先核对工作区状态、diff、最后报错、未跟踪文件和既有门禁日志；逐 hunk 判断已完成内容与待办，再接着做。不得重写已有改动或沿用未经复核的旧门禁结论。回报已采信的改动、未完成项及重跑的验证输出。
- **收口前安排独立审查**：实现与修复完成后、最终门禁和交付前，派独立审查 agent 对照原始需求 / spec 检查行为与边界；修完审查发现后再跑相关门禁。审查报告逐项标明依据和处置；「按 spec 属拍板设计」等不修项须有对应条款依据。

## 29. 长门禁被 harness 自动转后台：EXIT 只认日志文件

长门禁被 harness 自动转后台时 agent 会丢 `EXIT=` 行。命令及退出码必须同写日志：`{ cmd; rc=$?; echo EXIT=$rc; } > .tmp/<任务>/x.log 2>&1`，并设 `timeout: 600000`。若 harness 自动转后台，主会话接收完成通知后读取日志；日志无 `EXIT=` 行即「没跑完」，再通知执行 agent 续做，不另起轮询。

## 30. 更新派发指令须裁决冲突；门禁前台运行给出正面命令

互相矛盾的派发更新、被 agent 忽略的「暂缓提交」（仓库自动提交规则照跑）、挂后台后 idle 等通知（§19 的禁止项未阻止复发）都属此类。

- **更新派发范围时给出完整的新边界**：每条会覆盖先前派发内容的更新，都写明「以本条为准」、完整的当前允许动作与暂停动作；同时核对它是否与用户当轮明确授权、项目级授权冲突。明确写出冲突及裁决依据；无法确定时暂停冲突动作并回报主会话，不自行选择旧授权或新指令。
- **用户明确暂停某动作时，执行 agent 立即暂停该动作**：包括仓库规则原本自动授权的提交、推送或建 PR。主会话更新授权后，才恢复该动作；不得把此前授权当作继续执行的依据。
- **执行 agent 的依赖性命令前台运行**：门禁、编译、取锁等命令的结果供下一步使用时，给出完整命令与足够长的 Bash timeout（如 `600000`）。命令及退出码须同写日志：`{ cmd; rc=$?; echo EXIT=$rc; } > .tmp/<任务>/x.log 2>&1`。若 harness 自动转后台，主会话接收完成通知后读日志并通知 agent 续做；agent 不自行 idle 等通知。

## 31. 同会话多 agent 并行：锁等待、归属、临时目录、审计结论时效

- **cargo 锁被占时等锁，不杀等待循环**：取锁用 shared-worktree.md「构建目录」节的 node 轮询脚本，等到为止；锁超过合理时长再按该节判据（无 cargo 进程）处理，不绕过锁直接跑 cargo，也不放弃重跑。
- **脏文件归属先核对再下结论**：说归属前先对照主会话派发清单与 `git status --short`；清单内的 agent 改动属本会话，不当外来脏文件回避或上报。
- **`.tmp/<任务名>/` 清理前核对在用者**：收尾删目录前确认同任务全部 agent 已回报；多个 agent 共用任务名时，各用子目录 `.tmp/<任务名>/<agent名>/`，只删自己的。
- **审计结论落笔前复核时效**：过期的「某测试每次必红」类审计断言会被当成现状引用。引用审计结论写进 issue / 规则前，用当前 HEAD 跑一次对应命令（同 §26「前序数字先复核」）。
- **zsh 变量后接 `:` 要加花括号**：`$B:src` 会被当作 `:s` 修饰符展开，写 `${B}:src`（例：`git show "${B}:src/x.rs"`）。

## 32. 执行 agent 的依赖性命令前台运行；自动后台由主会话接续

当前 harness 的后台完成通知只送达主会话队列，不唤醒执行 agent。

- 命令输出供后续步骤使用时，执行 agent 前台阻塞运行并设置足够 timeout；命令和退出码按 §29 一起写入日志。
- 若 harness 自动转后台，执行 agent 回报命令和日志路径后继续做不依赖该结果的工作；主会话收到完成通知后读日志，再通知 agent 续做。
- 取锁等待脚本同样前台运行。主会话若发现执行 agent idle 且日志已有结果，读日志后 SendMessage 唤醒续做，不重派。
- **规则两轮后仍复发，升级成派发 prompt 固定句**：派发 prompt 尾部固定贴一句——「依赖性命令（编译 / 门禁 / 取锁）前台运行并设 timeout: 600000，命令与退出码按 §29 写日志；harness 自动转后台时回报命令与日志路径、继续做不依赖结果的工作，不 idle 等通知」。主会话看到 agent 回报含「等后台通知 / 等完成通知」类字样，当轮 SendMessage 发 §32 的核查指令（读日志路径、有 EXIT 就唤醒续做），不重派、不干等。
- **固定句防不住 harness 自动转后台**：根因是 harness 把超长前台命令自动转后台、完成通知只进主会话队列——环境行为，固定句只能缩短唤醒延迟，不能消除 idle。结论：「idle → 读 `.tmp/<任务>/` 日志 → 有 EXIT 就 SendMessage 贴结果唤醒」是本规则的预期运行形态而非规则失效，不再据此逐轮重议或升级固定句；含长门禁的派发把一次唤醒计入流程预算。

## 33. 查进度走 SendMessage 原 agent；挂死 agent 的 TaskStop 接管判据

派新 agent 读会话状态文件查进度，回报的全是无关并行线信息（新 agent 没有原 agent 上下文）。挂死的 agent 在外呼命令上久挂，SendMessage 催收无响应。

- **查进度 / 催办一律 SendMessage 原 agent（或 ListAgents 看状态）**，不派新 agent 读文件。派新 agent 只在两处合法：接管挂死 agent（下条）、原 agent 已结束且要独立取证。
- **挂死判据与接管**：agent 在外呼命令（`gh pr merge` / push / 长门禁）上久挂（如 30 分钟无新调用），SendMessage 催收 1 次无响应（等约 10 分钟）→ `TaskStop` + 重派接管 agent。接管 prompt 照 §28 带现场核对清单（worktree 状态 / diff / 最后报错 / 日志路径），并写明「原 agent 已 TaskStop，从核对现场接续，不重做已完成步骤」。
- 接管 agent 与原 agent 汇报口径一致：贴命令原文 + EXIT 值，接管完成的合并照常走 pr-merge-discipline 的核对义务。

## 34. PII 钩子脱敏明文名：锚点改用可验证替代

用户的 PII 钩子会把工具输出与落盘文本里的数字 id、分支名、worktree 名、socket 编号等替换成 `{{PII_SECRET_*}}`。agent 照被脱敏后的明文名定位对象必然失败，或在回报里转述出无法使用的引用。

- 引用会话名 / 分支名 / worktree 名 / 数字 id 前先核对是否已被脱敏；被脱敏就换可验证替代锚：worktree / 分支用绝对路径 + `git rev-parse` 按 commit SHA 定位；盯 PR 新评论的 watcher 基线用最新评论的 createdAt 时间戳落文件，不用评论 id 明文。
- commit SHA、Unix 时间戳、绝对路径钩子不脱敏，是安全锚；明文编号一律视为可能漂移，落盘记录优先带安全锚。
- **Write / Edit 的 `file_path` 参数同样会被脱敏**。给文件起名避开数字 id / hex 段（用时间戳或纯词名）；已落盘的字面名文件按内容特征（文件内的标题、提问文本）grep 定位后改名，不照被脱敏显示的名字再拼路径。写完按既有条目用 node `fs` 读盘上真实字节核验文件名与内容。
- **自己发出的命令回显同样被脱敏**：不只工具输出，命令里的文件名 / 资产名 / 过滤词经脱敏后照常执行。因此 ① 写文件、写 URL 后用 node `fs` 读盘上真实字节（`includes` 断言）验证，不信显示层；盘上字节正确 ≠ 显示错误内容。② **过滤词被脱敏会让单跑命令变成 0 匹配空跑假绿**（exit 0 但一个测试都没跑）：单跑定性按模块名过滤并核 `N passed` 计数非零 + gate.log 带 EXIT 锚；同款假绿也发生在 worktree / 分支名被脱敏成占位符字面量写进命令时。

## 35. 派单点名单装不下真实范围时停手待裁；任务文本自相矛盾时写矛盾择一落地

- **派单边界写成点名单、清单不足以编译完备时，agent 默认停手待裁**：主动停手撤回全部超范围改动、写报告等裁决（「暂停-核对-回报」），好过超范围改动直接落盘后的返工。派发 prompt 可预写兜底句：「点名单之外发现必要改动时停手回报清单，待裁决；已做的超范围改动精确撤回」。
- **两个既有事实互斥时，实现 agent 写出矛盾并择一落地，不硬凑、不死等**：派单文本与既有测试 / 事实冲突（如「改名一律保留」与删后重写按内容数学上不可区分互斥，任一行为必二选一）时，agent 写出矛盾分析、择一实现并在代码注释 / 回报写明依据，主会话复审采信。派单前提被事实否定（物理不可能）时，以事实否定前提后按事实实现，不伪造数字。与 wayfinder-session.md §11（规划侧等效退化形态）同源，本条管实现侧。

## 36. 长循环 agent 防上下文超限：产物是脚本不是过程；接管先盘点落盘

第一支 agent 因 API 上下文超限挂死后，接管 agent 复用其已落盘的 mock 与截图脚本续做成功，没有重派从头。

- **长循环任务（批量截图 / 批量渲染 / 长流水线）的产物落盘成脚本，迭代改脚本重跑**：mock 数据、页面清单、注入逻辑全写进 `.tmp/<任务名>/*.mjs`，每轮只改脚本再 `node` 重跑；不把图片 base64、整份日志、大文件读进上下文——目检截图逐张走 Read 看文件，日志 grep 关键行（如 mock 未命中命令清单），整读只留 `wc -l` 级的小输出。派发 prompt 把这条工作方式写明（§26 落盘节奏条在长循环任务上的形态）。
- **接管挂死 agent 前先盘点 `.tmp/<任务名>/` 落盘产物**（§33 接管判据的前置步）：脚本与日志在 = 工作可续，接管 prompt 直接指向脚本路径续跑，只修脚本暴露的问题；落盘为空才重派从头。

## 37. 用户换「展示什么」先问形态还是列；大测试日志合并后压成摘要

- **用户说「X 换了，我想要展示 …」，派发前先确认要改的是视觉形态（图表 / 表格）还是数据列**：两种解读都成立时按 §2「需求词有歧义就先定义」走 ask-me，prompt 写明「保留图表形态，只换数据项」这类完整口径，不照抄原词。
- **全量测试日志在 review 与合并完成后压缩**：`cargo test --workspace` 全量日志可达 190MB，留在 `.tmp/` 占盘。按 §18，先红与门禁日志留到 review 核对与合并完成；之后由收尾方保留 `EXIT=`、`test result:`、失败测试名与 panic 行（`grep -E 'EXIT=|test result|FAILED|panicked' x.log > x.summary.log`），再删原日志。磁盘余量不足（<10G）需提前压缩时，先报主会话。
