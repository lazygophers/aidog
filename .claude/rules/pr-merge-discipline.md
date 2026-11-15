# PR 合并前两条纪律：偏差落 spec、真机清单可见

2026-09-28 cc-sub-mitm（PR #43）收尾沉淀。全量 git 授权（push/merge 免确认）
不等于免提示：两条都不阻塞合并，但合并当轮回报里必须可见。

## 1. 评审裁决「不修」必须落 spec 偏差节

code review 报出但裁决不修的项（判断题非违规），当轮写进 spec 偏差节
（cc-sub-mitm 用 §5.5）+ 当日账本决策行，三要素齐全：为什么不修、复活条件
（什么情况重开）、裁决日期。缺复活条件的「不修」下一轮重审会被当成新发现
再报一遍——同模式先例：ui-parity-audit.md「有意偏离」档（带裁决日期与依据）。

- spec 已归档/删除的，写当日 `.scratch/memory/YYYY-MM-DD.md` 并在 PR 描述留一行。
- **直接 commit（无 PR）路径同样适用**（2026-10-04 补）：本仓大量改动不走 PR
  直接提交，此时「PR 描述留一行」无处落——记当日账本遗留段即可，但三要素
  （为什么不修 / 复活条件 / 裁决日期）一样不能少。该轮 perf-followup 与
  plat-select-status 两个 spec 各 3 条判断题「不修」只记了理由没记复活条件，
  spec 又已删除，重审时无法区分「已裁决」和「没看过」。
- 「不修」≠「不是问题」：某项后续被复现踩中时，先查当初复活条件是否已满足，
  满足就按它重开，不重新辩论。
- 审查二选一（「补来源 / 移除」这类）且被质疑的数据是 master 存量时，第三条
  收口路径：开跟进 issue 链接进 PR（2026-11-15 PR #63 tokenrhythm peak 先例，
  issue #169）。本 PR 内移除 master 存量属范围外改动，开票即满足审查。

## 2. 含真机行为的 PR：人工验收清单必须显式留在回报里

门禁（cargo test / clippy / tsc / vitest）覆盖不了的行为——抓包验证、GUI 交互、
外部客户端（Claude Code）实测——合并前主会话回报单列「人工验收 N 条未执行」
清单（PR #43 六条先例：真机零泄漏抓包 / 真实订阅流量对照 / claude login
auth.anthropic.com 抓包 / 零主动轮询观测 / 盲转无感 / #91703 实测），并同步账本
**遗留**段。全量授权下合并不等用户点头，但清单不出现在当轮回复 = 验收义务
静默蒸发。

## 3. PR 正文按审查改写后：推送前全篇扫一遍过期状态词（2026-11-15 PR #63）

PR #63 正文按审查改写了三轮仍有残留过期措辞（round 4 才改干净：「Status:
incomplete; keep this PR in draft」残留到 round 4、「Tauri 未验收」行 round 4
才 closed）。增量改写只动审查点名的段落，点名段落之外的过期行漏网。

- 改写后推送前对正文全文 grep 状态词：`draft / incomplete / open / closed /
  未完成 / 未验收 / ready / Provisional`，逐个命中行对照当前事实核一遍；
  改动过的段落用整段替换，不用增量补丁。
- 正文状态行必须与最新一轮审查回复逐条互证：审查说「改完 X 即 READY」，
  正文里 X 的表述就要当轮改到 READY 口径，不留「下轮再说」。
## 4. 审查他人 PR：只读 refs、先核对象存在、分歧先取证（2026-11-14 #159 / #162 / #158 / #63 批次）

- **主 checkout 不动**：`git fetch origin pull/<N>/head:refs/remotes/pr/<N>`，审查用
  `git diff origin/master...refs/remotes/pr/<N>` 与 `git show refs/remotes/pr/<N>:<path>`，不 checkout、不开 worktree
  （该批次主 checkout 正处于合并中途，checkout 会丢现场）。审完 `.tmp/<任务名>/` 删净。
- **派写代码 agent 前先核对象**：用户给的 PR 号先 `gh pr view <N>`（#163 不存在，答案是 #162，一次表单才对上）；
  「修复重复」「修一下」这类短指令先 `git status` 看有无 `MERGE_HEAD` / 冲突文件——本地合并冲突与 PR 代码缺陷
  两种解读都成立时，先问再派（该批次按 PR 缺陷派了修复 agent，用户更正为本地冲突，agent 留下半截合并的 worktree，事后删除）。
- **两条审查轴结论相反时，发评论前主会话先取证**（grep 调用链 / 读迁移代码，parallel-implement.md §6 同口径）。
  取证不到才在评论里标「两轴分歧、待作者确认」并写明缺哪条事实；#158「主库提交后崩溃窗口」是前提未取证就交了作者。
- **审查完成后自动发 PR 评论**（2026-11-15 提升自私有 memory；用户 2026-11-14 原话「如果是审查pr，都自动发送评论」）：审查 PR 出结论后（含复审）直接 `gh pr comment <N> --body-file .tmp/<任务名>/body.md` 发出，不等用户再说；正文专业书面语，不含本地绝对路径、密钥、人设称呼与语气词；发完删 `.tmp/<任务名>/`。范围仅限审查 PR 的评论，不扩展到 push / merge（push 仍需当轮明说）。

## 5. push 失败后合并：先核对 GitHub 端分支版本（2026-11-15 #162 实锤）

`git push` SSL 偶发失败而 `gh pr merge` 照样成功时，GitHub 合进 master 的是 **push 成功的那个旧版本**，
不是本地最终验证过的树。#162：本地 rebase 重 base 后 push 失败，GitHub 合的是 bdec7bc64 旧基版本——
#158+#162 组合态全量测试只对本地树做过，合并版本与验证版本不一致。

- `gh pr merge` 前核对：`git ls-remote origin <分支>` 的 sha == 本地分支 HEAD 才 merge；不等先重试 push。
- 已发生不等：合并后 `git fetch && git diff <本地验证commit>..origin/master` 核对差异仅含 base 差异
  （#162 差异仅为 base 的 #158 改动）、无内容丢失；实际合入的 sha 写进账本交付行。
- **多 PR 同动 Cargo.lock / 同 crate：合并后对合并态跑一次全量测试再清理**（本轮 detach worktree 验
  #158+#162 组合，38 suites 全绿后才清 worktree/分支）——GitHub 文本合并零冲突 ≠ 语义组合已验证。
