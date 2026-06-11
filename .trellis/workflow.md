# 开发工作流

---

## 核心原则

1. **先规划后编码** — 动手前先想清楚做什么
2. **规范注入，不靠记忆** — 规范通过 hook/skill 注入，不从记忆中回忆
3. **一切持久化** — 研究、决策、经验都写文件；对话会被压缩，文件不会
4. **增量开发** — 一次一个任务
5. **沉淀学到的** — 每个任务完成后，回顾并将新知识写回 spec

---

## Trellis 系统

### 开发者身份

首次使用时，初始化身份：

```bash
python3 ./.trellis/scripts/init_developer.py <your-name>
```

创建 `.trellis/.developer`（gitignored）+ `.trellis/workspace/<your-name>/`。

### 规范系统

`.trellis/spec/` 存放按包和层组织的编码规范。

- `.trellis/spec/<package>/<layer>/index.md` — 入口，包含**开发前检查清单** + **质量检查**。实际规范在它指向的 `.md` 文件中。
- `.trellis/spec/guides/index.md` — 跨包思维指南。

```bash
python3 ./.trellis/scripts/get_context.py --mode packages   # 列出包 / 层
```

**何时更新 spec**：发现新模式/约定 · 需要固化的 bug 修复经验 · 新技术决策。

### 任务系统

每个任务有自己的目录 `.trellis/tasks/{MM-DD-name}/`，内含 `prd.md`、`implement.jsonl`、`check.jsonl`、`task.json`，可选 `research/`、`info.md`。

```bash
# 任务生命周期
python3 ./.trellis/scripts/task.py create "<title>" [--slug <name>] [--parent <dir>]
python3 ./.trellis/scripts/task.py start <name>          # 设置活跃任务 (session 可用时为会话级)
python3 ./.trellis/scripts/task.py current --source      # 显示活跃任务及来源
python3 ./.trellis/scripts/task.py finish                # 清除活跃任务 (触发 after_finish hooks)
python3 ./.trellis/scripts/task.py archive <name>        # 移至 archive/{year-month}/
python3 ./.trellis/scripts/task.py list [--mine] [--status <s>]
python3 ./.trellis/scripts/task.py list-archive

# 代码规范上下文 (通过 JSONL 注入到 implement/check agent)
# `implement.jsonl` / `check.jsonl` 在 `task create` 时播种，供支持 sub-agent 的平台使用；
# AI 在 Phase 1.3 阶段策划实际的 spec + research 条目。
python3 ./.trellis/scripts/task.py add-context <name> <action> <file> <reason>
python3 ./.trellis/scripts/task.py list-context <name> [action]
python3 ./.trellis/scripts/task.py validate <name>

# 任务元数据
python3 ./.trellis/scripts/task.py set-branch <name> <branch>
python3 ./.trellis/scripts/task.py set-base-branch <name> <branch>    # PR 目标分支
python3 ./.trellis/scripts/task.py set-scope <name> <scope>

# 层级 (parent/child)
python3 ./.trellis/scripts/task.py add-subtask <parent> <child>
python3 ./.trellis/scripts/task.py remove-subtask <parent> <child>

# PR 创建
python3 ./.trellis/scripts/task.py create-pr [name] [--dry-run]
```

> 运行 `python3 ./.trellis/scripts/task.py --help` 查看权威的完整命令列表。

**当前任务机制**：`task.py create` 创建任务目录，并在会话身份可用时自动设置每会话的活跃任务指针，使规划面包屑立即生效。`task.py start` 写入同一指针（幂等）并将 `task.json.status` 从 `planning` 切换到 `in_progress`。状态存储在 `.trellis/.runtime/sessions/` 下。如果 hook 输入、`TRELLIS_CONTEXT_ID` 或平台原生会话环境变量中没有上下文 key，则没有活跃任务，`task.py start` 会失败并提示会话身份设置方法。`task.py finish` 删除当前会话文件（状态不变）。`task.py archive <task>` 写入 `status=completed`，将目录移至 `archive/`，并删除仍指向已归档任务的运行时会话文件。

### 工作区系统

在 `.trellis/workspace/<developer>/` 下记录每次 AI 会话，用于跨会话追踪。

- `journal-N.md` — 会话日志。**每个文件最多 2000 行**；超出时自动创建 `journal-(N+1).md`。
- `index.md` — 个人索引（总会话数、最近活跃）。

```bash
python3 ./.trellis/scripts/add_session.py --title "Title" --commit "hash" --summary "Summary"
```

### 上下文脚本

```bash
python3 ./.trellis/scripts/get_context.py                            # 完整会话运行时
python3 ./.trellis/scripts/get_context.py --mode packages            # 可用包 + spec 层
python3 ./.trellis/scripts/get_context.py --mode phase --step <X.Y>  # 某个工作流步骤的详细指南
```

---

## Phase 索引

```
Phase 1: 规划   → 搞清楚做什么 (头脑风暴 + 研究 → prd.md)
Phase 2: 执行   → 写代码并通过质量检查
Phase 3: 收尾   → 沉淀经验 + 收尾
```

[workflow-state:no_task]
无活跃任务。**A 直接回答** — 纯问答 / 解释 / 查询 / 闲聊；无文件写入 + 一行回答 + 仓库文件读取 ≤ 2 → AI 自行判断，无需覆盖。
**B 创建任务** — 任何实现 / 代码变更 / 构建 / 重构工作。入口序列：(1) `python3 ./.trellis/scripts/task.py create "<title>"` 创建任务 (status=planning, 面包屑切换到 [workflow-state:planning] 进入头脑风暴 + jsonl 阶段指引) → (2) 加载 `trellis-brainstorm` skill 与用户讨论需求并迭代 prd.md → (3) prd 完成且 jsonl 已策划后，运行 `task.py start <task-dir>` 切换到 [workflow-state:in_progress] 进入实施骨架。**"看起来很小" 不是把 B 降级为 A 或 C 的理由**。
**C 内联变更** (仅当前轮，B 的逃生舱) — 用户**当前消息**必须包含以下之一："skip trellis" / "no task" / "just do it" / "don't create a task" / "跳过 trellis" / "别走流程" / "小修一下" / "直接改" / "先别建任务" → 简短确认 ("ok, 跳过 trellis 流程")，然后内联处理。**没有看到这些短语时，不得自行内联**；不要编造用户从未说过的覆盖指令。
[/workflow-state:no_task]

### Phase 1: 规划
- 1.0 创建任务 `[required · once]` (仅 `task.py create`；status 进入 planning)
- 1.1 需求探索 `[required · repeatable]`
- 1.2 研究 `[optional · repeatable]`
- 1.3 配置上下文 `[required · once]` — Claude Code
- 1.4 激活任务 `[required · once]` (运行 `task.py start`；status → in_progress)
- 1.5 完成标准

[workflow-state:planning]
加载 `trellis-brainstorm` skill 与用户迭代 prd.md。
Phase 1.3 (required, once): 在 `task.py start` 之前，必须策划 `implement.jsonl` 和 `check.jsonl` — 列出 sub-agent 需要的 spec / research 文件，确保注入正确上下文。只有当 jsonl 已有 agent 策划的条目时才可跳过（仅有一个 `_example` 种子行不算）。
然后运行 `task.py start <task-dir>` 将 status 切换到 in_progress。
<!-- trellisx:start:planning -->
⛔ trellisx 规划硬规: 任务 MUST 拆 ≥ 2 subtask, 每 subtask 独立文件 .trellis/tasks/<task>/subtask/<id>-<slug>.md。PRD MUST 含 mermaid 调度图, **显式标并行组** (无依赖 subtask 归同批同时跑) + 依赖箭头。拆分目标 = 最大化可并行 subtask 数, 缩短关键路径。每 subtask 必须可独立派给一个 agent 执行。详见 trellisx-orchestrate skill。
<!-- trellisx:end:planning -->
[/workflow-state:planning]

[workflow-state:planning-inline]
加载 `trellis-brainstorm` skill 与用户迭代 prd.md。
Phase 1.3 jsonl 策划在 inline dispatch 模式下**跳过** — 主会话在 Phase 2 直接加载 `trellis-before-dev` 并自行读取 spec 上下文，无需向 sub-agent 注入 jsonl。
然后运行 `task.py start <task-dir>` 将 status 切换到 in_progress。
[/workflow-state:planning-inline]

### Phase 2: 执行
- 2.1 实施 `[required · repeatable]`
- 2.2 质量检查 `[required · repeatable]`
- 2.3 回滚 `[on demand]`

[workflow-state:in_progress]
**工具**: `trellis-implement` / `trellis-research` 仅是 sub-agent 类型（Task/Agent 工具，非 Skill — 没有同名 skill）。`trellis-update-spec` 是 skill。`trellis-check` 两者都有；代码变更后验证时优先用 Agent 形式。
**流程**: trellis-implement → trellis-check → trellis-update-spec → 提交 (Phase 3.4) → `/trellis:finish-work`。
**主会话默认 (无覆盖)**: 派发 `trellis-implement` / `trellis-check` sub-agent — 主 agent 默认不直接编辑代码。Phase 3.4 提交 (required, once): 在 trellis-update-spec 之后，或实施已确认完成时，主 agent **驱动提交** — 在面向用户的文本中陈述提交计划，然后执行 `git commit` — 在建议 `/trellis:finish-work` 之前。`/finish-work` 拒绝在脏工作树上运行（`.trellis/workspace/` 和 `.trellis/tasks/` 以外的路径）。
**Sub-agent 自免除**: 如果你已经在作为 `trellis-implement` 运行，直接从加载的任务上下文实施，不要再生成另一个 `trellis-implement`；如果你已经在作为 `trellis-check` 运行，直接审查/修复，不要再生成另一个 `trellis-check`。默认派发规则仅适用于主会话。
**Sub-agent 派发协议 (所有 sub-agent)**: 当你派发 `trellis-implement` / `trellis-check` / `trellis-research` 时，派发 prompt **必须**以一行开头：`Active task: <task path from \`task.py current\`>`。无例外。这一行在 hook 注入失败时充当关键回退（Windows + Claude Code PreToolUse 静默跳过、`--continue` 恢复、fork 分发、hooks 禁用等）。对于 `trellis-research`，这一行告诉 sub-agent 写入哪个 `{task_dir}/research/`。
**Inline 覆盖** (仅当前轮，sub-agent 派发的逃生舱): 用户的**当前消息**必须明确包含以下之一："do it inline" / "no sub-agent" / "你直接改" / "别派 sub-agent" / "main session 写就行" / "不用 sub-agent"。**没有看到这些短语时，不得自行内联**；不要编造用户从未说过的覆盖指令。
<!-- trellisx:start:in_progress -->
⛔ trellisx 执行硬规 (本 task 必守, 违反即流程错误):

1. **强制 worktree**: 本 task 全部源码改动 MUST 落在 worktree (git 根/子仓 .worktrees/<worktree>, trellis 生命周期 hook 已自动建)。**禁在主工作区写源码** — 写盘 file_path 必须是 worktree 路径。
2. **强制派 agent**: 每个 subtask 的实施 MUST 派 sub-agent (isolation:worktree) 或 agent-team 成员执行。**main 禁直接写源码** — main 只拆分 / 派发 / 收集 / 合并 / 协调。
3. **强制异步并行**: 无依赖的 subtask MUST 在同一条回复里一次性发起多个 sub-agent 调用 (Claude Code 同消息多 Agent = 真并行)。**禁逐个串行派** (串行 = 耗时叠加)。有依赖的按调度图顺序。
4. **强制按调度图**: 严格按 PRD 调度图的依赖 + 并行组执行, 禁跳步。
5. 收每个 agent 返回立即回传用户进度; task archive 时 worktree 干净则自动销毁。
<!-- trellisx:end:in_progress -->
[/workflow-state:in_progress]

[workflow-state:in_progress-inline]
**流程** (inline 模式): 主会话加载 `trellis-before-dev` → 主会话编辑代码 → 主会话加载 `trellis-check` → 运行 lint / 类型检查 / 测试 → 修复 → `trellis-update-spec` → 提交 (Phase 3.4) → `/trellis:finish-work`。
**主会话默认 (inline dispatch_mode)**: 主 agent 直接编辑代码。不要派发 `trellis-implement` / `trellis-check` sub-agent。写代码前加载 `trellis-before-dev` skill；报告完成前加载 `trellis-check` skill。
Phase 3.4 提交 (required, once): 在 `trellis-update-spec` 之后，或实施已确认完成时，主 agent **驱动提交** — 在面向用户的文本中陈述提交计划，然后执行 `git commit` — 在建议 `/trellis:finish-work` 之前。`/finish-work` 拒绝在脏工作树上运行（`.trellis/workspace/` 和 `.trellis/tasks/` 以外的路径）。
<!-- trellisx:start:in_progress_inline -->
trellisx (增量): inline 模式 main 直接 edit, 但源码目标路径必须在 worktree (.worktrees/<worktree>) 内。
<!-- trellisx:end:in_progress_inline -->
[/workflow-state:in_progress-inline]

### Phase 3: 收尾
- 3.1 质量验证 `[required · repeatable]`
- 3.2 调试回顾 `[on demand]`
- 3.3 更新 spec `[required · once]`
- 3.4 提交变更 `[required · once]`
- 3.5 收尾提醒

[workflow-state:completed]
代码已通过 Phase 3.4 提交；运行 `/trellis:finish-work` 完成收尾（归档任务 + 记录会话）。
如果到达此状态时还有未提交的代码，先回到 Phase 3.4 — `/finish-work` 拒绝在脏工作树上运行。
`task.py archive` 删除仍指向已归档任务的运行时会话文件。
[/workflow-state:completed]

### 规则

1. 确定当前所在的 Phase，然后从该 Phase 的下一步继续
2. 每个 Phase 内按顺序执行步骤；`[required]` 步骤不可跳过
3. Phase 可以回滚（例如执行中发现 prd 缺陷 → 回到 Phase 1 修复，然后重新进入执行）
4. 标记 `[once]` 的步骤在输出已存在时跳过；不要重复运行

### Skill 路由

当用户请求匹配以下意图时，优先加载对应 skill（或派发对应 sub-agent） — 不要跳过 skill。

| 用户意图 | 路由 |
|---|---|
| 想要新功能 / 需求不明确 | `trellis-brainstorm` |
| 准备写代码 / 开始实施 | 按 Phase 2.1 派发 `trellis-implement` sub-agent |
| 写完了 / 想验证 | 按 Phase 2.2 派发 `trellis-check` sub-agent |
| 卡住了 / 同一个 bug 修了好几次 | `trellis-break-loop` |
| 需要更新 spec | `trellis-update-spec` |

**为什么 `trellis-before-dev` 不在此表：** 写代码的不是你 — 是 `trellis-implement` sub-agent。Sub-agent 平台通过 `implement.jsonl` 注入 / prelude 获取 spec 上下文，而非主线程加载 `trellis-before-dev`。

### 不要跳过 Skill

| 你可能在想的 | 为什么这是错的 |
|---|---|
| "这很简单，直接在主线程写" | 派发 `trellis-implement` 成本很低；跳过它会诱惑你在主线程写代码，失去 spec 上下文 — sub-agent 通过 `implement.jsonl` 获取注入，你没有 |
| "我在 plan mode 里已经想清楚了" | Plan mode 的输出存在内存中 — sub-agent 看不到；必须持久化到 prd.md |
| "我已经知道 spec 了" | Spec 可能已经更新了；sub-agent 拿到的是最新副本，你可能不是 |
| "先写代码，后检查" | `trellis-check` 能发现你自己注意不到的问题；越早发现成本越低 |

### 加载步骤详情

在每个步骤，运行以下命令获取详细指引：

```bash
python3 ./.trellis/scripts/get_context.py --mode phase --step <step>
# 例如 python3 ./.trellis/scripts/get_context.py --mode phase --step 1.1
```

---

## Phase 1: 规划

目标：搞清楚要做什么，产出清晰的需求文档和实施所需的上下文。

#### 1.0 创建任务 `[required · once]`

创建任务目录（status 进入 `planning`，会话身份可用时自动设置每会话的活跃任务指针）：

```bash
python3 ./.trellis/scripts/task.py create "<task title>" --slug <name>
```

`--slug` 仅是人类可读名称。**不要**包含 `MM-DD-` 日期前缀；`task.py create` 会自动添加。

命令成功后，每轮面包屑自动切换到 `[workflow-state:planning]`，告诉 AI 进入头脑风暴 + jsonl 策划阶段。

⚠️ **此处只运行 `create` — 不要同时运行 `start`**。`start` 会将 status 切换到 `in_progress`，使面包屑在头脑风暴 + jsonl 完成之前就跳到实施阶段 — AI 会静默跳过它们。将 `start` 留到步骤 1.4，在 jsonl 策划完成后。

当 `python3 ./.trellis/scripts/task.py current --source` 已指向某个任务时跳过。

#### 1.1 需求探索 `[required · repeatable]`

加载 `trellis-brainstorm` skill，按照 skill 的指引与用户交互式探索需求。

头脑风暴 skill 会引导你：
- 每次只问一个问题
- 优先研究而非询问用户
- 优先提供选项而非开放式提问
- 每次用户回答后立即更新 `prd.md`

需求变更时随时回到此步骤，修改 `prd.md`。

#### 1.2 研究 `[optional · repeatable]`

研究可以在需求探索期间的任何时候进行。不限于本地代码 — 你可以使用任何可用工具（MCP 服务器、skill、网络搜索等）查找外部信息，包括第三方库文档、行业实践、API 参考等。

派发研究 sub-agent：

- **Agent 类型**: `trellis-research`
- **任务描述**: 研究 <具体问题>
- **关键要求**: 研究产出必须持久化到 `{TASK_DIR}/research/`

**研究产出约定**：
- 每个研究主题一个文件（例如 `research/auth-library-comparison.md`）
- 在文件中记录第三方库使用示例、API 参考、版本约束
- 标记发现的 spec 文件路径，供后续引用

头脑风暴和研究可以自由交替 — 暂停去研究一个技术问题，然后回来与用户讨论。

**核心原则**: 研究产出必须写入文件，不要只留在对话中。对话会被压缩；文件不会。

#### 1.3 配置上下文 `[required · once]`

策划 `implement.jsonl` 和 `check.jsonl`，让 Phase 2 sub-agent 获取正确的 spec 上下文。这些文件在 `task create` 时播种了一条自描述的 `_example` 行；你在这里的工作是填入真实条目。

**位置**: `{TASK_DIR}/implement.jsonl` 和 `{TASK_DIR}/check.jsonl`（已存在）。

**格式**: 每行一个 JSON 对象 — `{"file": "<path>", "reason": "<why>"}`。路径相对于仓库根。

**应该放什么**:
- **Spec 文件** — `.trellis/spec/<package>/<layer>/index.md` 和与此任务相关的具体规范文件（`error-handling.md`、`conventions.md` 等）
- **研究文件** — sub-agent 需要参考的 `{TASK_DIR}/research/*.md`

**不应该放什么**:
- 代码文件（`src/**`、`packages/**/*.ts` 等）— 这些由 sub-agent 在实施期间读取，不需要在此预注册
- 你即将修改的文件 — 同理

**两个文件的分工**:
- `implement.jsonl` → 实施 sub-agent 需要的 spec + 研究（正确写代码）
- `check.jsonl` → 检查 sub-agent 需要的 spec（质量规范、检查约定、如需也含研究）

**如何发现相关 spec**:

```bash
python3 ./.trellis/scripts/get_context.py --mode packages
```

列出每个包及其 spec 层路径。选择与此任务域匹配的条目。

**如何追加条目**:

直接在编辑器中编辑 jsonl 文件，或使用：

```bash
python3 ./.trellis/scripts/task.py add-context "$TASK_DIR" implement "<path>" "<reason>"
python3 ./.trellis/scripts/task.py add-context "$TASK_DIR" check "<path>" "<reason>"
```

有真实条目后删除种子 `_example` 行（可选 — 消费端会自动跳过）。

跳过条件：`implement.jsonl` 已有 agent 策划的条目（仅种子行不算）。

#### 1.4 激活任务 `[required · once]`

prd.md 完成且 1.3 jsonl 策划完成后，将任务 status 切换到 `in_progress`：

```bash
python3 ./.trellis/scripts/task.py start <task-dir>
```

命令成功后，面包屑自动切换到 `[workflow-state:in_progress]`，Phase 2 / 3 的其余步骤随之进行。

如果 `task.py start` 报错提示会话身份（hook 输入、`TRELLIS_CONTEXT_ID` 或平台原生会话环境中没有上下文 key），按错误提示设置会话身份后重试。

#### 1.5 完成标准

| 条件 | 必需 |
|------|:---:|
| `prd.md` 存在 | ✅ |
| 用户确认需求 | ✅ |
| 已运行 `task.py start` (status = in_progress) | ✅ |
| `research/` 有产出 (复杂任务) | 推荐 |
| `info.md` 技术设计 (复杂任务) | 可选 |

| `implement.jsonl` 有 agent 策划的条目 (不只是种子行) | ✅ |

---

## Phase 2: 执行

目标：将 prd 转化为通过质量检查的代码。

#### 2.1 实施 `[required · repeatable]`

派发实施 sub-agent：

- **Agent 类型**: `trellis-implement`
- **任务描述**: 按 prd.md 实施需求，参考 `{TASK_DIR}/research/` 下的资料；完成后运行项目 lint 和类型检查
- **派发 prompt 守卫**: 告知派发的 agent 它已经是 `trellis-implement` sub-agent，必须直接实施，不要再生成另一个 `trellis-implement` / `trellis-check`

平台 hook/plugin 自动处理：
- 读取 `implement.jsonl` 并将引用的 spec 文件注入到 agent prompt
- 注入 prd.md 内容

#### 2.2 质量检查 `[required · repeatable]`

派发检查 sub-agent：

- **Agent 类型**: `trellis-check`
- **任务描述**: 对照 spec 和 prd 审查所有代码变更；直接修复发现的问题；确保 lint 和类型检查通过
- **派发 prompt 守卫**: 告知派发的 agent 它已经是 `trellis-check` sub-agent，必须直接审查/修复，不要再生成另一个 `trellis-check` / `trellis-implement`

检查 agent 的职责：
- 对照 spec 审查代码变更
- 自动修复发现的问题
- 运行 lint 和类型检查以验证

#### 2.3 回滚 `[on demand]`

- `check` 发现 prd 缺陷 → 回到 Phase 1，修复 `prd.md`，然后重做 2.1
- 实施出了问题 → 回滚代码，重做 2.1
- 需要更多研究 → 研究（同 Phase 1.2），将发现写入 `research/`

---

## Phase 3: 收尾

目标：确保代码质量，沉淀经验，记录工作。

#### 3.1 质量验证 `[required · repeatable]`

加载 `trellis-check` skill 进行最终验证：
- Spec 合规性
- lint / 类型检查 / 测试
- 跨层一致性（变更涉及多层时）

发现问题 → 修复 → 重新检查，直到全部通过。

#### 3.2 调试回顾 `[on demand]`

如果本任务涉及反复调试（同一问题修复了多次），加载 `trellis-break-loop` skill：
- 分类根因
- 解释之前的修复为什么失败
- 提出预防方案

目标是沉淀调试经验，避免同类问题再次发生。

#### 3.3 更新 spec `[required · once]`

加载 `trellis-update-spec` skill，审查本任务是否产生了值得记录的新知识：
- 新发现的模式或约定
- 踩过的坑
- 新的技术决策

相应更新 `.trellis/spec/` 下的文档。即使结论是"没什么要更新的"，也要走完这个判断过程。

#### 3.4 提交变更 `[required · once]`

AI 驱动本任务代码变更的批量提交，以便 `/finish-work` 之后能干净运行。目标：先产生工作提交，然后归档 + 日志提交才落地 — 不交错。

**步骤**:

1. **检查脏状态**:
   ```bash
   git status --porcelain
   ```
   快照所有脏路径。如果工作树干净，跳到 3.5。

2. **从近期历史学习提交风格** (使起草的消息融入风格):
   ```bash
   git log --oneline -5
   ```
   注意前缀约定（`feat:` / `fix:` / `chore:` / `docs:` ...）、语言（中文/英文）和长度风格。

3. **将脏文件分为两组**:
   - **本会话 AI 编辑的** — 你在本会话中通过 Edit/Write/Bash 工具调用写入/编辑的文件。你知道改了什么和为什么。
   - **未识别的** — 你在本会话中没有触碰过的脏文件（可能是用户手动编辑、上次会话的残留 WIP、或无关工作）。不要静默包含这些。

4. **起草提交计划**。将 AI 编辑的文件按逻辑分组为提交（每个连贯变更单元一个提交，不是一个文件一个提交）。每条：`<提交消息>` + 文件列表。将未识别文件单独列在底部。

5. **一次性展示计划，请求一次性确认**。格式:
   ```
   拟提交 (按顺序):
     1. <消息>
        - <文件>
        - <文件>
     2. <消息>
        - <文件>

   未识别的脏文件 (不在任何提交中 — 确认包含/排除):
     - <文件>
     - <文件>

   回复 'ok' / '行' 执行。回复修改意见，或 '我自己来' / 'manual' 取消。
   ```

6. **确认后**: 按顺序对每个批次运行 `git add <files>` + `git commit -m "<msg>"`。不要 amend。不要 push。

7. **拒绝时** (用户回复 "不行" / "我自己来" / "manual" / 任何对计划的异议): 停止。不要尝试第二个计划。用户会手动提交；确认后跳到 3.5。

**规则**:
- 全程不使用 `git commit --amend` — 三阶段三提交流（工作提交 → 归档提交 → 日志提交）。
- 此步骤绝不 push 到远程。
- 如果用户想要不同的消息措辞但接受文件分组，修改消息后再次确认 — 但如果拒绝分组，退出到手动模式。
- 批量计划是一个 prompt；不要每个提交都单独问。

#### 3.5 收尾提醒

完成以上步骤后，提醒用户可以运行 `/finish-work` 完成收尾（归档任务，记录会话）。

---

## 自定义 Trellis (适用于 fork)

本节面向想修改 Trellis 工作流本身的开发者。所有自定义通过编辑此文件完成；脚本只是解析器。

### 修改步骤的含义

编辑上方 Phase 1 / 2 / 3 部分中对应步骤的演练正文。**关键约束**：如果你更改了某个步骤的 `[required · once]` 标记或新增了 `[required · once]` 步骤，必须在对应 Phase 的 `[workflow-state:STATUS]` 标签块中添加匹配的强制行 — 否则每轮面包屑会遗漏该强化，AI 会静默跳过该步骤。回归测试会断言这一点。

全部 4 个标签块位于上方 `## Phase 索引` 部分中，紧跟每个 Phase 摘要之后：

| 范围 | 对应标签 |
|---|---|
| 无活跃任务 (Phase 1 之前) | `[workflow-state:no_task]` (Phase 索引 ASCII art 之后) |
| 整个 Phase 1 (任务创建 → 准备实施) | `[workflow-state:planning]` (Phase 1 摘要之后) |
| Phase 2 + Phase 3.1–3.4 (实施 + 检查 + 收尾) | `[workflow-state:in_progress]` (Phase 2 摘要之后) |
| Phase 3.5 之后 (已归档) | `[workflow-state:completed]` (Phase 3 摘要之后；**当前无效**) |

### 修改每轮 prompt 文本

直接编辑对应 `[workflow-state:STATUS]` 块的正文。编辑后运行 `trellis update`（如果你是模板维护者）或重启 AI 会话（如果你在自定义自己的项目） — 无需修改脚本。

### 添加自定义 status

添加新块：

```
[workflow-state:my-status]
你的每轮 prompt 文本
[/workflow-state:my-status]
```

约束：
- STATUS 字符集：`[A-Za-z0-9_-]+`（允许下划线和连字符，如 `in-review`、`blocked-by-team`）
- 必须有生命周期 hook 将 `task.json.status` 写入你的自定义值，否则标签永远不会被读取
- 生命周期 hook 在 `task.json.hooks.after_*` 中，绑定到 `after_create / after_start / after_finish / after_archive` 之一

### 添加生命周期 hook

在 `task.json` 中添加 `hooks` 字段：

```json
{
  "hooks": {
    "after_finish": [
      "your-script-or-command-here"
    ]
  }
}
```

支持的事件：`after_create / after_start / after_finish / after_archive`。注意 `after_finish` ≠ status 变更（它只清除活跃任务指针）；用 `after_archive` 处理"任务完成"通知。

### 完整契约

工作流状态机的运行时契约、所有 status 写入者的位置、伪状态（`no_task` / `stale_<source_type>`）、hook 可达性矩阵及其他深层细节，参见：

- `.trellis/spec/cli/backend/workflow-state-contract.md` — 运行时契约 + 写入者表 + 测试不变量
- `.trellis/scripts/inject-workflow-state.py` — 实际解析器（仅读取 workflow.md，无内嵌文本）
