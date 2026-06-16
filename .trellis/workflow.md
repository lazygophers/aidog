# Development Workflow

---

## Core Principles

1. **Plan before code** — figure out what to do before you start
2. **Specs injected, not remembered** — guidelines are injected via hook/skill, not recalled from memory
3. **Persist everything** — research, decisions, and lessons all go to files; conversations get compacted, files don't
4. **Incremental development** — one task at a time
5. **Capture learnings** — after each task, review and write new knowledge back to spec

---

## Trellis System

### Developer Identity

On first use, initialize your identity:

```bash
python3 ./.trellis/scripts/init_developer.py <your-name>
```

Creates `.trellis/.developer` (gitignored) + `.trellis/workspace/<your-name>/`.

### Spec System

`.trellis/spec/` holds coding guidelines organized by package and layer.

- `.trellis/spec/<package>/<layer>/index.md` — entry point with **Pre-Development Checklist** + **Quality Check**. Actual guidelines live in the `.md` files it points to.
- `.trellis/spec/guides/index.md` — cross-package thinking guides.

```bash
python3 ./.trellis/scripts/get_context.py --mode packages   # list packages / layers
```

**When to update spec**: new pattern/convention found · bug-fix prevention to codify · new technical decision.

### Task System

Every task has its own directory under `.trellis/tasks/{MM-DD-name}/` holding `prd.md`, `implement.jsonl`, `check.jsonl`, `task.json`, optional `research/`, `info.md`.

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

> Run `python3 ./.trellis/scripts/task.py --help` to see the authoritative, up-to-date list.

**Current-task mechanism**: `task.py create` creates the task directory and (when session identity is available) auto-sets the per-session active-task pointer so the planning breadcrumb fires immediately. `task.py start` writes the same pointer (idempotent if already set) and flips `task.json.status` from `planning` to `in_progress`. State is stored under `.trellis/.runtime/sessions/`. If no context key is available from hook input, `TRELLIS_CONTEXT_ID`, or a platform-native session environment variable, there is no active task and `task.py start` fails with a session identity hint. `task.py finish` deletes the current session file (status unchanged). `task.py archive <task>` writes `status=completed`, moves the directory to `archive/`, and deletes any runtime session files that still point at the archived task.

### Workspace System

Records every AI session for cross-session tracking under `.trellis/workspace/<developer>/`.

- `journal-N.md` — session log. **Max 2000 lines per file**; a new `journal-(N+1).md` is auto-created when exceeded.
- `index.md` — personal index (total sessions, last active).

```bash
python3 ./.trellis/scripts/add_session.py --title "Title" --commit "hash" --summary "Summary"
```

### Context Script

```bash
python3 ./.trellis/scripts/get_context.py                            # full session runtime
python3 ./.trellis/scripts/get_context.py --mode packages            # available packages + spec layers
python3 ./.trellis/scripts/get_context.py --mode phase --step <X.Y>  # detailed guide for a workflow step
```

---

## Phase Index

```
Phase 1: Plan    → figure out what to do (brainstorm + research → prd.md)
Phase 2: Execute → write code and pass quality checks
Phase 3: Finish  → distill lessons + wrap-up
```

[workflow-state:no_task]
No active task. **A Direct answer** — pure Q&A / explanation / lookup / chat; no file writes + one-line answer + repo reads ≤ 2 files → AI judges, no override needed.
**B Create a task** — any implementation / code change / build / refactor work. Entry sequence: (1) `python3 ./.trellis/scripts/task.py create "<title>"` to create the task (status=planning, breadcrumb switches to [workflow-state:planning] for brainstorm + jsonl phase guidance) → (2) load `trellis-brainstorm` skill to discuss requirements with the user and iterate on prd.md → (3) once prd is done and jsonl is curated, run `task.py start <task-dir>` to enter [workflow-state:in_progress] for the implementation skeleton. **"It looks small" is NOT grounds for downgrading B to A or C**.
**C Inline change** (per-turn only, escape hatch for B) — the user's CURRENT message MUST contain one of: "skip trellis" / "no task" / "just do it" / "don't create a task" / "跳过 trellis" / "别走流程" / "小修一下" / "直接改" / "先别建任务" → briefly acknowledge ("ok, skipping trellis flow this turn"), then inline. **Without seeing one of these phrases you must NOT inline on your own**; do not invent an override the user never said.

<!-- trellisx:start:no_task -->
trellisx 规约 (强化原生判定, 不切断建 task 路径): 本项目愿景 = 除极简任务外, 一律走 Trellis task 执行。
- 涉及代码改动 / 多步骤 / 跨文件 的请求 → **默认创建 task** (不是"倾向建", 是默认就建; 仅用户明确拒绝才 inline)。建议直接用 AskUserQuestion 征得同意并进入 planning, 而非自行 inline。
- 边界模糊 (拿不准该不该建, 或 simple 还是 complex) → **MUST 主动用 AskUserQuestion 问用户** "本轮是否创建 Trellis task?", 禁默认跳过 / 禁自行替用户决定。
- 仅极简任务 (纯问答 / 查询 / 单行琐改 / 纯解释说明) 可不建。
- 判断"新建 task"还是"并入现有 task"时, 读 `.trellis/task.md` 看板对照现有任务 (id/名称/描述/状态) 辅助判断。
原生的「先分类 + 征得同意才建」不变 — 但默认倾向从"可建可不建"上调为"默认建, 除非极简或用户拒绝"。
<!-- trellisx:end:no_task -->
[/workflow-state:no_task]

### Phase 1: Plan
- 1.0 Create task `[required · once]` (just `task.py create`; status enters planning)
- 1.1 Requirement exploration `[required · repeatable]`
- 1.2 Research `[optional · repeatable]`
- 1.3 Configure context `[required · once]` — Claude Code
- 1.4 Activate task `[required · once]` (run `task.py start`; status → in_progress)
- 1.5 Completion criteria

[workflow-state:planning]
Load the `trellis-brainstorm` skill and iterate on prd.md with the user.
Phase 1.3 (required, once): before `task.py start`, you MUST curate `implement.jsonl` and `check.jsonl` — list the spec / research files sub-agents need so they get the right context injected. You may skip only if the jsonl already has agent-curated entries (the seed `_example` row alone doesn't count).
Then run `task.py start <task-dir>` to flip status to in_progress.

<!-- trellisx:start:planning -->
trellisx 规划规约 (启用判定跟随 trellis 原生 parent/child 语义, 不看数量):

⚙️ **规划产出派后台 agent**: 写 `prd.md`/`design.md`/`implement.md` 等实质产出**派 agent** (`run_in_background: true`) 完成 (经 `trellis-brainstorm` + `trellisx-orchestrate`), main 禁亲自写; 需求澄清问题由 agent 在返回标 `需要:`, main 用 `AskUserQuestion` 转达用户。产物评审 (`AskUserQuestion`) 由 main 亲做。

判定: 本请求是否含**多个独立可验收交付** (各自可独立 plan/implement/check/archive)?
- **是 (多交付)** → 拆为 parent + child tasks (trellis 原生 `task.py create --parent`)。每个 child 独立 worktree; PRD MUST 含 mermaid 调度图显式标并行组 + 依赖箭头; child 间依赖写进 child 自己的 prd.md/implement.md (非树位置隐含)。**执行统一由 `trellis-implement` 入口调度** (main 派 trellis-implement, 由其对各 subtask 派专用 subagent 并行执行), main 不直接派 subtask agent。
- **否 (单一交付)** → 轻量单 task inline, **不强制拆 subtask**。仍走单 worktree 隔离。

拆分目的 = 让独立可验收交付各自隔离 + 最大化并行, 缩短关键路径; 不是为凑数量。详见 trellisx-orchestrate skill。

task 创建后, 用 `trellisx-workspace` 及时更新 `.trellis/task.md` 看板表 (新增/更新该任务行)。
<!-- trellisx:end:planning -->
[/workflow-state:planning]

### Phase 2: Execute
- 2.1 Implement `[required · repeatable]`
- 2.2 Quality check `[required · repeatable]`
- 2.3 Rollback `[on demand]`

[workflow-state:in_progress]
**Tools**: `trellis-implement` / `trellis-research` are sub-agent types only (Task/Agent tool, NOT Skill — there is no skill by these names). `trellis-update-spec` is a skill. `trellis-check` exists as both; prefer the Agent form when verifying after code changes.
**Flow**: trellis-implement → trellis-check → trellis-update-spec → commit (Phase 3.4) → `/trellis:finish-work`.
**Main-session default (no override)**: dispatch the `trellis-implement` / `trellis-check` sub-agents — the main agent does NOT edit code by default. Phase 3.4 commit (required, once): after trellis-update-spec, or whenever implementation is verifiably complete, the main agent **drives the commit** — state the commit plan in user-facing text, then run `git commit` — BEFORE suggesting `/trellis:finish-work`. `/finish-work` refuses to run on a dirty working tree (paths outside `.trellis/workspace/` and `.trellis/tasks/`).
**Sub-agent self-exemption**: if you are already running as `trellis-implement`, implement directly from the loaded task context and do NOT spawn another `trellis-implement`; if you are already running as `trellis-check`, review/fix directly and do NOT spawn another `trellis-check`. The default dispatch rule applies to the main session only.
**Sub-agent dispatch 协议 (所有 sub-agent)**: 派发 `trellis-implement` / `trellis-check` / `trellis-research` 时, dispatch prompt **MUST** 首行写 `Active task: <task path from `task.py current`>`, 无例外。此行是 hook 注入失败时的兜底 (`--continue` resume / hook 被禁用等); 对 `trellis-research` 还告诉它往哪个 `{task_dir}/research/` 写。
**Inline override** (per-turn only, escape hatch for sub-agent dispatch): the user's CURRENT message MUST explicitly contain one of: "do it inline" / "no sub-agent" / "你直接改" / "别派 sub-agent" / "main session 写就行" / "不用 sub-agent". **Without seeing one of these phrases you must NOT inline on your own**; do not invent an override the user never said.

<!-- trellisx:start:in_progress -->
⛔ trellisx 执行硬规 (本 task 必守, 违反即流程错误):

0. **全程 agent + 后台异步 (最高优先级)**: main **禁直接落地任何实质工作** (写 prd/design/implement、改源码、跑 check、commit/archive) —— 一律派 agent 执行, 且 `run_in_background: true` **真异步** (main 不阻塞, agent 完成回调)。main 只做编排 + 用户交互决策 (`AskUserQuestion` 后台 agent 不能做) + 完成即时回传 + 看板维护。无依赖交付**同一回复一次性派多个后台 agent** (真并行)。每个 dispatch prompt 须 6 字段自包含 (目标/已知含 `Active task:`/范围/输出/验收/失败处理)。
1. **强制 worktree** (两种模式都守): 本 task 全部源码改动 MUST 落在 worktree (git 根/子仓 .worktrees/<worktree>, trellis 生命周期 hook 已自动建)。**禁在主工作区写源码** — 写盘 file_path 必须是 worktree 路径。
2. **实施派发模型 (main → trellis-implement → 专用 subagent, 全后台异步)**: 进入实施, main **派一个 `trellis-implement` 子代理** (`run_in_background: true`) 执行实施阶段, **main 禁直接派 subtask agent、禁直接写源码** (只派 implement/check + 进度回传 + 合并)。`trellis-implement` 读 `implement.md`, **对每个 subtask 派专用 subagent (isolation:worktree)**; 无依赖的 subtask MUST 在同一条回复里一次性发起多个 subagent 调用 (真并行), 禁逐个串行; 严格按 implement.md / PRD 调度图依赖 + 并行组执行。trellis-implement 收拢全部 subtask 产物 → 交 `trellis-check`。循环: planning → trellis-implement → trellis-check → finish。
3. **轻量模式** (单 subtask): main 仍**派 `trellis-implement`** (`run_in_background: true`), 由其在 worktree 内**内联直做** (无需再派 subagent), 保持 planning → implement → check → finish 循环一致。**main 不绕过 implement 自己写源码** (仍守第 1 条: 写盘路径在 worktree)。
4. **强制自动收尾 (check 通过后跑收尾, 全链自动, 不停在「提醒」)**: `trellis-check` 通过后, AI **强制收尾**, **两路任选其一, 都做全链** (commit→merge→del worktree→archive): ① `python3 .trellis/scripts/task.py finish` → `after_finish` hook 自动跑 `trellisx-finish.py`; ② `/trellis:finish-work` → apply 已注入它**先跑 `trellisx-finish.py` 全链再 journal** (修原生 archive-direct 绕 merge 丢提交)。不把收尾降级为"提醒用户" —— **合并与 worktree 删除是必须的, 非可选**:
   ① **worktree 内提交** —— `git add -A` + `git commit` (缺省消息 `chore(task): <tid> 收尾提交`), 落到 `trellisx-<name>` 分支。
   ② **合并回主分支** —— `git merge --no-ff trellisx-<name>`; **合并冲突 → 脚本自动 `merge --abort` + 报冲突文件清单 + 非 0 退出**。hook 失败仅 WARN, `task.py finish` 仍返 0 → AI **MUST 检 finish 输出有无 `trellisx-finish` 冲突告警**, 有则停下转手动, 禁强解 / 禁当成功。**未合并就 archive = 分支被销毁丢提交** (见 `trellisx-worktree.py` 教训, 故脚本先合并)。
   ③ **archive** —— 链内 `task.py archive <tid>` 归档 + 触发 `after_archive` 销毁**已合并**的 worktree + 删分支。
   - **commit 强制 (owner 授权)**: 本项目 owner 已显式要求 trellisx 流程**强制提交收尾**, 脚本直接 commit 不逐次等确认。此为 owner 对自有注入流程的设计授权, 覆盖默认「禁主动 commit」。
   - **check 未过** → 修复重检, 禁跑 finish; **未 archive (worktree 仍在) = 流程未闭环, 禁宣告 Done / 禁结束本轮**。
   - 手动兜底: 链路异常 → 直接跑 `trellisx-finish.py [--task <tid>]` (幂等可重入)。
5. **及时维护 task.md 看板**: start / 阶段推进 (exec→check→finish) / archive 后, MUST 用 `trellisx-workspace` 更新 `.trellis/task.md` 看板行 (状态/阶段/进度/worktree)。看板滞后于实际 = 流程缺陷。
6. 收每个 agent 返回立即回传用户进度; task archive 时 worktree 干净则自动销毁。
7. **任务中途修正路由 (执行中收到用户新指令)**: 本 task 已在跑 (agent/member 已派发) 时收到新指令, coordinator 先判归属:
   - **属当前任务** (修正 / 补充 / 细化已派交付) → ⛔ 禁 main 自己直接改源码、禁开新 task。按序: ① 先改对应**真值文档** (`prd.md` / `design.md` / `implement.md` 受影响条款, 标锚点) → ② 对**仍在跑**的目标 agent/member 用 `SendMessage` 下发修正 (引用改后 PRD 锚点, 令其就地纠偏, 不等跑完返工)。**先改文档再通知** (PRD 是真值, agent 复读以对齐)。
   - **独立新任务** (与当前交付无关) → 走 no_task 强推 task (新建 / 排队), 不打断当前 agent。
   - **判不准** → 🔴 用 AskUserQuestion 让用户裁定「并入当前任务 / 另起新任务」, 禁擅自二选一。
   兜底: 目标 agent 已完成 / workflow 模式无法中途 SendMessage → 改在 check 阶段按新 PRD 纠正, 或停掉重派一个修正 agent; inline 单交付 (无 running agent) → main 改 PRD 后就地调整执行, 跳过 SendMessage。
<!-- trellisx:end:in_progress -->
[/workflow-state:in_progress]

### Phase 3: Finish
- 3.1 Quality verification `[required · repeatable]`
- 3.2 Debug retrospective `[on demand]`
- 3.3 Spec update `[required · once]`
- 3.4 Commit changes `[required · once]`
- 3.5 Wrap-up reminder

[workflow-state:completed]
Code committed via Phase 3.4; run `/trellis:finish-work` to wrap up (archive the task + record session).
If you reach this state with uncommitted code, return to Phase 3.4 first — `/finish-work` refuses to run on a dirty working tree.
`task.py archive` deletes any runtime session files that still point at the archived task.
[/workflow-state:completed]

### Rules

1. Identify which Phase you're in, then continue from the next step there
2. Run steps in order inside each Phase; `[required]` steps can't be skipped
3. Phases can roll back (e.g., Execute reveals a prd defect → return to Plan to fix, then re-enter Execute)
4. Steps tagged `[once]` are skipped if the output already exists; don't re-run

### Skill Routing

When a user request matches one of these intents, load the corresponding skill (or dispatch the corresponding sub-agent) first — do not skip skills.

[Claude Code]

| User intent | Route |
|---|---|
| Wants a new feature / requirement unclear | `trellis-brainstorm` |
| About to write code / start implementing | Dispatch the `trellis-implement` sub-agent per Phase 2.1 |
| Finished writing / want to verify | Dispatch the `trellis-check` sub-agent per Phase 2.2 |
| Stuck / fixed same bug several times | `trellis-break-loop` |
| Spec needs update | `trellis-update-spec` |

**Why `trellis-before-dev` is NOT in this table:** you are not the one writing code — the `trellis-implement` sub-agent is. Sub-agent platforms get spec context via `implement.jsonl` injection / prelude, not via the main thread loading `trellis-before-dev`.

[/Claude Code]

### DO NOT skip skills

[Claude Code]

| What you're thinking | Why it's wrong |
|---|---|
| "This is simple, I'll just code it in the main thread" | Dispatching `trellis-implement` is the cheap path; skipping it tempts you to write code in the main thread and lose spec context — sub-agents get `implement.jsonl` injected, you don't |
| "I already thought it through in plan mode" | Plan-mode output lives in memory — sub-agents can't see it; must be persisted to prd.md |
| "I already know the spec" | The spec may have been updated since you last read it; the sub-agent gets the fresh copy, you may not |
| "Code first, check later" | `trellis-check` surfaces issues you won't notice yourself; earlier is cheaper |

[/Claude Code]

### Loading Step Detail

At each step, run this to fetch detailed guidance:

```bash
python3 ./.trellis/scripts/get_context.py --mode phase --step <step>
# e.g. python3 ./.trellis/scripts/get_context.py --mode phase --step 1.1
```

---

## Phase 1: Plan

Goal: figure out what to build, produce a clear requirements doc and the context needed to implement it.

#### 1.0 Create task `[required · once]`

Create the task directory (status enters `planning`, the session active-task pointer auto-targets the new task when session identity is available):

```bash
python3 ./.trellis/scripts/task.py create "<task title>" --slug <name>
```

`--slug` is the human-readable name only. Do **not** include the `MM-DD-` date prefix; `task.py create` adds that prefix automatically.

After this command succeeds, the per-turn breadcrumb auto-switches to `[workflow-state:planning]`, telling the AI to enter the brainstorm + jsonl curation phase.

⚠️ **Run only `create` here — do not also run `start`**. `start` flips status to `in_progress`, which switches the breadcrumb to the implementation phase before brainstorm + jsonl are done — the AI will silently skip them. Save `start` for step 1.4, after jsonl curation is complete.

Skip when `python3 ./.trellis/scripts/task.py current --source` already points to a task.

#### 1.1 Requirement exploration `[required · repeatable]`

Load the `trellis-brainstorm` skill and explore requirements interactively with the user per the skill's guidance.

The brainstorm skill will guide you to:
- Ask one question at a time
- Prefer researching over asking the user
- Prefer offering options over open-ended questions
- Update `prd.md` immediately after each user answer

Return to this step whenever requirements change and revise `prd.md`.

#### 1.2 Research `[optional · repeatable]`

Research can happen at any time during requirement exploration. It isn't limited to local code — you can use any available tool (MCP servers, skills, web search, etc.) to look up external information, including third-party library docs, industry practices, API references, etc.

[Claude Code]

Spawn the research sub-agent:

- **Agent type**: `trellis-research`
- **Task description**: Research <specific question>
- **Key requirement**: Research output MUST be persisted to `{TASK_DIR}/research/`

[/Claude Code]

**Research artifact conventions**:
- One file per research topic (e.g. `research/auth-library-comparison.md`)
- Record third-party library usage examples, API references, version constraints in files
- Note relevant spec file paths you discovered for later reference

Brainstorm and research can interleave freely — pause to research a technical question, then return to talk with the user.

**Key principle**: Research output must be written to files, not left only in the chat. Conversations get compacted; files don't.

#### 1.3 Configure context `[required · once]`

[Claude Code]

Curate `implement.jsonl` and `check.jsonl` so the Phase 2 sub-agents get the right spec context. These files were seeded on `task create` with a single self-describing `_example` line; your job here is to fill in real entries.

**Location**: `{TASK_DIR}/implement.jsonl` and `{TASK_DIR}/check.jsonl` (already exist).

**Format**: one JSON object per line — `{"file": "<path>", "reason": "<why>"}`. Paths are repo-root relative.

**What to put in**:
- **Spec files** — `.trellis/spec/<package>/<layer>/index.md` and any specific guideline files (`error-handling.md`, `conventions.md`, etc.) relevant to this task
- **Research files** — `{TASK_DIR}/research/*.md` that the sub-agent will need to consult

**What NOT to put in**:
- Code files (`src/**`, `packages/**/*.ts`, etc.) — those are read by the sub-agent during implementation, not pre-registered here
- Files you're about to modify — same reason

**Split between the two files**:
- `implement.jsonl` → specs + research the implement sub-agent needs to write code correctly
- `check.jsonl` → specs for the check sub-agent (quality guidelines, check conventions, same research if needed)

**How to discover relevant specs**:

```bash
python3 ./.trellis/scripts/get_context.py --mode packages
```

Lists every package + its spec layers with paths. Pick the entries that match this task's domain.

**How to append entries**:

Either edit the jsonl file directly in your editor, or use:

```bash
python3 ./.trellis/scripts/task.py add-context "$TASK_DIR" implement "<path>" "<reason>"
python3 ./.trellis/scripts/task.py add-context "$TASK_DIR" check "<path>" "<reason>"
```

Delete the seed `_example` line once real entries exist (optional — it's skipped automatically by consumers).

Skip when: `implement.jsonl` has agent-curated entries (the seed row alone doesn't count).

[/Claude Code]

#### 1.4 Activate task `[required · once]`

Once prd.md is complete and 1.3 jsonl curation is done, flip the task status to `in_progress`:

```bash
python3 ./.trellis/scripts/task.py start <task-dir>
```

After this command succeeds, the breadcrumb auto-switches to `[workflow-state:in_progress]`, and the rest of Phase 2 / 3 follows.

If `task.py start` errors with a session-identity message (no context key from hook input, `TRELLIS_CONTEXT_ID`, or platform-native session env), follow the hint in the error to set up session identity, then retry.

#### 1.5 Completion criteria

| Condition | Required |
|------|:---:|
| `prd.md` exists | ✅ |
| User confirms requirements | ✅ |
| `task.py start` has been run (status = in_progress) | ✅ |
| `research/` has artifacts (complex tasks) | recommended |
| `info.md` technical design (complex tasks) | optional |

[Claude Code]

| `implement.jsonl` has agent-curated entries (not just the seed row) | ✅ |

[/Claude Code]

---

## Phase 2: Execute

Goal: turn the prd into code that passes quality checks.

#### 2.1 Implement `[required · repeatable]`

[Claude Code]

Spawn the implement sub-agent:

- **Agent type**: `trellis-implement`
- **Task description**: Implement the requirements per prd.md, consulting materials under `{TASK_DIR}/research/`; finish by running project lint and type-check
- **Dispatch prompt guard**: Tell the spawned agent it is already the `trellis-implement` sub-agent and must implement directly, not spawn another `trellis-implement` / `trellis-check`.

The platform hook/plugin auto-handles:
- Reads `implement.jsonl` and injects the referenced spec files into the agent prompt
- Injects prd.md content

[/Claude Code]

#### 2.2 Quality check `[required · repeatable]`

[Claude Code]

Spawn the check sub-agent:

- **Agent type**: `trellis-check`
- **Task description**: Review all code changes against spec and prd; fix any findings directly; ensure lint and type-check pass
- **Dispatch prompt guard**: Tell the spawned agent it is already the `trellis-check` sub-agent and must review/fix directly, not spawn another `trellis-check` / `trellis-implement`.

The check agent's job:
- Review code changes against specs
- Auto-fix issues it finds
- Run lint and typecheck to verify

[/Claude Code]

#### 2.3 Rollback `[on demand]`

- `check` reveals a prd defect → return to Phase 1, fix `prd.md`, then redo 2.1
- Implementation went wrong → revert code, redo 2.1
- Need more research → research (same as Phase 1.2), write findings into `research/`

---

## Phase 3: Finish

Goal: ensure code quality, capture lessons, record the work.

#### 3.1 Quality verification `[required · repeatable]`

Load the `trellis-check` skill and do a final verification:
- Spec compliance
- lint / type-check / tests
- Cross-layer consistency (when changes span layers)

If issues are found → fix → re-check, until green.

#### 3.2 Debug retrospective `[on demand]`

If this task involved repeated debugging (the same issue was fixed multiple times), load the `trellis-break-loop` skill to:
- Classify the root cause
- Explain why earlier fixes failed
- Propose prevention

The goal is to capture debugging lessons so the same class of issue doesn't recur.

#### 3.3 Spec update `[required · once]`

Load the `trellis-update-spec` skill and review whether this task produced new knowledge worth recording:
- Newly discovered patterns or conventions
- Pitfalls you hit
- New technical decisions

Update the docs under `.trellis/spec/` accordingly. Even if the conclusion is "nothing to update", walk through the judgment.

#### 3.4 Commit changes `[required · once]`

The AI drives a batched commit of this task's code changes so `/finish-work` can run cleanly afterwards. Goal: produce work commits FIRST, then bookkeeping (archive + journal) commits land after — never interleaved.

**Step-by-step**:

1. **Inspect dirty state**:
   ```bash
   git status --porcelain
   ```
   Snapshot every dirty path. If the working tree is clean, skip to 3.5.

2. **Learn commit style** from recent history (so drafted messages blend in):
   ```bash
   git log --oneline -5
   ```
   Note the prefix convention (`feat:` / `fix:` / `chore:` / `docs:` ...), language (中文/English), and length style.

3. **Classify dirty files into two groups**:
   - **AI-edited this session** — files you wrote/edited via Edit/Write/Bash tool calls in this session. You know what changed and why.
   - **Unrecognized** — dirty files you did NOT touch this session (could be the user's manual edits, leftover WIP from a previous session, or unrelated work). Do NOT silently include these.

4. **Draft a commit plan**. Group AI-edited files into logical commits (1 commit per coherent change unit, not 1 commit per file). Each entry: `<commit message>` + file list. List unrecognized files separately at the bottom.

5. **Present the plan once, ask for one-shot confirmation**. Format:
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

6. **On confirmation**: run `git add <files>` + `git commit -m "<msg>"` for each batch in order. Do not amend. Do not push.

7. **On rejection** (user replies "不行" / "我自己来" / "manual" / any pushback on the plan): stop. Do not attempt a second plan. The user will commit by hand; you skip ahead to 3.5 once they confirm.

**Rules**:
- No `git commit --amend` anywhere — three-stage three-commit flow (work commits → archive commit → journal commit).
- Never push to remote in this step.
- If the user wants different message wording but accepts the file grouping, edit the message and re-confirm once — but if they reject the grouping, exit to manual mode.
- The batched plan is one prompt; do not prompt per commit.

#### 3.5 Wrap-up reminder

<!-- trellisx:start:finish_force -->
⛔ **强制自动收尾 (不是提醒)**: check 通过后, AI **必须**运行
`python3 .trellis/scripts/task.py finish` (或 `/trellis:finish-work`)。
`after_finish` hook **自动**执行: 提交 worktree → 合并 --no-ff 回主分支 → archive → 销毁 worktree。
**worktree 删除与合并是必须的, 非可选, 非「提醒用户去做」。**
- 合并冲突 → 收尾脚本 abort + 报冲突 + 非 0 退出 (hook 失败仅 WARN, `task.py finish` 仍返 0)。
  AI **MUST 检 finish 输出有无 `trellisx-finish` 冲突告警**, 有则转手动解决, 禁强解 / 禁当成功。
- check 未过禁跑 finish; 未 archive (worktree 仍在) = 流程未闭环, 禁宣告 Done。
- commit 为 owner 授权的强制动作 (脚本直接提交); 需会话 journal 时用 `/trellis:finish-work` 入口。
- 手动兜底: 链路异常时可直接跑 `python3 .trellis/scripts/trellisx-finish.py [--task <tid>]` (幂等可重入)。
<!-- trellisx:end:finish_force -->

---

## Customizing Trellis (for forks)

This section is for developers who want to modify the Trellis workflow itself. All customization is done by editing this file; the scripts are parsers only.

### Changing what a step means

Edit the corresponding step's walkthrough body in the Phase 1 / 2 / 3 sections above. **Critical constraint**: if you change a step's `[required · once]` marker or add a new `[required · once]` step, you MUST also add a matching enforcement line to that phase's `[workflow-state:STATUS]` tag block — otherwise the per-turn breadcrumb omits the reinforcement, and the AI silently skips the step. The regression tests assert this.

All 4 tag blocks live in the `## Phase Index` section above, immediately after each phase summary:

| Scope | Corresponding tag |
|---|---|
| No active task (before Phase 1) | `[workflow-state:no_task]` (after the Phase Index ASCII art) |
| All of Phase 1 (task created → ready for implementation) | `[workflow-state:planning]` (after Phase 1 summary) |
| Phase 2 + Phase 3.1–3.4 (implementation + check + wrap-up) | `[workflow-state:in_progress]` (after Phase 2 summary) |
| After Phase 3.5 (archived) | `[workflow-state:completed]` (after Phase 3 summary; **currently DEAD**) |

### Changing the per-turn prompt text

Directly edit the body of the corresponding `[workflow-state:STATUS]` block. After editing, run `trellis update` (if you're a template maintainer) or restart your AI session (if you're customizing your own project) — no script changes required.

### Adding a custom status

Add a new block:

```
[workflow-state:my-status]
your per-turn prompt text
[/workflow-state:my-status]
```

Constraints:
- STATUS charset: `[A-Za-z0-9_-]+` (underscores and hyphens allowed, e.g. `in-review`, `blocked-by-team`)
- A lifecycle hook must write `task.json.status` to your custom value, otherwise the tag is never read
- Lifecycle hooks live in `task.json.hooks.after_*` and bind to one of `after_create / after_start / after_finish / after_archive`

### Adding a lifecycle hook

Add a `hooks` field to your `task.json`:

```json
{
  "hooks": {
    "after_finish": [
      "your-script-or-command-here"
    ]
  }
}
```

Supported events: `after_create / after_start / after_finish / after_archive`. Note that `after_finish` ≠ a status change (it only clears the active-task pointer); use `after_archive` for "task is done" notifications.

### Full contract

For the workflow state machine's runtime contract, the locations of all status writers, pseudo-statuses (`no_task` / `stale_<source_type>`), the hook reachability matrix, and other deep details, see:

- `.trellis/spec/cli/backend/workflow-state-contract.md` — runtime contract + writer table + test invariants
- `.trellis/scripts/inject-workflow-state.py` — actual parser (reads workflow.md only, no embedded text)
