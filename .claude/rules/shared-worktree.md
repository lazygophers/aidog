# 多会话共用一个工作区：提交纪律

多个 Claude 会话同时在**同一个 checkout** 上改代码时，提交必须逐 hunk 认领。
2026-09-23 一天之内发生三次误扫（A 的未提交改动被 B 的提交带走），三次内容都正确、
功能都没坏，所以没人当场发现——这正是它危险的地方。

## 硬规则

1. **禁止 `git commit -a` / `git commit` 不带路径。** 一律 `git commit -- <路径> [<路径>…]`。
2. **提交前逐 hunk 认领**：`git diff -- <文件>` 扫一遍，确认每一段改动都是自己写的。
   不是自己的就 `git add -p` 只挑自己的那几段，再 `git commit`（不带 `-a`）。
3. `git status` 只认到**文件**这一层，**防不住同文件内跨 hunk 的误扫**——
   第三次事故就是这么来的：文件确实是自己的活，文件里还有别人的两段。
4. 发现自己扫走了别人的改动：**不回滚、不重写历史**（代价大于收益），
   立刻告诉对方「你的那部分已经在 `<commit>` 里了」，让对方撤掉本地副本，
   否则会重复改一遍。
5. **共享 checkout 上 `git pull --rebase --autostash` 会把工作区里所有人
   （含别人的 dirty 文件）一起 stash 再 pop**。用前先确认远端新提交不碰
   dirty 文件集：`git fetch && git diff HEAD...@{u} --name-only` 对照
   `git status --short`，无交集才可用；有交集就停，改手动处理
   （2026-09-29 按「先核实远端 diff 不碰 dirty 文件」用过一次，前置检查
   才是安全条件，autostash 本身不安全）。

## 文件所有权

冲突高发的共用文件要有主：

| 文件 | 归属 |
|---|---|
| `flutter/lib/src/pages/ui_bits.dart` | team-lead；他人改前先说 |
| `flutter/lib/src/shell/*` | team-lead |

其余文件按「谁在做那块活谁临时持有」，做完明确交还。

## 后台 agent 的提交边界

2026-09-27 r102：取证 agent 在官方源取不到（HF 401）时，凭「架构上限」自行写值并
commit（c3bb3e4d0 把 crazyrouter gte-rerank-v2 写成 32768），其最终报告自己都没采信
该值；主会话复核推翻，改官方值 30000（e49d0b385）。

- **取证型 agent 只产报告，不写数据文件、不 commit。** 项目 CLAUDE.md 的自动 commit
  授权属于主会话**已复核**的改动，不随任务派生继承给 agent。
- 派 agent 时主会话要写明边界：只读取证 → 报告回传；需要落库的值由主会话采信后自己写。
- agent 报告里标「存疑 / 需拍板 / 非官方源」的值不得出现在任何 commit 里——commit
  message 自己写着 `architectural limit` 的值，就是没被采信的值。

## implementer 挂掉后的半成品采信（2026-09-28 cc-sub-mitm PR #43）

修复 agent 中途 502 断连，主会话核对其未提交 diff 后采信、补跑全部门禁、代 commit。
与上节 r102「取证 agent 不 commit」的边界在**验证成本**：

- **取证型 agent 产物是值**：机器验不了真假，必须主会话复核采信后才落库
  （r102 原文）。报告里标「存疑 / 需拍板 / 非官方源」的值不进任何 commit。
- **实现型 agent 产物是代码**：门禁可机器验证。挂掉后主会话可采信半成品，三条
  前提全满足才采：① `git diff` 逐 hunk 看过、每段能归到票面任务；② 全部门禁
  补跑绿（不是只跑改动相关子集）；③ commit message 注明来源（如
  `fix: ... (adopted from crashed agent, gates re-run)`）。

**agent 基础设施故障（502 / 断连 / 超时）恢复路径**：核对现场（`git status` +
`git diff`）后二选一——改动完整或差最后一步、门禁可跑 → 主会话按上面三条接管
收尾；改动半截不可判定（编译不过、门禁红了修不动、线索只剩半份报告）→ 回滚
该 worktree 重派，不硬续。判据只看「主会话能否独立验证到绿」，不看「已经写了
多少」——沉没的 token 不是采信理由。

## 判据

提交前问自己一句：**这次 `git diff --cached` 里的每一段，我都能说出它属于哪条任务吗？**
说不出的那段就是别人的。

## rtk 包装的 `git worktree add` 静默失败（2026-10-01）

perf-backend 收尾轮：PreToolUse hook 把 `git worktree add` 改写经 rtk 执行，
输出 "ok" 但 worktree 实际没建（`rev-parse --show-toplevel` 指回主仓库），
后续 `git add` 全部加进主仓库，差错一点就在共享 checkout 上提交了别人的位置。

- **worktree 的建/删/查一律 `/usr/bin/git` 直跑**，绕开 rtk 包装。建完当场
  `rev-parse --show-toplevel` 验一次 toplevel 指向新目录——「命令输出 ok」
  不等于「worktree 存在」（registry-data-edit.md「判脚本干了什么以输出数字为准」
  同模式）。

## agent 子会话的 Bash cwd 每次调用重置（2026-09-30）

全局 CLAUDE.md 写「Bash 的 cwd 跨调用保留」，那只对主会话成立。**派生的
agent（team agent / implementer）相反：cwd 每次 Bash 调用后都重置回自己的
启动目录**（通常是主 checkout）。jev-decision-proxy 轮（PR #46）因此翻车：
implementer 在 worktree 里跑相对路径命令，静默打到主 checkout 两个文件
（当场发现还原）。

- **脚本 / 命令里引用文件一律绝对路径**——不止 worktree 场景（2026-10-08
  model-price-tag 轮实锤：locale 批量插入脚本的 insert 函数用裸文件名，cd 丢失后
  文件解析落错目录，整轮白跑一次）。worktree 里同理：绝对路径，或单条
  `(cd <worktree 绝对路径> && <cmd>)`。不能信任上一条命令切过去的目录——
  那条 cd 已经没了。
- 动手前后各跑一次 `git -C <主 checkout> status --short`：多出来的脏文件
  就是误伤，当场还原并核对内容。

## registry 盖戳脚本无参模式会盖到别人的脏文件（2026-10-02）

`scripts/bump-registry-last-updated.mjs` 无参 = 「盖 git 变更（含 untracked）的 registry json」，
共享 checkout 上 `git status` 里的脏文件含**别的会话未提交的改动**——commandcode-goat 轮另一会话
在改 litellm/openrouter（4331 个文件），无参跑把 8709 个文件全部重盖戳。盖戳内容中性（只动
last_updated 时间戳），但别人的 commit 会无声带上你的盖戳 diff（「误扫」的无害化版本），且时间戳
被无谓推高、远程同步白拉一遍。

- **无参盖戳前先 `git status --short` 核对脏文件集**：全是自己的 → 直接跑；混有他人文件 →
  等对方收工，或跑完**当场告知对方**「你的脏 registry 文件被盖了戳」并把自己 commit 严格限路径
  （硬规则 1）。
- 判据同 registry-data-edit.md「以输出数字为准」：输出「N 个文件已盖戳」的 N 远超自己的改动集
  时，先 `git diff -- <他人文件>` 核对只有时间戳一行变化，再继续。
