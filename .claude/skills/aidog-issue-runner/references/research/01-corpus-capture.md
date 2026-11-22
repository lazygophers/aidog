# 语料采集（2026-11-22）

来源全部为本仓库一手规则文件（第 4 类可信源，file:line 见条目）。按「和先验不一样 / 先验没有」筛增量。

## A · PR 合并纪律（先验缺 → 新增）

来源 `.claude/rules/pr-merge-discipline.md`：
- §2：含真机行为的 PR，人工验收清单必须显式留在回报里，不同步账本 = 验收义务静默蒸发。
- §3：PR 正文改写后全篇扫过期状态词（draft / incomplete / 未验收…），整段替换不增量补丁。
- §4：审查 PR 用 `git fetch origin pull/<N>/head:refs/remotes/pr/<N>` 只读 refs，不 checkout；审查结论自动发 PR 评论（用户 2026-11-14 授权）；合并评论必须带三项门禁命令原文 + EXIT + 测试数 + 变异反证。
- §5：READY 绑定 40 位 head SHA；合并优先 `gh pr merge <PR> --match-head-commit <完整SHA>`；push SSL 失败后 GitHub 合的可能是旧版，合并后 `git diff <本地验证commit>..origin/master` 核差异；多 PR 同动 Cargo.lock / 同 crate → 对合并态全量测试后再清理。
- §6：对端会话不可达 → 记「待补发」（接收对象、内容、SHA、尝试记录）。

## B · push 预检与失败处置（先验缺 → 新增/纠偏）

来源 `.claude/rules/shared-worktree.md`：
- 「推送只走 origin」节：分支从 origin/master 开（2026-11-14 本地 HEAD 借 PR 发上他线提交实锤）；fetch 失败不回落本地 master（2026-11-15 实锤）。
- 「push 预检条件设计」节（issue 69 轮）：禁 `--is-ancestor origin/master <分支>`；预检 = `git rev-list --count origin/master..<分支>` 只含本票提交；预检失败正解 = `git fetch origin && git merge origin/master` 进分支解冲突后重跑。
- 授权推送只推授权时 sha：`git push origin <sha>:refs/heads/<分支>`。

## C · 中断续做与半成品采信（先验缺 → 新增）

来源 `parallel-implement.md` §15、§28，shared-worktree.md「implementer 挂掉后的半成品采信」：
- 断连（SSL / 429 / 会话退出）是常态。续做五查：① 票 Status 是否停 claimed；② `git status --short` + `diff --stat` 半成品范围 + 最后报错；③ 主仓有无误写副本（cmp）；④ 锁残留（`/tmp/aidog-cargo.lock`、`.git/index.lock` 判据）；⑤ 半成品 JSON 先跑 `yarn check:defaults` 再续做。
- 采信三前提：diff 逐 hunk 归票；全部门禁补跑绿（非子集）；commit message 注明 `adopted from interrupted agent`。判据只看「主会话能否独立验证到绿」。
- 同一问题连续三轮修复仍失败 → 停，ask-me。

## D · 派发 prompt 固定要素（先验缺 → 新增）

来源 parallel-implement.md §2/§3/§32、wayfinder-session.md §2：
- 规则/闸门引用贴原文 + file:line，不凭记忆概括（2026-11-21 consent 闸门票实锤：prompt 写「唯一闸门」，票面是另一个词）。
- 路径先验存在 + 尾部自检句「prompt 引用的路径不存在时先核实再动手」。
- 清单写死条数 N（prompt 清单会被截断）。
- 状态词三行照贴：开工 `Status: claimed`；结票 `## Answer` + `Status: resolved`；禁 done/closed。
- 固定句：「依赖性命令（编译/门禁/取锁）前台运行并设 timeout: 600000，命令与退出码按 §29 写日志；harness 自动转后台时回报命令与日志路径、继续做不依赖结果的工作，不 idle 等通知」。idle 复发是预期形态：idle → 读日志 → 有 EXIT 就 SendMessage 贴结果唤醒（§32 固定句后注）。
- 「新增 X」票先 grep 存在性（§21）；UI 票 grep 生产渲染点。

## E · 假红归因（先验缺 → 新增）

来源 shared-worktree.md「构建目录」节、parallel-implement.md §17/§20：
- extern 丢失 / doctest E0463 / 二进制消失 = 并发重建假红，先取锁独占重跑再归因，不在假红上改代码。
- 计时断言偶红：先单跑该测试一次，绿 = 负载抖动记遗留，仍红才按回归。
- 本机代理 env：`cargo test --workspace` 设了 HTTP_PROXY 时 4~5 测红；门禁命令前缀 `env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy`。
- 中途新红先看夹具近期提交（`git log --oneline -5 -- <夹具文件>`），别在自己 diff 里翻。
- 首跑红且无 panic / build-script 产物缺失：先 `touch src-tauri/crates/aidog_core/build.rs` + 重跑，再谈归因。

## F · commit 与提交面纪律（先验缺 → 新增）

来源 shared-worktree.md「硬规则」节、commit -m 条：
- 禁 `git commit -a` / 无路径 commit；pathspec 精确到文件。共改文件走 patch 过滤（diff → 过滤自己 hunk → `git apply --cached` → 核对 → 不带 pathspec commit）。
- commit message 带 `$` 用单引号或 `git commit -F`（2026-10-15 价格 message 被展开实锤）。
- 开工与提交前各跑 `/usr/bin/git diff --cached --stat`，不明来源暂存不动、不并入提交。

## G · 磁盘与产物（先验缺 → 新增）

来源 shared-worktree.md、parallel-implement.md §24/§37、perf-testing.md §10：
- 开跑前 `df -h .`，余量 <10G 不开跑；满盘先报主会话，不擅自删 target（删 target 也要持锁）。
- 门禁日志 `.tmp/<任务名>/`；先红与门禁日志留到 review + 合并完成后，压缩成摘要（保留 EXIT= / test result / FAILED / panicked）再删原日志。
- 临时产物全部收回仓库 `.tmp/`，禁 /tmp 新建（锁 `/tmp/aidog-cargo.lock` 例外）。

## H · 两套票轨（先验没理清 → 新增）

来源 `docs/agents/issue-tracker.md` 迁移注记：
- GitHub Issues 是开发任务入口（61/64/157/160 各轮）；`.scratch/` 本地票照旧并存。GitHub issue 开发按 shared-worktree.md issue 节；本地票按 issue-tracker.md。两者不互转。

## 误读清单（大众印象 vs 规则）

1. 「check:docs 在 worktree 红 = 我改坏了」→ §12：worktree 恒 EXIT 2 属已知，主仓合并时补跑。
2. 「agent 报绿 = 门禁真绿」→ §7：命令面（缩写/缓存）与 crate 面（子集）两个盲区，采信前主会话复跑。
3. 「push 失败 = 换本地基线重开」→ fetch 失败不是回落许可；预检失败正解是 merge origin/master 进分支。
4. 「门禁中途红 = 我的回归」→ 先查夹具近期提交与他人脏文件集。

## 和先验不一样的地方（≥8 条）

1. 推送预检禁 is-ancestor（纠偏：我原以为祖先关系检查是安全方向）——推翻。
2. READY 绑定 SHA + --match-head-commit（先验无）——新增。
3. 合并态复测条件（多 PR 同动 Cargo.lock）——新增。
4. idle 唤醒是预期运行形态而非规则失效（先验以为固定句能防住）——加深。
5. 中途新红先查夹具 git log（先验先查自己 diff）——加深。
6. JSON 半成品先过 check:defaults 再续做（先验无）——新增。
7. 删 target 也要持 /tmp/aidog-cargo.lock（先验以为删目录不属 cargo 串行）——新增。
8. 两套票轨并存且各走各规则（先验以为已统一）——推翻。
9. 人工验收清单必须显式留回报（先验只在 pr-merge-discipline 见过一句，未进 skill）——加深。
10. commit message 的 $ 展开不报错静默进历史——新增。

## 封存件

本 skill 的「一手事实基准」= 上述各条 file:line 出处。验证用机械核对替代封存题（引用路径/命令存在性 + 节名存在性），结论记 FIDELITY.md，标「未做封存题增量测试」。
