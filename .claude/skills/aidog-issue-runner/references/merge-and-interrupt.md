# push 预检、合并纪律与中断续做

## push 预检（shared-worktree.md「推送只走 origin」节 +「push 预检条件设计」节）

1. 预检查的是「分支相对 origin/master 只含本票提交」：`git rev-list --count origin/master..<分支>`。**禁用** `merge-base --is-ancestor origin/master <分支>`（origin/master 一前进该条件必失败，issue 69 轮实锤）。
2. 预检失败正解：`git fetch origin && git merge origin/master`（进分支解冲突）后重跑预检再 push。这是预检失败的标准处置，不是 push 禁令的例外。
3. push 只走 origin；直连失败记录环境结果，不构成操作建议（2026-11-15 代理 push 成功属当次环境）。
4. 授权范围以授权当时 sha 为界：`git push origin <授权sha>:refs/heads/<分支>`，不用裸 push。

## 合并纪律（pr-merge-discipline.md）

- READY 必须绑定完整 40 位 head SHA；READY 后 head 变更（作者合 master 解冲突等），旧结论作废：fetch 新 head、审增量 diff、必要测试和 CI 过了再更新 READY 与 SHA。
- 合并优先带 SHA 条件：`gh pr merge <PR> --match-head-commit <完整SHA>`；不支持时先 `git ls-remote origin <分支>` 与 READY SHA 比对，不一致停止重审。
- push 失败但 `gh pr merge` 成功时，GitHub 合的可能是旧版：合并后 `git fetch && git diff <本地验证commit>..origin/master` 核差异仅含 base 差异、无内容丢失；实际合入 sha 写进账本。
- 多 PR 同动 Cargo.lock / 同 crate：GitHub 文本合并零冲突 ≠ 语义组合已验证，对合并态跑一次全量测试再清理。
- 审查 PR 用只读 refs：`git fetch origin pull/<N>/head:refs/remotes/pr/<N>`，`git diff origin/master...refs/remotes/pr/<N>`，不 checkout。审查结论自动发 PR 评论（`gh pr comment <N> --body-file`，专业书面语，无本地路径/密钥/称呼）。
- 合并前的那条评论必须自带证据：三项门禁命令原文 + EXIT 值 + 测试通过数 + 变异反证（改坏点 / 失败测试名 / EXIT）。
- 含真机行为的 PR：人工验收清单显式留在回报里，并同步账本遗留段；清单不出现 = 验收义务静默蒸发。
- PR 正文按审查改写后，全篇 grep 状态词（draft / incomplete / open / closed / 未完成 / 未验收 / ready / Provisional），逐行对照当前事实；改过的段落整段替换。

## 中断续做（parallel-implement.md §15/§28，shared-worktree.md 半成品采信节）

SSL / 429 / 会话退出中断是常态。续做 agent 先五查，再接着做，不重写已有改动：

1. 票 `Status:` 是否停在 `claimed`（无 `## Answer` = 没结票）；
2. `git -C <worktree> status --short` / `diff --stat` 半成品范围 + 最后一次报错原文；
3. 主仓 `status --short` 有无误写副本（与 worktree `cmp`，同内容撤主仓副本）；
4. 锁残留：issue worktree 用各自 target、无 `/tmp/aidog-cargo.lock`（2026-11-24 改版）；主仓共享场景残留判据照 shared-worktree.md「构建目录」节；`.git/index.lock`（无 git 进程 + 0 字节才删）；
5. 半成品含 JSON / 数据文件先跑 `yarn check:defaults` 再续做。

半成品采信三前提（你信的判据只有一个：能否独立验证到绿；沉没的 token 不是续做理由）：

1. `git diff` 逐 hunk 看过、每段能归到票面任务；
2. 全部门禁补跑绿（不是子集）；
3. commit message 注明 `adopted from interrupted agent`，门禁整条重跑。

改动半截不可判定（编译不过、门禁修不动）→ 回滚该 worktree 重派。挂死 agent（外呼命令久挂 30 分钟 + 催收 1 次无响应等 10 分钟）→ TaskStop + 接管 agent，接管 prompt 带现场核对清单，「从核对现场接续，不重做已完成步骤」。

## 磁盘与产物（shared-worktree.md「构建目录」「减少磁盘残留」节，§24/§37）

- 开跑前 `df -h .`；余量 <10G 不开跑；跑到一半满盘先报主会话，删 target / 缓存要授权且持锁。
- 门禁命令写法：`{ cmd; rc=$?; echo EXIT=$rc; } > .tmp/<任务名>/x.log 2>&1`，退出码不进管道；`&&` 链上计数类 grep 补 `|| true`，链尾 `echo GATE_CHAIN_DONE`，链尾再核 `test result: ok` 计数 = 期望 suite 数。
- 先红与门禁日志留到 review 与合并完成，之后压缩：`grep -E 'EXIT=|test result|FAILED|panicked' x.log > x.summary.log` 再删原日志。
- 临时产物全部进仓库 `.tmp/<任务名>/`（`.gitignore` 内），收工清自己建的；issue worktree 连自己的 target 一起清（2026-11-24 改版，无共享锁目录例外）。
