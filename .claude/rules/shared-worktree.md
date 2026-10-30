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
- agent 填入数据的任何外部 slug / URL（registry `logo_url` 的 simpleicons slug、
  `source_urls` 等），主会话采信前 curl 一次可达（2026-10-08 passthrough-platform 轮：
  implementer 填 slug `shuffle` 双源 404，被 `known_simpleicons_slugs_are_valid`
  白名单测试拦下——测试是最后一道防线，主会话核对是更早的一道）。

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
- **判断工作区状态也走 `/usr/bin/git status --short`**（2026-10-22 入站密钥轮）：
  rtk 包装的 `git status --short` 输出被截断，只列部分已改文件，一度误判改动
  丢失。rtk 版只当摘要，数文件数以 `/usr/bin/git` 为准。

## agent / detach worktree 复用单一 CARGO_TARGET_DIR（2026-10-22 立，2026-10-30 改）

历史（2026-10-22）：为跑基线 `git worktree add --detach /tmp/aidog-baseline-*` 并把
`CARGO_TARGET_DIR` 指向主仓 `src-tauri/target` 共享，同名 workspace crate 产物串味
（`aidog_core` 报 `cannot find function ... in crate aidog_db`；worktree 删后
`build.rs:31 registry dir: NotFound`）。2026-10-30 又实锤：别的会话 worktree 共用主仓
target 致 `test_registry.rs:359` 假红。

现行做法（用户 2026-10-30 原话「要清理，以后尽量不要有临时编译目录，复用同一个」）：

- agent worktree / detach worktree 一律 `CARGO_TARGET_DIR=/tmp/aidog-agent-target`，
  不按票新建；收尾不再逐票删 target。
- 主仓 `src-tauri/target` 只给主仓自身构建用。
- 跑测试前 `cargo clean -p aidog_db --target-dir /tmp/aidog-agent-target`，消除
  aidog_db build.rs registry 产物陈旧（该目录是 agent 自有，清它不影响别的会话）。
- 前端构建输出 `--outDir /tmp/aidog-agent-dist`，不按票新建 dist。

## `.git/index.lock` 残留（2026-10-22）

0 字节 `.git/index.lock` 残留 33 分钟、无 git 进程时：先
`pgrep -fl 'git '` 确认无进程、`ls -l` 确认 0 字节且 mtime 久远，再删，然后提交。
有进程或非 0 字节就停，那是别的会话正在写。

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

## commit -m 双引号里的 `$` 会被 shell 展开（2026-10-15）

registry-refresh 轮：`git commit -m "…$0.10/$0.50…"` 中两个 `$0` 均被 zsh
展开成 `/bin/zsh`，message 变成「/bin/zsh.10//bin/zsh.50」，两条 commit 均中招，
后经用户授权 rebase 改回。`$0` 这类位置性参数展开**不报错**，静默进历史——
价格类 commit message 高频带 `$数字`，是重灾区。

- 带 `$` 的 commit message 一律单引号 `'…'`，或写临时文件后 `git commit -F <file>`。
- commit 后看一眼 git 输出首行摘要核对 message 原样落盘。

## 别的会话当天 commit 改夹具，绿门禁中途变红（2026-10-18）

整仓审计收官全量门禁时 hooks 测试转红，本 session 的 diff 不碰它。根因：另一会话
2a17cc818 给 `src-tauri/defaults/settings.json` 加了用户 hooks（tracking_progress
等），测试断言「remove 后 hooks.Stop 键消失」随之过时——夹具语义变了，断言没跟。
主会话按 parallel-implement.md「基线红只由主会话修一次」收敛断言为「aidog hook
项已清」（9d085a726），不删别人夹具里的用户 hook。

- **排查中途新红先看夹具的近期提交**：`git log --oneline -5 -- <测试断言涉及的
  夹具文件>`，别的会话当天改过就先归因到它，不在自己 diff 里翻（perf-testing.md
  §6「按理已删的函数出现在 profile 里」同模式：先核实环境，再怀疑自己）。
- 共享 checkout 上「开工时跑过全绿」不能保证收官时还绿——夹具、默认配置
  （defaults/*.json）、门禁脚本本身都可能被并行会话推进。修法守基线红纪律：
  主会话修一次、修断言不改别人夹具、commit 里注明夹具来源（9d085a726 的
  "defaults now ships user hooks" 即此）。

## 共享 $TMPDIR 固定名 fixture：格式变更后跨 worktree 互踩从无害变致命（2026-10-20）

`test_rw_pool` 的两文件测试把主库存 `temp_dir()` 根，伴生库 log.db / platform.db
由 `Db` 派生逻辑落在 `$TMPDIR` 顶层固定名（无 pid 后缀，清理循环也不删）。明文
时代两个 worktree 并发跑测试，后写者覆盖先写者的文件，恰好无害；SQLCipher 整库
加密后该文件变成加密库，plain worktree 的 `Db::new` 直接打不开——跨 worktree
并发测试互踩从「碰巧无害」变「随机测试红」（00627eb41 改 `tempfile::tempdir()`
修）。

- **测试 fixture 的 DB / 文件路径必须进程级唯一**（`tempfile::tempdir()` 或带
  pid + 随机后缀），禁 `$TMPDIR` 顶层固定名。判据：换台机器并发跑两个 checkout，
  你的 fixture 文件名会不会撞。
- 「无害的共享」是格式决定的，不是结构决定的：做格式/编码变更类 spec（加密、
  序列化格式、schema 迁移）时把 fixture 路径列进审查面——上一代认为安全的共享，
  在格式变更后会变致命。
- 排查这类红的对照法：隔离 `TMPDIR`（`TMPDIR=$(mktemp -d) cargo test ...`）全绿、
  真 `$TMPDIR` 偶发红 = fixture 互踩实锤，不是 flaky。

## harness `isolation: worktree` 的 worktree 收工清理：先 unlock 再 remove（2026-10-28）

harness 隔离建的 agent worktree 自带 lock，收工直接 `/usr/bin/git worktree remove`
被 locked 拒删。清理两步：`/usr/bin/git worktree unlock <path>` 后 `remove --force`；
建/删/查照旧全走 `/usr/bin/git` 直跑（见上文 rtk 条）。

## 共享 checkout 合回 master：只挪指针，不 stash 外来脏文件（2026-10-29 计费线收官）

工作区带别的会话的脏文件（如 `src-tauri/defaults/registry/index.json` 盖戳）时，
集成分支回 master 的 `stash → checkout master → merge → pop` 会连别人的改动一起
搬动，用户当场拒绝。做法：

1. 核对 master 是集成分支祖先：`/usr/bin/git merge-base --is-ancestor master <分支>`
   退出码 0 才可快进。
2. 按当前分支二选一：
   - 当前不在 master → `/usr/bin/git fetch . <分支>:master`（只挪指针，不动工作区）；
   - 当前就在 master → `/usr/bin/git merge --ff-only <分支>`（脏文件与合入内容不重叠才可用）。
3. 切回 master 前确认脏文件在两边内容相同（`git diff <分支> master --stat -- <脏文件>`
   为空），checkout 才不会碰它们。本步只适用于第一种（当前不在 master）。
4. 收尾 `/usr/bin/git status --short` 核对脏文件原样还在。

## 推送只走 origin（2026-10-29）

历史：`no-mistakes` 远端（目录 `/Users/luoxin/.no-mistakes/repos/9d3abbd5e514.git` 已不存在）
已于 2026-10-29 `git remote remove`。现行做法：推送只走 `origin`（GitHub `lazygophers/aidog`），
先 `git fetch origin && git rev-list --left-right --count origin/master...master`，左边为 0
（纯快进）才 `git push origin master`；origin 是公开远端，推送前仍按全局规则向用户确认一次。

## 多个 agent 同时动主 checkout 的 master：串行（2026-10-29）

合并 agent 与文档 commit agent 同时操作主仓 master，出现 `ff-only` 失败（master 已被
5b6788efe 推进）和瞬时 `.git/index.lock`。

- 凡写主 checkout（merge / commit / 改 master 指针）的 agent 串行派；并行实现留在各自 worktree，
  只在合并阶段排队进主仓。
- ff-only 失败且双方无冲突：改 `merge --no-ff`（使 `branch -d` 仍可删，不用 rebase 改写历史、
  不用 cherry-pick 后 `-D`）。
