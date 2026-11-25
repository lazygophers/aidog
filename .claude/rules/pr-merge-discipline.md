# PR 合并前两条纪律：偏差落 spec、真机清单可见

全量 git 授权（push/merge 免确认）不等于免提示：以下各条不阻塞合并，但合并当轮回报里必须可见。

## 1. 评审裁决「不修」必须落 spec 偏差节

code review 报出但裁决不修的项（判断题非违规），当轮写进 spec 偏差节 + 当日账本决策行，四要素齐全：为什么不修、复活条件（什么情况重开）、裁决日期、**与哪条用户硬约束 / 拍板口径相关**。缺复活条件的「不修」下一轮重审会被当成新发现再报一遍——同模式先例：ui-parity-audit.md「有意偏离」档。凡与用户硬约束（数据不丢、精确还原、安全边界这类）存在张力的「不修」，第四要素写明约束出处，供下一轮复核直查。

- spec 已归档/删除的，写当日 `.scratch/memory/YYYY-MM-DD.md` 并在 PR 描述留一行。
- **直接 commit（无 PR）路径同样适用**：「PR 描述留一行」无处落时记当日账本遗留段即可，三要素一样不能少。
- 「不修」≠「不是问题」：后续被复现踩中时，先查当初复活条件是否已满足，满足就按它重开，不重新辩论。
- 审查二选一（「补来源 / 移除」这类）且被质疑的数据是 master 存量时，第三条收口路径：开跟进 issue 链接进 PR（先例：PR #63 tokenrhythm peak，issue #169）。本 PR 内移除 master 存量属范围外改动，开票即满足审查。

## 2. 含真机行为的 PR：人工验收清单必须显式留在回报里

门禁（cargo test / clippy / tsc / vitest）覆盖不了的行为——抓包验证、GUI 交互、外部客户端（Claude Code）实测——合并前主会话回报单列「人工验收 N 条未执行」清单（先例 PR #43：真机零泄漏抓包 / 真实订阅流量对照 / claude login auth.anthropic.com 抓包 / 零主动轮询观测 / 盲转无感 / #91703 实测），并同步账本 **遗留** 段。全量授权下合并不等用户点头，但清单不出现在当轮回复 = 验收义务静默蒸发。

## 3. PR 正文按审查改写后：推送前全篇扫一遍过期状态词

- 推送前对正文全文 grep 状态词：`draft / incomplete / open / closed / 未完成 / 未验收 / ready / Provisional`，逐个命中行对照当前事实核一遍；改动过的段落用整段替换，不用增量补丁。
- 正文状态行与最新一轮审查回复逐条互证：审查说「改完 X 即 READY」，正文里 X 当轮改到 READY 口径，不留「下轮再说」。

## 4. 审查他人 PR：只读 refs、先核对象存在、分歧先取证

- **主 checkout 不动**：`git fetch origin pull/<N>/head:refs/remotes/pr/<N>`，审查用 `git diff origin/master...refs/remotes/pr/<N>` 与 `git show refs/remotes/pr/<N>:<path>`，不 checkout、不开 worktree。审完 `.tmp/<任务名>/` 删净。
- **派写代码 agent 前先核对象**：用户给的 PR 号先 `gh pr view <N>` 核实存在；「修复重复」「修一下」这类短指令先 `git status` 看有无 `MERGE_HEAD` / 冲突文件——本地合并冲突与 PR 代码缺陷两种解读都成立时先问再派。
- **两条审查轴结论相反时，发评论前主会话先取证**（grep 调用链 / 读迁移代码，parallel-implement.md §6 同口径）。取证不到才在评论里标「两轴分歧、待作者确认」并写明缺哪条事实。
- **审查完成后自动发 PR 评论**：出结论后（含复审）直接 `gh pr comment <N> --body-file .tmp/<任务名>/body.md` 发出，不等用户再说；正文专业书面语，不含本地绝对路径、密钥、人设称呼与语气词；发完删 `.tmp/<任务名>/`。范围仅限审查 PR 的评论，不扩展到 push / merge（push 仍需当轮明说）。
- **合并时评论必须自带证据**：合并前那条评论必须含三项门禁的命令原文 + EXIT 值 + 测试通过数，以及变异反证的改坏点 / 失败测试名 / EXIT；本机 `.tmp/pr-<N>/` 证据目录会被其他会话清理，评论是唯一长期证据。

## 5. push 失败后合并：先核对 GitHub 端分支版本

`git push` SSL 偶发失败而 `gh pr merge` 照样成功时，GitHub 合进 master 的是 **push 成功的那个旧版本**，不是本地最终验证过的树。

- READY 必须绑定完整 head SHA（40 位）；READY 后 head 改变，旧结论失效。fetch 新 head 后审查增量 diff、确认受影响的必要测试和 CI 均通过，再更新 READY 与 SHA。
- 合并前比较 GitHub 当前 head 与 READY SHA：优先 `gh pr merge <PR> --match-head-commit <完整SHA>`；不支持时用 `git ls-remote origin <分支>` 与本地 READY SHA 比较，一致才合并，否则停止并重新审查增量。
- 已发生不等：合并后 `git fetch && git diff <本地验证commit>..origin/master` 核对差异仅含 base 差异、无内容丢失；实际合入的 sha 写进账本交付行。
- **多 PR 同动 Cargo.lock / 同 crate：合并后对合并态跑一次全量测试再清理**——GitHub 文本合并零冲突 ≠ 语义组合已验证。
- **PR head 在 READY 判定后变更，旧结论作废**：fetch 新 head 后核对增量 diff，确认新改动在审查范围内、必要测试和 CI 通过、冲突解法保留两侧意图，再更新 READY 与完整 SHA。冲突解法涉及行为两侧真值的，增量核对要独立源码比对，不以 GitHub 零冲突放行。
