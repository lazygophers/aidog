# 开发工作流

---

## 核心原则

1. **先规划再写代码** — 动手前先想清楚要做什么
2. **规范靠注入, 不靠记忆** — 指南经 hook/skill 注入, 不从记忆里回想
3. **一切落盘** — 调研、决策、教训全写进文件; 对话会被压缩, 文件不会
4. **增量开发** — 一次只做一个 task
5. **沉淀经验** — 每个 task 后回顾, 把新知识写回 spec

---

## Trellis 系统

### 开发者身份

首次使用时, 初始化你的身份:

```bash
python3 ./.trellis/scripts/init_developer.py <your-name>
```

创建 `.trellis/.developer` (已 gitignore) + `.trellis/workspace/<your-name>/`。

### Spec 系统

`.trellis/spec/` 存放按 package 与 layer 组织的编码指南。

- `.trellis/spec/<package>/<layer>/index.md` — 入口, 含 **Pre-Development Checklist** + **Quality Check**。实际指南存于它指向的 `.md` 文件。
- `.trellis/spec/guides/index.md` — 跨 package 的思维指南。

```bash
python3 ./.trellis/scripts/get_context.py --mode packages   # 列出 packages / layers
```

**何时更新 spec**: 发现新模式/约定 · 需固化的 bug-fix 防范 · 新技术决策。

### Task 系统

每个 task 在 `.trellis/tasks/{MM-DD-name}/` 下有独立目录, 含 `prd.md`、`implement.jsonl`、`check.jsonl`、`task.json`, 可选 `research/`、`info.md`。

```bash
# Task lifecycle
python3 ./.trellis/scripts/task.py create "<title>" [--slug <name>] [--parent <dir>]
python3 ./.trellis/scripts/task.py start <name>          # set active task (session-scoped when available)
python3 ./.trellis/scripts/task.py current --source      # show active task and source
python3 ./.trellis/scripts/task.py finish                # clear active task (triggers after_finish hooks)
python3 ./.trellis/scripts/task.py archive <name>        # move to archive/{year-month}/
python3 ./.trellis/scripts/task.py list [--mine] [--status <s>]
python3 ./.trellis/scripts/task.py list-archive

# Code-spec context (injected into implement/check agents via JSONL).
# `implement.jsonl` / `check.jsonl` are seeded on `task create` for sub-agent-capable
# platforms; the AI curates real spec + research entries during Phase 1.3.
python3 ./.trellis/scripts/task.py add-context <name> <action> <file> <reason>
python3 ./.trellis/scripts/task.py list-context <name> [action]
python3 ./.trellis/scripts/task.py validate <name>

# Task metadata
python3 ./.trellis/scripts/task.py set-branch <name> <branch>
python3 ./.trellis/scripts/task.py set-base-branch <name> <branch>    # PR target
python3 ./.trellis/scripts/task.py set-scope <name> <scope>

# Hierarchy (parent/child)
python3 ./.trellis/scripts/task.py add-subtask <parent> <child>
python3 ./.trellis/scripts/task.py remove-subtask <parent> <child>

# PR creation
python3 ./.trellis/scripts/task.py create-pr [name] [--dry-run]
```

> 运行 `python3 ./.trellis/scripts/task.py --help` 查看权威的、最新的命令列表。

**Current-task 机制**: `task.py create` 创建 task 目录, 并 (当 session identity 可用时) 自动设置 per-session active-task 指针, 使 planning breadcrumb 立即触发。`task.py start` 写入同一指针 (已设置则幂等) 并把 `task.json.status` 从 `planning` 翻为 `in_progress`。状态存于 `.trellis/.runtime/sessions/`。若 hook input、`TRELLIS_CONTEXT_ID` 或平台原生 session 环境变量都拿不到 context key, 则无 active task, `task.py start` 失败并给出 session identity 提示。`task.py finish` 删除当前 session 文件 (status 不变)。`task.py archive <task>` 写入 `status=completed`, 把目录移到 `archive/`, 并删除所有仍指向被归档 task 的 runtime session 文件。

### Workspace 系统

在 `.trellis/workspace/<developer>/` 下记录每次 AI session, 用于跨 session 追踪。

- `journal-N.md` — session 日志。**每文件最多 2000 行**; 超出时自动新建 `journal-(N+1).md`。
- `index.md` — 个人索引 (总 session 数、最近活跃)。

```bash
python3 ./.trellis/scripts/add_session.py --title "Title" --commit "hash" --summary "Summary"
```

### Context 脚本

```bash
python3 ./.trellis/scripts/get_context.py                            # 完整 session runtime
python3 ./.trellis/scripts/get_context.py --mode packages            # 可用 packages + spec layers
python3 ./.trellis/scripts/get_context.py --mode phase --step <X.Y>  # 某 workflow 步骤的详细指南
```

---

## Phase 索引

```
Phase 1: Plan    → 想清楚要做什么 (brainstorm + research → prd.md)
Phase 2: Execute → 写代码并通过质量检查
Phase 3: Finish  → 提炼教训 + 收尾
```

[workflow-state:no_task]
无 active task。**A 直接回答** — 纯问答 / 解释 / 查询 / 闲聊; 无文件写入 + 一行答复 + 读仓库 ≤ 2 文件 → AI 自行判断, 无需 override。
**B 创建 task** — 任何实现 / 代码改动 / 构建 / 重构工作。进入序列: (1) `python3 ./.trellis/scripts/task.py create "<title>"` 创建 task (status=planning, breadcrumb 切到 [workflow-state:planning] 提供 brainstorm + jsonl 阶段指引) → (2) 加载 `trellis-brainstorm` skill 与用户讨论需求并迭代 prd.md → (3) prd 完成且 jsonl 整理好后, 运行 `task.py start <task-dir>` 进入 [workflow-state:in_progress] 写实现骨架。**"看起来很小" 不构成把 B 降级为 A 或 C 的理由**。
**C Inline 改动** (仅限本轮, 是 B 的 escape hatch) — 用户的当前消息 MUST 含以下之一: "skip trellis" / "no task" / "just do it" / "don't create a task" / "跳过 trellis" / "别走流程" / "小修一下" / "直接改" / "先别建任务" → 简短确认 ("ok, skipping trellis flow this turn"), 然后 inline。**未看到这些短语之一, 你绝不能自行 inline**; 不要编造用户从未说过的 override。

<!-- trellisx:start:no_task -->
trellisx 规约 (强化原生判定, 不切断建 task 路径): 本项目愿景 = 除极简任务外, 一律走 Trellis task 执行。

**作用域边界 — 何时建 task (workflow 四规范的触发前提)**:

| 特征 | 判定 |
| --- | --- |
| 纯查询 / 文档阅读 / 问答 (无改动) | 豁免, 不建 task |
| 单文件单处改, ≤20 行且位置已知 | 豁免 |
| 跨 ≥2 文件 / 单文件多处 / 多步骤改 | **必建 task** |
| 需外部调研 (库选型/方案对比) 或产出文档交付 | **必建 task** (调研为 research subtask) |
| 边界模糊 | **MUST AskUserQuestion 由用户裁定** |

**无 task → 不进 workflow → 四规范豁免**。Planning 阶段也不进 workflow: brainstorm 主线由 main 同步前台 (subagent 不能 AskUserQuestion); 纯信息调研可派 trellis-research 并行, 但设计决策由 main 汇总裁定。

- 表中"必建 task"的请求 → **默认创建 task** (不是"倾向建", 是默认就建; 仅用户明确拒绝才 inline)。建议直接用 AskUserQuestion 征得同意并进入 planning, 而非自行 inline。
- "边界模糊"行 → **MUST 主动用 AskUserQuestion 问用户** "本轮是否创建 Trellis task?", 禁默认跳过 / 禁自行替用户决定。
- 判断"新建 task"还是"并入现有 task"时, 读 `.trellis/task.md` 看板对照现有任务 (id/名称/描述/状态) 辅助判断。
原生的「先分类 + 征得同意才建」不变 — 但默认倾向从"可建可不建"上调为"默认建, 除非极简或用户拒绝"。
<!-- trellisx:end:no_task -->
[/workflow-state:no_task]

### Phase 1: Plan
- 1.0 创建 task `[required · once]` (仅 `task.py create`; status 进入 planning)
- 1.1 需求探索 `[required · repeatable]`
- 1.2 调研 `[optional · repeatable]`
- 1.3 配置 context `[required · once]` — Claude Code
- 1.4 激活 task `[required · once]` (运行 `task.py start`; status → in_progress)
- 1.5 完成标准

[workflow-state:planning]
加载 `trellis-brainstorm` skill 并与用户一起迭代 prd.md。
Phase 1.3 (required, once): 在 `task.py start` 之前, 你 MUST 整理 `implement.jsonl` 与 `check.jsonl` —— 列出 sub-agent 需要的 spec / research 文件, 使其注入正确 context。仅当 jsonl 已有 agent 整理过的条目时才可跳过 (单凭 seed `_example` 行不算)。
然后运行 `task.py start <task-dir>` 把 status 翻为 in_progress。

<!-- trellisx:start:planning -->
trellisx 规划规约 (启用判定跟随 trellis 原生 parent/child 语义, 不看数量):

⚙️ **规划同步前台 (交互式, 禁自行凭空设计)**: planning 含 `trellis-brainstorm` **逐问用户**的交互, subagent 不能与用户对话, 故 **planning 由 main 同步前台执行**, 不派 subagent。分工: **`trellis-brainstorm` 为主导** (main 同步逐问用户做需求探索 + 方案设计 + 边界, 产出 prd/design); **`trellisx-orchestrate` 仅管执行层编排** (实际执行的 subagent 职责划分、并行/依赖、资源互斥, 产出 implement.md), **不用它做需求/方案设计**。产物评审 (`AskUserQuestion`) 由 main 亲做。注: exec/check 等非交互实质工作走 subagent 执行 (异步与否按需自定, 不强制)。

判定: 本请求是否含**多个独立可验收交付** (各自可独立 plan/implement/check/archive)?
- **是 (多交付)** → 拆为 parent + child tasks (trellis 原生 `task.py create --parent`)。每个 child 独立 worktree; PRD MUST 含 mermaid 调度图显式标并行组 + 依赖箭头; child 间依赖写进 child 自己的 prd.md/implement.md (非树位置隐含)。**执行统一由 `trellis-implement` 入口调度** (main 派 trellis-implement, 由其对各 subtask 派专用 subagent 并行执行), main 不直接派 subtask agent。
- **否 (单一交付)** → 轻量单 task inline, **不强制拆 subtask**。仍走单 worktree 隔离。

拆分目的 = 让独立可验收交付各自隔离 + 最大化并行, 缩短关键路径; 不是为凑数量。详见 trellisx-orchestrate skill。

task 创建后, 用 `trellisx-workspace` 及时更新 `.trellis/task.md` 看板表 (新增/更新该任务行)。
<!-- trellisx:end:planning -->
[/workflow-state:planning]

### Phase 2: Execute
- 2.1 实现 `[required · repeatable]`
- 2.2 质量检查 `[required · repeatable]`
- 2.3 回滚 `[on demand]`

[workflow-state:in_progress]
**工具**: `trellis-implement` / `trellis-research` 只是 sub-agent 类型 (Task/Agent 工具, 不是 Skill —— 不存在同名 skill)。`trellis-update-spec` 是一个 skill。`trellis-check` 两者皆有; 代码改动后做验证时优先用 Agent 形式。
**流程**: trellis-implement → trellis-check → trellis-update-spec → commit (Phase 3.4) → `/trellis:finish-work`。
**Main-session 默认 (无 override)**: 派发 `trellis-implement` / `trellis-check` sub-agent —— main agent 默认不改代码。Phase 3.4 commit (required, once): 在 trellis-update-spec 之后, 或实现可验证地完成时, main agent **驱动提交** —— 在面向用户的文本里说明 commit 计划, 然后运行 `git commit` —— 须在建议 `/trellis:finish-work` 之前。`/finish-work` 拒绝在脏工作区运行 (`.trellis/workspace/` 与 `.trellis/tasks/` 之外的路径)。
**Sub-agent 自豁免**: 若你本身已作为 `trellis-implement` 运行, 直接从已加载的 task context 实现, 不要再派一个 `trellis-implement`; 若你已作为 `trellis-check` 运行, 直接 review/fix, 不要再派一个 `trellis-check`。默认派发规则只对 main session 生效。
**Sub-agent dispatch 协议 (所有 sub-agent)**: 派发 `trellis-implement` / `trellis-check` / `trellis-research` 时, dispatch prompt **MUST** 首行写 `Active task: <task path from `task.py current`>`, 无例外。此行是 hook 注入失败时的兜底 (`--continue` resume / hook 被禁用等); 对 `trellis-research` 还告诉它往哪个 `{task_dir}/research/` 写。
**Inline override** (仅限本轮, 是 sub-agent dispatch 的 escape hatch): 用户的当前消息 MUST 明确含以下之一: "do it inline" / "no sub-agent" / "你直接改" / "别派 sub-agent" / "main session 写就行" / "不用 sub-agent"。**未看到这些短语之一, 你绝不能自行 inline**; 不要编造用户从未说过的 override。

<!-- trellisx:start:in_progress -->
⛔ trellisx 执行硬规 (本 task 必守, 违反即流程错误):

0. **实质工作走 subagent 执行 (最高优先级)**: **概念分清** —— **task** = 任务记录 (`task.py create/start/finish/archive` 脚本), **main 同步跑**; **实质工作** (改源码、跑 check) **由 subagent 执行**。main **禁直接落地实质工作** —— 一律派 subagent (`Agent` 工具); main 只做编排 + task.py 脚本同步调用 + 用户交互决策 (`AskUserQuestion` subagent 不能做) + 完成即时回传 + 看板维护。是否 `run_in_background` 异步 / 并行 **按需自定, 不强制** (用户需要异步会自处理, 只需确保走 subagent)。每个 dispatch prompt 须 6 字段自包含 (目标/已知含 `Active task:`/范围/输出/验收/失败处理)。commit→merge→archive 由 `task.py finish` + `after_finish` hook 自动 (见 #4), 非派 agent。**🔴🛑 "派 agent" = 真实调用 `Agent` 工具, 不是叙述 (最易踩)**: 每个派 agent 动作 MUST 在**同一回复**产生真实 `Agent` tool_use; **严禁本回复无 `Agent` 工具调用就回传"已派出/agent 在做"** —— 宣称 ≠ 调用 = 幻觉跳步; 同理 "已建 task/看板已登记/worktree 已建" 必须是真实跑过 `task.py`/`trellisx-workspace`/hook 的结果。回传前自检本回复确有对应 tool_use, 无则先调用再回传。
1. **task 在 worktree 内执行 (task 级隔离, 目的 = 防并发多个 task 互相冲突)**: 隔离单位是 **task, 不是单次写盘**。`task.py start` 后 trellis hook 自动建本 task 的 worktree (`<git根>/.worktrees/<name>`); 本 task 的全部执行 (trellis-implement 及其 subagent) 都**在这个 worktree 内进行**, 主工作区保持干净。**默认一个 task 一个 worktree** (绝大多数场景); **仅当** task 内有**会互相冲突**的并行 subtask 时, 才给这些 subtask 各开子 worktree (少数场景) —— 此时收尾经 task↔worktree 映射把各子分支先合并回主分支再 archive (`trellisx-finish.py` 已支持多 worktree 合并)。
   - **执行载体 = 独立 Workflow 编排 (1 task : 1 workflow : N worktree)**: 本 task 的 exec(+check) 用 **Workflow 工具**编排成**一个独立 workflow** (Claude Code; 1 task → 1 workflow); workflow 内 fan-out 的 agent 各**worktree 隔离** (默认共享 task worktree = 1; 冲突型并行 subtask 各开子 worktree → N, finish 合并各子分支)。**runtime 退化**: 无 Workflow 平台 → 退回派 subagent 流水线 (`Agent` + `isolation:worktree`); 须显式标注当前载体 (**Claude Code Workflow** / **其他平台 agent 流水线**)。worktree 隔离规则两种载体下都不变。
   - **生成的 Workflow 必守 trellisx 四规范** (有 task 且进入执行阶段时): ① **phases 细致标类型** —— `meta.phases` 逐项标 `title` + `detail`, 阶段类型取自 {research, design, implement, verify, finalize}, 用户一眼看出每 phase 干什么属什么类型。② **允许并行** —— 无依赖 phases / fan-out agent 用 `parallel()` 并发; 有前后序的用分层 `parallel()` 表达, 不留无谓串行。③ **失败自动重试** —— 单 step/agent 失败自动重试 ≤2 次, 不让整个 workflow 直接失败; 重试仍败才降级 (返回 null + 下游 `.filter(Boolean)` 跳过, 或转 manual Blocked)。④ **收尾无残留** —— finalize phase 强制 `task.py finish` (hook 触发 commit→merge→remove worktree→archive) + finish 前 AI 自查无悬挂 Workflow/agent (TaskList 查 / TaskStop 关)。worktree 未销 / Workflow 未关 = 未闭环, 禁宣告 Done。
   - **retry 两层, 各管一类失败** (规范③落地): **workflow 脚本内自动重试** (瞬时错误: 网络抖动/工具超时/index.lock) —— `agent_with_retry` 包装, 默认 2 次, 指数退避语义。**main 重派** (逻辑失败: 自验 ✗/产物不达标) —— 对齐 apply 修复循环, 同一 subtask 累计 ≤3 次, 第 4 次禁盲目重试, 回 planning 重拆。重试计数 per-agent; 全部重试耗尽 → 该 agent 返回 null, workflow 不崩, 收尾时该项标 Blocked 上报。
2. **实施派发模型 (main → trellis-implement → subagent, 同在 task worktree 内)**: 进入实施, main **派一个 `trellis-implement` 子代理**执行实施阶段, **main 禁直接派 subtask agent、禁直接写源码** (只派 implement/check + 进度回传 + 合并)。`trellis-implement` 读 `implement.md`, 在**本 task 的 worktree 内**对各 subtask 派 subagent 执行 (**默认共享 task worktree**, 文件集不相交即可并行); **仅当并行 subtask 会互相冲突时**才给它们各开 `isolation:worktree` 子 worktree (finish 经映射合并各子分支)。严格按 implement.md / PRD 调度图依赖 + 并行组执行 (是否并行/异步按需自定)。trellis-implement 收拢全部 subtask 产物 → 交 `trellis-check`。循环: planning → trellis-implement → trellis-check → finish。
3. **轻量模式** (单 subtask): main 仍**派 `trellis-implement`**, 由其在 task worktree 内**内联直做** (无需再派 subagent), 保持 planning → implement → check → finish 循环一致。**main 不绕过 implement 自己写源码** (仍守第 1 条: task 在 worktree 内执行)。
4. **强制自动收尾 (check 通过后跑收尾, 全链自动, 不停在「提醒」)**: `trellis-check` 通过后, AI **强制收尾**, **两路任选其一, 都做全链** (commit→merge→del worktree→archive): ① `python3 .trellis/scripts/task.py finish` → `after_finish` hook 自动跑 `trellisx-finish.py`; ② `/trellis:finish-work` → apply 已注入它**先跑 `trellisx-finish.py` 全链再 journal** (修原生 archive-direct 绕 merge 丢提交)。不把收尾降级为"提醒用户" —— **合并与 worktree 删除是必须的, 非可选**:
   ① **worktree 内提交** —— `git add -A` + `git commit` (缺省消息 `chore(task): <tid> 收尾提交`), 落到 `trellisx-<name>` 分支。
   ② **合并回主分支** —— `git merge --no-ff trellisx-<name>`; **合并冲突 → 脚本自动 `merge --abort` + 报冲突文件清单 + 非 0 退出**。hook 失败仅 WARN, `task.py finish` 仍返 0 → AI **MUST 检 finish 输出有无 `trellisx-finish` 冲突告警**, 有则停下转手动, 禁强解 / 禁当成功。**未合并就 archive = 分支被销毁丢提交** (见 `trellisx-worktree.py` 教训, 故脚本先合并)。
   ③ **archive** —— 链内 `task.py archive <tid>` 归档 + 触发 `after_archive` 销毁**已合并**的 worktree + 删分支。
   - **commit 强制 (owner 授权)**: 本项目 owner 已显式要求 trellisx 流程**强制提交收尾**, 脚本直接 commit 不逐次等确认。此为 owner 对自有注入流程的设计授权, 覆盖默认「禁主动 commit」。
   - **check 未过** → 修复重检, 禁跑 finish; **未 archive (worktree 仍在) = 流程未闭环, 禁宣告 Done / 禁结束本轮**。
   - 手动兜底: 链路异常 → 直接跑 `trellisx-finish.py [--task <tid>]` (幂等可重入)。
5. **及时维护 task.md 看板**: start / 阶段推进 (exec→check→finish) / archive 后, MUST 用 `trellisx-workspace` 更新 `.trellis/task.md` 看板行 (状态/阶段/进度/worktree)。看板滞后于实际 = 流程缺陷。
6. 收每个 agent 返回立即回传用户进度; task `finish`/`archive` 时由 after_finish/after_archive hook 销毁干净的 worktree。(注: 平台 `trellisx-guard` 的 **Stop hook 不自动销毁** —— 仅对「已合并回主分支但未清理」的 worktree **提醒 + block 结束**, 让你手动 `git worktree remove` 或 `task.py finish`; 连续 ≥3 次后降级为 systemMessage 不再阻断。)
7. **任务中途修正路由 (执行中收到用户新指令)**: 本 task 已在跑 (agent/member 已派发) 时收到新指令, coordinator 先判归属:
   - **属当前任务** (修正 / 补充 / 细化已派交付) → ⛔ 禁 main 自己直接改源码、禁开新 task。按序: ① 先改对应**真值文档** (`prd.md` / `design.md` / `implement.md` 受影响条款, 标锚点) → ② 对**仍在跑**的目标 agent/member 用 `SendMessage` 下发修正 (引用改后 PRD 锚点, 令其就地纠偏, 不等跑完返工)。**先改文档再通知** (PRD 是真值, agent 复读以对齐)。
   - **独立新任务** (与当前交付无关) → 走 no_task 强推 task (新建 / 排队), 不打断当前 agent。
   - **判不准** → 🔴 用 AskUserQuestion 让用户裁定「并入当前任务 / 另起新任务」, 禁擅自二选一。
   兜底: 目标 agent 已完成 / workflow 模式无法中途 SendMessage → 改在 check 阶段按新 PRD 纠正, 或停掉重派一个修正 agent; inline 单交付 (无 running agent) → main 改 PRD 后就地调整执行, 跳过 SendMessage。
<!-- trellisx:end:in_progress -->
[/workflow-state:in_progress]

### Phase 3: Finish
- 3.1 质量验证 `[required · repeatable]`
- 3.2 调试复盘 `[on demand]`
- 3.3 Spec 更新 `[required · once]`
- 3.4 提交改动 `[required · once]`
- 3.5 收尾提醒

[workflow-state:completed]
代码已经 Phase 3.4 提交; 运行 `/trellis:finish-work` 收尾 (归档 task + 记录 session)。
若到达此状态时仍有未提交代码, 先回 Phase 3.4 —— `/finish-work` 拒绝在脏工作区运行。
`task.py archive` 会删除所有仍指向被归档 task 的 runtime session 文件。
[/workflow-state:completed]

### 规则

1. 先识别你处在哪个 Phase, 再从那里的下一步继续
2. 每个 Phase 内按顺序执行各步; `[required]` 步骤不能跳过
3. Phase 可回滚 (例如 Execute 暴露 prd 缺陷 → 回 Plan 修, 再重新进入 Execute)
4. 标 `[once]` 的步骤若产出已存在则跳过; 不要重跑

### Skill 路由

当用户请求匹配以下意图之一时, 先加载对应 skill (或派对应 sub-agent) —— 不要跳过 skill。

[Claude Code]

| 用户意图 | 路由 |
|---|---|
| 想要新功能 / 需求不清 | `trellis-brainstorm` |
| 即将写代码 / 开始实现 | 按 Phase 2.1 派 `trellis-implement` sub-agent |
| 写完了 / 想验证 | 按 Phase 2.2 派 `trellis-check` sub-agent |
| 卡住 / 同一 bug 反复修了几次 | `trellis-break-loop` |
| Spec 需要更新 | `trellis-update-spec` |

**为什么 `trellis-before-dev` 不在此表:** 写代码的不是你 —— 是 `trellis-implement` sub-agent。Sub-agent 平台通过 `implement.jsonl` 注入 / prelude 获得 spec context, 不是靠主线程加载 `trellis-before-dev`。

[/Claude Code]

### 不要跳过 skill

[Claude Code]

| 你心里在想 | 为何这是错的 |
|---|---|
| "这很简单, 我直接在主线程写代码" | 派 `trellis-implement` 是省事的路径; 跳过它会诱使你在主线程写代码并丢失 spec context —— sub-agent 被注入 `implement.jsonl`, 你没有 |
| "我在 plan mode 已经想清楚了" | Plan-mode 产出只存在记忆里 —— sub-agent 看不到; 必须落盘到 prd.md |
| "我已经知道 spec 了" | spec 可能在你上次读后已更新; sub-agent 拿到新副本, 你可能没有 |
| "先写代码, 后检查" | `trellis-check` 会发现你自己注意不到的问题; 越早越省 |

[/Claude Code]

### 加载步骤细节

每一步运行这个来获取详细指引:

```bash
python3 ./.trellis/scripts/get_context.py --mode phase --step <step>
# 例如 python3 ./.trellis/scripts/get_context.py --mode phase --step 1.1
```

---

## Phase 1: Plan

目标: 想清楚要构建什么, 产出清晰的需求文档与实现所需的 context。

#### 1.0 创建 task `[required · once]`

创建 task 目录 (status 进入 `planning`, 当 session identity 可用时 session active-task 指针自动指向新 task):

```bash
python3 ./.trellis/scripts/task.py create "<task title>" --slug <name>
```

`--slug` 仅为人类可读名。**不要**包含 `MM-DD-` 日期前缀; `task.py create` 会自动加该前缀。

此命令成功后, per-turn breadcrumb 自动切到 `[workflow-state:planning]`, 告诉 AI 进入 brainstorm + jsonl 整理阶段。

⚠️ **此处只运行 `create` —— 不要同时运行 `start`**。`start` 把 status 翻为 `in_progress`, 会在 brainstorm + jsonl 完成前就把 breadcrumb 切到实现阶段 —— AI 会静默跳过它们。把 `start` 留到步骤 1.4, jsonl 整理完成之后。

当 `python3 ./.trellis/scripts/task.py current --source` 已指向某 task 时跳过。

#### 1.1 需求探索 `[required · repeatable]`

加载 `trellis-brainstorm` skill, 按 skill 指引与用户交互式探索需求。

brainstorm skill 会引导你:
- 一次只问一个问题
- 优先调研而非问用户
- 优先给选项而非开放式提问
- 每次用户回答后立即更新 `prd.md`

需求变化时随时回到此步并修订 `prd.md`。

#### 1.2 调研 `[optional · repeatable]`

调研可在需求探索期间随时发生。不限于本地代码 —— 你可以用任何可用工具 (MCP servers、skills、web search 等) 查外部信息, 包括第三方库文档、行业实践、API 参考等。

[Claude Code]

派 research sub-agent:

- **Agent type**: `trellis-research`
- **Task description**: Research <specific question>
- **Key requirement**: Research output MUST be persisted to `{TASK_DIR}/research/`

[/Claude Code]

**Research 产物约定**:
- 每个调研主题一个文件 (例如 `research/auth-library-comparison.md`)
- 把第三方库用法示例、API 参考、版本约束记进文件
- 记下你发现的相关 spec 文件路径供日后参考

Brainstorm 与 research 可自由交错 —— 暂停去调研一个技术问题, 再回来与用户对话。

**关键原则**: Research 产出必须写进文件, 不能只留在对话里。对话会被压缩; 文件不会。

#### 1.3 配置 context `[required · once]`

[Claude Code]

整理 `implement.jsonl` 与 `check.jsonl`, 使 Phase 2 sub-agent 拿到正确的 spec context。这些文件在 `task create` 时以单行自描述 `_example` 播种; 你这里的任务是填入真实条目。

**位置**: `{TASK_DIR}/implement.jsonl` 与 `{TASK_DIR}/check.jsonl` (已存在)。

**格式**: 每行一个 JSON 对象 —— `{"file": "<path>", "reason": "<why>"}`。路径相对 repo 根。

**该放什么**:
- **Spec 文件** —— 与本 task 相关的 `.trellis/spec/<package>/<layer>/index.md` 及任何具体指南文件 (`error-handling.md`、`conventions.md` 等)
- **Research 文件** —— sub-agent 需查阅的 `{TASK_DIR}/research/*.md`

**不该放什么**:
- 代码文件 (`src/**`、`packages/**/*.ts` 等) —— 这些由 sub-agent 在实现期间读取, 不在此预注册
- 你即将修改的文件 —— 同理

**两个文件如何分配**:
- `implement.jsonl` → implement sub-agent 正确写代码所需的 specs + research
- `check.jsonl` → check sub-agent 的 specs (质量指南、检查约定, 需要时同样的 research)

**如何发现相关 spec**:

```bash
python3 ./.trellis/scripts/get_context.py --mode packages
```

列出每个 package + 其 spec layers 及路径。挑出匹配本 task 领域的条目。

**如何追加条目**:

直接在编辑器里改 jsonl 文件, 或用:

```bash
python3 ./.trellis/scripts/task.py add-context "$TASK_DIR" implement "<path>" "<reason>"
python3 ./.trellis/scripts/task.py add-context "$TASK_DIR" check "<path>" "<reason>"
```

有真实条目后删除 seed `_example` 行 (可选 —— 消费方会自动跳过它)。

跳过条件: `implement.jsonl` 已有 agent 整理过的条目 (单凭 seed 行不算)。

[/Claude Code]

#### 1.4 激活 task `[required · once]`

prd.md 完成且 1.3 jsonl 整理好后, 把 task status 翻为 `in_progress`:

```bash
python3 ./.trellis/scripts/task.py start <task-dir>
```

此命令成功后, breadcrumb 自动切到 `[workflow-state:in_progress]`, 之后走 Phase 2 / 3 的其余部分。

若 `task.py start` 报 session-identity 错 (hook input、`TRELLIS_CONTEXT_ID` 或平台原生 session env 都拿不到 context key), 按错误里的提示配置 session identity, 然后重试。

#### 1.5 完成标准

| 条件 | 必需 |
|------|:---:|
| `prd.md` 存在 | ✅ |
| 用户确认需求 | ✅ |
| `task.py start` 已运行 (status = in_progress) | ✅ |
| `research/` 有产物 (复杂 task) | 推荐 |
| `info.md` 技术设计 (复杂 task) | 可选 |

[Claude Code]

| `implement.jsonl` 有 agent 整理过的条目 (不止 seed 行) | ✅ |

[/Claude Code]

---

## Phase 2: Execute

目标: 把 prd 变成通过质量检查的代码。

#### 2.1 实现 `[required · repeatable]`

[Claude Code]

派 implement sub-agent:

- **Agent type**: `trellis-implement`
- **Task description**: Implement the requirements per prd.md, consulting materials under `{TASK_DIR}/research/`; finish by running project lint and type-check
- **Dispatch prompt guard**: Tell the spawned agent it is already the `trellis-implement` sub-agent and must implement directly, not spawn another `trellis-implement` / `trellis-check`.

平台 hook/plugin 自动处理:
- 读取 `implement.jsonl` 并把引用的 spec 文件注入 agent prompt
- 注入 prd.md 内容

[/Claude Code]

#### 2.2 质量检查 `[required · repeatable]`

[Claude Code]

派 check sub-agent:

- **Agent type**: `trellis-check`
- **Task description**: Review all code changes against spec and prd; fix any findings directly; ensure lint and type-check pass
- **Dispatch prompt guard**: Tell the spawned agent it is already the `trellis-check` sub-agent and must review/fix directly, not spawn another `trellis-check` / `trellis-implement`.

check agent 的职责:
- 对照 spec 审查代码改动
- 自动修复发现的问题
- 运行 lint 与 typecheck 验证

[/Claude Code]

#### 2.3 回滚 `[on demand]`

- `check` 暴露 prd 缺陷 → 回 Phase 1, 修 `prd.md`, 再重做 2.1
- 实现出错 → 还原代码, 重做 2.1
- 需要更多调研 → 调研 (同 Phase 1.2), 把发现写进 `research/`

---

## Phase 3: Finish

目标: 确保代码质量, 沉淀教训, 记录工作。

#### 3.1 质量验证 `[required · repeatable]`

加载 `trellis-check` skill 做最终验证:
- Spec 符合度
- lint / type-check / tests
- 跨 layer 一致性 (改动跨 layer 时)

发现问题 → 修复 → 重检, 直至全绿。

#### 3.2 调试复盘 `[on demand]`

若本 task 涉及反复调试 (同一问题被修了多次), 加载 `trellis-break-loop` skill 来:
- 归类根因
- 解释为何早先的修复失败
- 提出防范

目的是沉淀调试教训, 使同类问题不再复发。

#### 3.3 Spec 更新 `[required · once]`

加载 `trellis-update-spec` skill, 审视本 task 是否产出值得记录的新知识:
- 新发现的模式或约定
- 你踩过的坑
- 新技术决策

据此更新 `.trellis/spec/` 下的文档。即便结论是"无需更新", 也走一遍这个判断。

#### 3.4 提交改动 `[required · once]`

AI 驱动本 task 代码改动的批量提交, 使 `/finish-work` 之后能干净运行。目标: 先产出工作提交 (work commits), 再落记账提交 (archive + journal) —— 绝不交错。

**逐步**:

1. **检查脏状态**:
   ```bash
   git status --porcelain
   ```
   快照每个脏路径。若工作区干净, 跳到 3.5。

2. **从近期历史学习提交风格** (使草拟的 message 融入):
   ```bash
   git log --oneline -5
   ```
   记下前缀约定 (`feat:` / `fix:` / `chore:` / `docs:` ...)、语言 (中文/English) 与长度风格。

3. **把脏文件分为两组**:
   - **本 session AI 编辑过** — 你在本 session 通过 Edit/Write/Bash 工具调用写/改的文件。你清楚改了什么、为何改。
   - **不认识的** — 本 session 你没碰过的脏文件 (可能是用户手改、上一 session 的残留 WIP, 或无关工作)。不要静默纳入这些。

4. **草拟提交计划**。把 AI 编辑过的文件归入逻辑提交 (一个连贯改动单元一个提交, 不是一文件一提交)。每条: `<commit message>` + 文件清单。不认识的文件单列在底部。

5. **一次性展示计划, 请求一次确认**。格式:
   ```
   Proposed commits (in order):
     1. <message>
        - <file>
        - <file>
     2. <message>
        - <file>

   Unrecognized dirty files (NOT in any commit — confirm include/exclude):
     - <file>
     - <file>

   Reply 'ok' / '行' to execute. Reply with edits, or '我自己来' / 'manual' to abort.
   ```

6. **确认后**: 对每个批次按顺序运行 `git add <files>` + `git commit -m "<msg>"`。不要 amend。不要 push。

7. **被拒后** (用户回 "不行" / "我自己来" / "manual" / 对计划的任何反对): 停下。不要尝试第二个计划。用户会手动提交; 待其确认后你跳到 3.5。

**规则**:
- 任何地方都不用 `git commit --amend` —— 三阶段三提交流程 (work commits → archive commit → journal commit)。
- 本步骤绝不 push 到远端。
- 若用户想要不同的 message 措辞但接受文件分组, 改 message 并重新确认一次 —— 但若他们拒绝分组, 退出到手动模式。
- 批量计划是一个 prompt; 不要逐提交提示。

#### 3.5 收尾提醒

<!-- trellisx:start:finish_force -->
⛔ **强制自动收尾 (不是提醒)**: check 通过后, AI **必须**运行
`python3 .trellis/scripts/task.py finish` (或 `/trellis:finish-work`)。
`after_finish` hook **自动**执行: 提交 worktree → 合并 --no-ff 回主分支 → archive → 销毁 worktree。
**worktree 删除与合并是必须的, 非可选, 非「提醒用户去做」。**
- **收尾两层, 责任不同 (step⓪ AI 层先于 ① git 层)**:
  - **⓪ AI 层 (脚本做不到, 必须 AI 主动)**: 跑收尾脚本前, 先确认本 task 的 **Workflow / 后台 agent 任务已全部终止** —— 用 `TaskList` 查有无悬挂的后台 Workflow/agent (Claude Code Workflow 或退化的 agent 流水线), 有残留则 `TaskStop` 逐个关闭, 再跑 finish。**`trellisx-finish.py` 只销 worktree, 不关 Workflow/Task** —— 关闭悬挂任务是 AI 层职责, 脚本不代劳。
  - **① git 层 (确定性脚本 `trellisx-finish.py`)**: commit → merge --no-ff 各子分支 (子先主后) → 销毁 worktree → archive。冲突则 abort + 报清单, 不强解。
  - 顺序: 先 ⓪ AI 层清悬挂任务 → 再 ① git 层 finish。悬挂任务未清就收尾 = worktree 被合并/销毁时仍有进程在写 = 流程错误。
- 合并冲突 → 收尾脚本 abort + 报冲突 + 非 0 退出 (hook 失败仅 WARN, `task.py finish` 仍返 0)。
  AI **MUST 检 finish 输出有无 `trellisx-finish` 冲突告警**, 有则转手动解决, 禁强解 / 禁当成功。
- check 未过禁跑 finish; 未 archive (worktree 仍在) = 流程未闭环, 禁宣告 Done。
- commit 为 owner 授权的强制动作 (脚本直接提交); 需会话 journal 时用 `/trellis:finish-work` 入口。
- 手动兜底: 链路异常时可直接跑 `python3 .trellis/scripts/trellisx-finish.py [--task <tid>]` (幂等可重入)。
<!-- trellisx:end:finish_force -->

---

## 定制 Trellis (for forks)

本节面向想修改 Trellis workflow 本身的开发者。所有定制都通过编辑本文件完成; 脚本只是 parser。

### 改变某步骤的含义

编辑上面 Phase 1 / 2 / 3 各节中对应步骤的 walkthrough 正文。**关键约束**: 若你改了某步骤的 `[required · once]` 标记或新增一个 `[required · once]` 步骤, 你 MUST 同时给该 phase 的 `[workflow-state:STATUS]` tag 块加一条匹配的强制行 —— 否则 per-turn breadcrumb 漏掉这条强化, AI 会静默跳过该步骤。回归测试会断言这一点。

全部 4 个 tag 块位于上面 `## Phase 索引` 节, 紧跟在每个 phase 摘要之后:

| 作用域 | 对应 tag |
|---|---|
| 无 active task (Phase 1 之前) | `[workflow-state:no_task]` (在 Phase 索引 ASCII 图之后) |
| 整个 Phase 1 (task 已创建 → 准备实现) | `[workflow-state:planning]` (在 Phase 1 摘要之后) |
| Phase 2 + Phase 3.1–3.4 (实现 + check + 收尾) | `[workflow-state:in_progress]` (在 Phase 2 摘要之后) |
| Phase 3.5 之后 (已归档) | `[workflow-state:completed]` (在 Phase 3 摘要之后; **当前为 DEAD**) |

### 改变 per-turn prompt 文本

直接编辑对应 `[workflow-state:STATUS]` 块的正文。编辑后, 运行 `trellis update` (若你是模板维护者) 或重启 AI session (若你在定制自己的项目) —— 无需改脚本。

### 新增自定义 status

加一个新块:

```
[workflow-state:my-status]
your per-turn prompt text
[/workflow-state:my-status]
```

约束:
- STATUS 字符集: `[A-Za-z0-9_-]+` (允许下划线与连字符, 例如 `in-review`、`blocked-by-team`)
- 必须有一个生命周期 hook 把 `task.json.status` 写为你的自定义值, 否则该 tag 永远不被读
- 生命周期 hook 位于 `task.json.hooks.after_*`, 绑定到 `after_create / after_start / after_finish / after_archive` 之一

### 新增生命周期 hook

给你的 `task.json` 加一个 `hooks` 字段:

```json
{
  "hooks": {
    "after_finish": [
      "your-script-or-command-here"
    ]
  }
}
```

支持的事件: `after_create / after_start / after_finish / after_archive`。注意 `after_finish` ≠ status 变更 (它只清 active-task 指针); "task 完成" 类通知用 `after_archive`。

### 完整契约

workflow 状态机的运行时契约、所有 status writer 的位置、伪状态 (`no_task` / `stale_<source_type>`)、hook 可达性矩阵及其他深层细节, 见:

- `.trellis/spec/cli/backend/workflow-state-contract.md` — 运行时契约 + writer 表 + 测试不变量
- `.trellis/scripts/inject-workflow-state.py` — 实际 parser (只读 workflow.md, 无内嵌文本)
