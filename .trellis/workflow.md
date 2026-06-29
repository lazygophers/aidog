# 开发工作流

---

## 核心原则

1. **先规划再写码** —— 动手前先想清楚要做什么
2. **规约靠注入, 不靠记忆** —— 指南通过 hook/skill 注入, 不依赖记忆回想
3. **一切持久化** —— 调研、决策、教训全部落文件; 对话会被压缩, 文件不会
4. **增量开发** —— 一次只做一个任务
5. **沉淀学习** —— 每个任务后回顾, 把新知识写回 spec

---

## Trellis 系统

### 开发者身份

首次使用时初始化身份:

```bash
python3 ./.trellis/scripts/init_developer.py <your-name>
```

创建 `.trellis/.developer` (gitignored) + `.trellis/workspace/<your-name>/`。

### Spec 系统

`.trellis/spec/` 存放按包和层组织的编码指南。

- `.trellis/spec/<package>/<layer>/index.md` —— 入口, 含 **Pre-Development Checklist** + **Quality Check**。实际指南存于它指向的 `.md` 文件中。
- `.trellis/spec/guides/index.md` —— 跨包思维指南。

```bash
python3 ./.trellis/scripts/get_context.py --mode packages   # 列出包 / 层
```

**何时更新 spec**: 发现新模式/约定 · 需固化的 bug 修复预防 · 新技术决策。

### 任务系统

每个任务在 `.trellis/tasks/{MM-DD-name}/` 下有独立目录, 含 `prd.md`、`implement.jsonl`、`check.jsonl`、`task.json`, 可选 `research/`、`info.md`。

```bash
# 任务生命周期
python3 ./.trellis/scripts/task.py create "<title>" [--slug <name>] [--parent <dir>]
python3 ./.trellis/scripts/task.py start <name>          # 设为活动任务 (可用时按会话作用域)
python3 ./.trellis/scripts/task.py current --source      # 显示活动任务及来源
python3 ./.trellis/scripts/task.py finish                # 清除活动任务 (触发 after_finish hooks)
python3 ./.trellis/scripts/task.py archive <name>        # 移到 archive/{year-month}/
python3 ./.trellis/scripts/task.py list [--mine] [--status <s>]
python3 ./.trellis/scripts/task.py list-archive

# 代码 spec 上下文 (经 JSONL 注入 implement/check agent)。
# implement.jsonl / check.jsonl 在 task create 时为支持 sub-agent 的平台播种;
# AI 在 Phase 1.3 期间整理真实 spec + research 条目。
python3 ./.trellis/scripts/task.py add-context <name> <action> <file> <reason>
python3 ./.trellis/scripts/task.py list-context <name> [action]
python3 ./.trellis/scripts/task.py validate <name>

# 任务元数据
python3 ./.trellis/scripts/task.py set-branch <name> <branch>
python3 ./.trellis/scripts/task.py set-base-branch <name> <branch>    # PR 目标
python3 ./.trellis/scripts/task.py set-scope <name> <scope>

# 层级 (parent/child)
python3 ./.trellis/scripts/task.py add-subtask <parent> <child>
python3 ./.trellis/scripts/task.py remove-subtask <parent> <child>

# 创建 PR
python3 ./.trellis/scripts/task.py create-pr [name] [--dry-run]
```

> 运行 `python3 ./.trellis/scripts/task.py --help` 查看权威、最新的命令列表。

**当前任务机制**: `task.py create` 创建任务目录, 并 (当会话身份可用时) 自动设置每会话活动任务指针, 使规划面包屑立即触发。`task.py start` 写入同一指针 (已设置则幂等), 并把 `task.json.status` 从 `planning` 翻为 `in_progress`。状态存于 `.trellis/.runtime/sessions/`。若无法从 hook 输入、`TRELLIS_CONTEXT_ID` 或平台原生会话环境变量获得上下文键, 则无活动任务, `task.py start` 以会话身份提示失败。`task.py finish` 删除当前会话文件 (状态不变)。`task.py archive <task>` 写入 `status=completed`, 把目录移到 `archive/`, 并删除仍指向被归档任务的运行时会话文件。

### 工作区系统

记录每次 AI 会话以做跨会话跟踪, 位于 `.trellis/workspace/<developer>/`。

- `journal-N.md` —— 会话日志。**每文件最多 2000 行**; 超过时自动创建新的 `journal-(N+1).md`。
- `index.md` —— 个人索引 (总会话数、最后活跃)。

```bash
python3 ./.trellis/scripts/add_session.py --title "Title" --commit "hash" --summary "Summary"
```

### 上下文脚本

```bash
python3 ./.trellis/scripts/get_context.py                            # 完整会话运行时
python3 ./.trellis/scripts/get_context.py --mode packages            # 可用包 + spec 层
python3 ./.trellis/scripts/get_context.py --mode phase --step <X.Y>  # 某工作流步骤的详细指南
```

---

## Phase 索引

```
Phase 1: Plan    → 想清楚要做什么 (brainstorm + research → prd.md)
Phase 2: Execute → 写代码并通过质量检查
Phase 3: Finish  → 提炼教训 + 收尾
```

[workflow-state:no_task]
无活动任务。**A 直接回答** —— 纯 Q&A / 解释 / 查询 / 闲聊; 无文件写入 + 一行回答 + 仓库读取 ≤ 2 文件 → AI 自判, 无需 override。
**B 创建任务** —— 任何实现 / 代码改动 / 构建 / 重构工作。入口序列: (1) `python3 ./.trellis/scripts/task.py create "<title>"` 创建任务 (status=planning, 面包屑切到 [workflow-state:planning] 做 brainstorm + jsonl 阶段指引) → (2) 加载 `trellis-brainstorm` skill 与用户讨论需求并迭代 prd.md → (3) prd 完成且 jsonl 整理好后, 运行 `task.py start <task-dir>` 进入 [workflow-state:in_progress] 进入实现骨架。**"看起来很小" 不构成把 B 降级为 A 或 C 的理由**。
**C 内联改动** (仅限本轮, B 的逃生舱) —— 用户的**当前**消息必须含其一: "skip trellis" / "no task" / "just do it" / "don't create a task" / "跳过 trellis" / "别走流程" / "小修一下" / "直接改" / "先别建任务" → 简短确认 ("ok, 本轮跳过 trellis 流程"), 然后内联。**没看到这些短语之一你绝不能自行内联**; 不要发明用户从未说过的 override。
<!-- trellisx:start:no_task -->
trellisx 规约 (强化原生判定, 不切断建 task 路径): 本项目愿景 = 除极简任务外, 一律走 Trellis task 执行。

**作用域边界 — 何时建 task (执行规约的触发前提)**:

| 特征 | 判定 |
| --- | --- |
| 纯查询 / 文档阅读 / 问答 (无改动) | 豁免, 不建 task |
| 单文件单处改, ≤20 行且位置已知 | 豁免 |
| 跨 ≥2 文件 / 单文件多处 / 多步骤改 | **必建 task** |
| 需外部调研 (库选型/方案对比) 或产出文档交付 | **必建 task** (调研为 research subtask) |
| 边界模糊 | **MUST AskUserQuestion 由用户裁定** |

**无 task → 不进执行编排 → 执行规约豁免**。Planning 阶段不派执行 subagent: brainstorm 主线由 main 同步前台 (subagent 不能 AskUserQuestion); 纯信息调研可派 trellis-research 并行, 但设计决策由 main 汇总裁定。

- 表中"必建 task"的请求 → **默认创建 task** (不是"倾向建", 是默认就建; 仅用户明确拒绝才 inline)。建议直接用 AskUserQuestion 征得同意并进入 planning, 而非自行 inline。
- "边界模糊"行 → **MUST 主动用 AskUserQuestion 问用户** "本轮是否创建 Trellis task?", 禁默认跳过 / 禁自行替用户决定。
- 判断"新建 task"还是"并入现有 task"时, 读 `.trellis/task.md` 看板对照现有任务 (id/名称/描述/状态) 辅助判断。
原生的「先分类 + 征得同意才建」不变 — 但默认倾向从"可建可不建"上调为"默认建, 除非极简或用户拒绝"。
<!-- trellisx:end:no_task -->
[/workflow-state:no_task]

### Phase 1: Plan
- 1.0 创建任务 `[required · once]` (仅 `task.py create`; 状态进入 planning)
- 1.1 需求探索 `[required · repeatable]`
- 1.2 调研 `[optional · repeatable]`
- 1.3 配置上下文 `[required · once]` —— Claude Code
- 1.4 激活任务 `[required · once]` (运行 `task.py start`; 状态 → in_progress)
- 1.5 完成标准

[workflow-state:planning]
加载 `trellis-brainstorm` skill 并与用户迭代 prd.md。
Phase 1.3 (required, once): 在 `task.py start` 之前, 你 MUST 整理 `implement.jsonl` 和 `check.jsonl` —— 列出 sub-agent 需要的 spec / research 文件, 使其得到正确的上下文注入。仅当 jsonl 已有 agent 整理过的条目时方可跳过 (单独的 seed `_example` 行不算)。
然后运行 `task.py start <task-dir>` 把状态翻为 in_progress。
<!-- trellisx:start:planning -->
trellisx 规划规约 (两层拆分概念分清):

⚙️ **规划同步前台 (交互式, 禁自行凭空设计)**: planning 含 `trellis-brainstorm` **逐问用户**的交互, subagent 不能与用户对话, 故 **planning 由 main 同步前台执行**, 不派 subagent。分工: **`trellis-brainstorm` 为主导** (main 同步逐问用户做需求探索 + 方案设计 + 边界, 产出 prd/design); **`trellisx-orchestrate` 仅管执行层编排** (实际执行的 subtask 职责划分、并行/依赖、资源互斥, 产出 implement.md), **不用它做需求/方案设计**。产物评审 (`AskUserQuestion`) 由 main 亲做。注: exec/check 等非交互实质工作走 subagent 执行 (异步与否按需自定, 不强制)。

判定两层 (正交, 各自独立判):
1. **parent/child?** 本请求是否含**多个独立可验收交付** (各自可独立 plan/implement/check/archive)?
   - **是** → 拆 parent + child tasks (`task.py create --parent`)。child **动态调度**: 独立 child (无依赖) **可并行** (各 child 各 worktree, 并发上限 2); 有依赖才串行; 完成即派下一个。parent 是 **child 级调度器** (持 child DAG); child 间依赖写进各 child 的 prd.md/implement.md (非树位置隐含); parent PRD MUST 含 child map 表 + 跨 child 验收 + 集成 review; 每个 child 独立 worktree。
   - **否** → 单 task。
2. **subtask 拆分?** 该 task (含每个 child) 的 exec 是否含多个**独立无影响**工作单元?
   - **是** → implement.md 拆 subtask, 各派专用 `trellis-implement` **并行**执行 (**main 是调度器**, 动态 DAG 调度并发上限 2, 完成即派, 见 trellisx-orchestrate `scheduling.md`; 无依赖并行, 共享 task worktree 文件集不相交即可, subtask 不绑定 worktree); **trellis-implement 不调度不递归** (工具集无 Agent/Task, Recursion Guard); main 不直接写源码。
   - **否** → 轻量 inline (trellis-implement 在 task worktree 内内联直做), 不强制拆 subtask。

**两层正交 (同构调度器)**: parent/child 管任务级**动态调度**分解 (child 是独立 task, 各 child 各 worktree, parent 持 child DAG 并发上限 2); subtask 管任务内 exec **动态调度** (subagent fan-out, 共享 task worktree, main 持 subtask DAG 并发上限 2)。两层调度器同构 (持 DAG / 动态派 / 完成即派 / 并发 2), 隔离单位不同。child 自身可再 subtask 拆分。拆分目的 = 独立交付各自隔离 + 任务内/任务间最大化并行, 缩短关键路径; 不是为凑数量。详见 trellisx-orchestrate skill。

task 创建后, 用 `trellisx-workspace` 及时更新 `.trellis/task.md` 看板表 (新增/更新该任务行)。

📄 **spec 自动加载 (planning 软约束, AI 判定 "如需")**: 进入 planning 后, AI **主动**按 task 主题 grep `.trellis/spec/` 下相关 guide (关键词从 task 标题 / prd 草稿提取), 命中则读入作为 PRD 编排上下文 (约束 / 命令式契约 / 验收基准), 指导 deliverable 矩阵与验收标准。无 `.trellis/spec/` 或无相关 guide → 跳过, 不报错 (软约束, 非硬门)。目的: planning 即对齐 spec 真值, 避免写完 PRD 才发现违反既有契约。
<!-- trellisx:end:planning -->
[/workflow-state:planning]

### Phase 2: Execute
- 2.1 实现 `[required · repeatable]`
- 2.2 质量检查 `[required · repeatable]`
- 2.3 回滚 `[on demand]`

[workflow-state:in_progress]
**工具**: `trellis-implement` / `trellis-research` 仅为 sub-agent 类型 (Task/Agent 工具, 非 Skill —— 不存在同名 skill)。`trellis-update-spec` 是 skill。`trellis-check` 两者皆有; 代码改动后验证时优先用 Agent 形式。
**流程**: trellis-implement → trellis-check → trellis-update-spec → commit (Phase 3.4) → `/trellis:finish-work`。
**Main 会话默认 (无 override)**: 派 `trellis-implement` / `trellis-check` sub-agent —— main 默认不直接改代码。Phase 3.4 commit (required, once): 在 trellis-update-spec 之后, 或实现可验证完成时, main **驱动提交** —— 在面向用户文本中陈述提交计划, 然后运行 `git commit` —— **在**建议 `/trellis:finish-work` **之前**。`/finish-work` 拒绝在脏工作树上运行 (`.trellis/workspace/` 和 `.trellis/tasks/` 之外的路径)。
**Sub-agent 自豁免**: 若你已作为 `trellis-implement` 运行, 直接从已加载的任务上下文实现, 不要再派 `trellis-implement`; 若你已作为 `trellis-check` 运行, 直接 review/fix, 不要再派 `trellis-check`。默认派发规则仅适用于 main 会话。
**Sub-agent 派发协议**: 当你派 `trellis-implement` / `trellis-check` / `trellis-research` 时, 派发 prompt **MUST** 以一行开头: `Active task: <task.py current 给出的 task 路径>`。无例外。对 Claude Code, hook 通常直接注入上下文使该行冗余, 但 hook 失效时 (`--continue` 恢复、fork 分发、hooks 被禁用等) 它是关键回退。对 `trellis-research`, 该行告知 sub-agent 写入哪个 `{task_dir}/research/`。
**内联 override** (仅限本轮, sub-agent 派发的逃生舱): 用户的**当前**消息必须显式含其一: "do it inline" / "no sub-agent" / "你直接改" / "别派 sub-agent" / "main session 写就行" / "不用 sub-agent"。**没看到这些短语之一你绝不能自行内联**; 不要发明用户从未说过的 override。
<!-- trellisx:start:in_progress -->
⛔ trellisx 执行硬规 (本 task 必守, 违反即流程错误):

0. **实质工作走 subagent 执行 (最高优先级)**: **概念分清** —— **task** = 任务记录 (`task.py create/start/finish/archive` 脚本), **main 同步跑**; **实质工作** (改源码、跑 check) **由 subagent 执行**。main **禁直接落地实质工作** —— 一律派 subagent (`Agent` 工具); main 只做编排 + task.py 脚本同步调用 + 用户交互决策 (`AskUserQuestion` subagent 不能做) + 完成即时回传 + 看板维护。是否 `run_in_background` 异步 / 并行 **按需自定, 不强制** (用户需要异步会自处理, 只需确保走 subagent)。每个 dispatch prompt 须 6 字段自包含 (目标/已知含 `Active task:`/范围/输出/验收/失败处理)。commit→merge→archive 由 `task.py finish` + `after_finish` hook 自动 (见 #4), 非派 agent。**🔴🛑 "派 agent" = 真实调用 `Agent` 工具, 不是叙述 (最易踩)**: 每个派 agent 动作 MUST 在**同一回复**产生真实 `Agent` tool_use; **严禁本回复无 `Agent` 工具调用就回传"已派出/agent 在做"** —— 宣称 ≠ 调用 = 幻觉跳步; 同理 "已建 task/看板已登记/worktree 已建" 必须是真实跑过 `task.py`/`trellisx-workspace`/hook 的结果。回传前自检本回复确有对应 tool_use, 无则先调用再回传。
1. **task 在 worktree 内执行 (task 级隔离, 目的 = 防并发多个 task 互相冲突)**: 隔离单位是 **task, 不是单次写盘**。`task.py start` 后 trellis hook 自动建本 task 的 worktree (`<git根>/.worktrees/<name>`); 本 task 的全部执行 (trellis-implement 及其 subagent) 都**在这个 worktree 内进行**, 主工作区保持干净。**默认一个 task 一个 worktree** (绝大多数场景)。**subtask 与 worktree 无绑定关系** —— subtask 在 task worktree 内并行共享, 不为 subtask 单独开 worktree; **多 worktree 允许** (opt-in, 非默认, 不靠 subtask 自动触发), 多 worktree 时收尾经 task↔worktree 映射把各分支先合并回主分支再 archive (`trellisx-finish.py` 已支持多 worktree 合并)。
   - **执行载体 = subagent 编排 (默认, 足够)**: 本 task 的 exec(+check) **默认**由 **main 调度** → 派各 `trellis-implement` 各执行 1 subtask (见 #2), fan-out 的 trellis-implement **共享 task worktree** (不传 `isolation:worktree`, subtask 与 worktree 无绑定); **main 动态 DAG 调度并发上限 2, 完成即派下一个, 不空等全部** (见 trellisx-orchestrate `scheduling.md`); 多 worktree 允许 opt-in 非自动 (finish 合并各分支)。**绝大多数 task 用 subagent 编排即可, 不需要 Workflow。**
   - **Workflow 仅特别复杂 task 才启 (opt-in, 非强制)**: 仅当任务达 workflow 门槛 —— 大规模 fan-out / 仓库级审计 / ≥5 同类文件批量改 / 500+ 文件迁移 / 多阶段重度并行编排, **且**确定性扇出编排收益明显时, 才把 exec 用 Claude Code Workflow 工具编排成独立 workflow (1 task : 1 workflow : N worktree)。**启用 workflow 须用户显式同意。** 普通 task 不启 workflow, 直接 subagent 编排。**worktree 隔离规则在两种载体下都不变 (强制)。**
   - **若启用了 Workflow (仅特别复杂 task), 生成的 Workflow 须守四规范**: ① **phases 细致标类型** —— `meta.phases` 逐项标 `title` + `detail`, 阶段类型取自 {research, design, implement, verify, finalize}, 用户一眼看出每 phase 干什么属什么类型。② **允许并行** —— 无依赖 phases / fan-out agent 用 `parallel()` 并发; 有前后序的用分层 `parallel()` 表达, 不留无谓串行。③ **失败自动重试** —— 单 step/agent 失败自动重试 ≤2 次, 不让整个 workflow 直接失败; 重试仍败才降级 (返回 null + 下游 `.filter(Boolean)` 跳过, 或转 manual Blocked)。④ **收尾无残留** —— finalize phase 强制 `task.py finish` (hook 触发 commit→merge→remove worktree→archive) + finish 前 AI 自查无悬挂 Workflow/agent (TaskList 查 / TaskStop 关)。worktree 未销 / Workflow 未关 = 未闭环, 禁宣告 Done。⑤ **异步不阻塞** —— Workflow 调用即返回 task ID, 完成自动回 `<task-notification>`; **禁 `Bash(sleep N && ...)` 或轮询循环占住 main 空等 workflow 跑完** —— 调用后结束本回合, notification 回来再进收尾。sleep 等待 = 既阻塞 main 又对不齐真实时长 = 反模式。**未启用 workflow 的普通 task 不适用四规范, 其收尾由 #4 强制收尾保证。**
   - **retry 两层, 各管一类失败**: **载体内自动重试** (瞬时错误: 网络抖动/工具超时/index.lock) —— workflow 载体下用 `agent_with_retry` 包装 (默认 2 次, 指数退避语义); subagent 载体下 main 据失败原因即时重派。**main 重派** (逻辑失败: 自验 ✗/产物不达标) —— 对齐 apply 修复循环, 同一 subtask 累计 ≤3 次, 第 4 次禁盲目重试, 回 planning 重拆。重试计数 per-agent; 全部重试耗尽 → 该 agent 返回 null/标 Blocked, 不崩整体, 收尾时上报。
2. **实施派发模型 (main 调度 → 派 trellis-implement 各执行 1 subtask, 同在 task worktree 内)**: 进入实施, **main 是调度器** —— 读各 subtask 文件的 `write-files` + `exec-scope` 算冲突 (三类依赖边) → 建 DAG → 动态派 `trellis-implement` (**并发上限 2, 完成即派下一个, 不空等全部**, 见 trellisx-orchestrate `scheduling.md`)。每个 `trellis-implement` 接 1 个 subtask, 在**本 task 的 worktree 内**执行 (**共享 task worktree**, 文件集不相交即可并行; subtask 与 worktree 无绑定); 多 worktree 允许 opt-in 非自动 (finish 经映射合并各分支)。**main 禁直接写源码** (只调度派发 + 进度回传 + 合并)。**trellis-implement 不调度不递归** —— 工具集仅 Read/Write/Edit/Bash, 无 Agent/Task (Recursion Guard), 物理上不能派 subagent / 递归。严格按 implement.md / PRD 调度图依赖 + 并行组执行。main 收拢全部 subtask 产物 → 交 `trellis-check`。循环: planning → main 派 trellis-implement → trellis-check → finish。
3. **轻量模式** (单 subtask): main **派一个 `trellis-implement`** (动态调度退化为 1 派), 由其在 task worktree 内**内联直做** (无需再派), 保持 planning → implement → check → finish 循环一致。**main 不绕过 implement 自己写源码** (仍守第 1 条: task 在 worktree 内执行)。
4. **强制自动收尾 (check 通过后跑收尾, 全链自动, 不停在「提醒」)**: `trellis-check` 通过后, AI **强制收尾**, **两路任选其一, 都做全链** (commit→merge→del worktree→archive): ① `python3 .trellis/scripts/task.py finish` → `after_finish` hook 自动跑 `trellisx-finish.py`; ② `/trellis:finish-work` → apply 已注入它**先跑 `trellisx-finish.py` 全链再 journal** (修原生 archive-direct 绕 merge 丢提交)。不把收尾降级为"提醒用户" —— **合并与 worktree 删除是必须的, 非可选**:
   ① **worktree 内提交** —— `git add -A` + `git commit` (缺省消息 `chore(task): <tid> 收尾提交`), 落到 `trellisx-<name>` 分支。
   ② **合并回主分支** —— `git merge --no-ff trellisx-<name>`; **合并冲突 → 脚本自动 `merge --abort` + 报冲突文件清单 + 非 0 退出**。hook 失败仅 WARN, `task.py finish` 仍返 0 → AI **MUST 检 finish 输出有无 `trellisx-finish` 冲突告警**, 有则停下转手动, 禁强解 / 禁当成功。**未合并就 archive = 分支被销毁丢提交** (见 `trellisx-worktree.py` 教训, 故脚本先合并)。
   ③ **archive** —— 链内 `task.py archive <tid>` 归档 + 触发 `after_archive` 销毁**已合并**的 worktree + 删分支。
   - **commit 强制 (owner 授权)**: 本项目 owner 已显式要求 trellisx 流程**强制提交收尾**, 脚本直接 commit 不逐次等确认。此为 owner 对自有注入流程的设计授权, 覆盖默认「禁主动 commit」。
   - **check 未过** → 修复重检, 禁跑 finish; **未 archive (worktree 仍在) = 流程未闭环, 禁宣告 Done / 禁结束本轮**。
   - 手动兜底: 链路异常 → 直接跑 `trellisx-finish.py [--task <tid>]` (幂等可重入)。
5. **及时维护 task.md 看板**: start / 阶段推进 (exec→check→finish) / archive 后, MUST 用 `trellisx-workspace` 更新 `.trellis/task.md` 看板行 (状态/worktree)。看板滞后于实际 = 流程缺陷。
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
- 3.5 收尾

[workflow-state:completed]
代码已经 Phase 3.4 提交; 运行 `/trellis:finish-work` 收尾 (归档任务 + 记录会话)。
若到此状态仍有未提交代码, 先回 Phase 3.4 —— `/finish-work` 拒绝在脏工作树上运行。
`task.py archive` 删除仍指向被归档任务的运行时会话文件。
[/workflow-state:completed]

### 规则

1. 先识别你在哪个 Phase, 再从该 Phase 的下一步继续
2. 每个 Phase 内按顺序执行; `[required]` 步骤不可跳过
3. Phase 可回滚 (例: Execute 揭示 prd 缺陷 → 回 Plan 修复, 再重入 Execute)
4. 标 `[once]` 的步骤若输出已存在则跳过, 别重跑

### Skill 路由

当用户请求匹配以下意图之一, 先加载对应 skill (或派对应 sub-agent) —— 不要跳过 skill。

| 用户意图 | 路由 |
|---|---|
| 想要新功能 / 需求不清 | `trellis-brainstorm` |
| 即将写码 / 开始实现 | 按 Phase 2.1 派 `trellis-implement` sub-agent |
| 写完了 / 想验证 | 按 Phase 2.2 派 `trellis-check` sub-agent |
| 卡住 / 同一 bug 反复修 | `trellis-break-loop` |
| Spec 需更新 | `trellis-update-spec` |

**为何 `trellis-before-dev` 不在此表:** 写码的不是你 —— 是 `trellis-implement` sub-agent。sub-agent 经 `implement.jsonl` 注入 / prelude 获取 spec 上下文, 不靠 main 线程加载 `trellis-before-dev`。

### 不要跳过 skill

| 你在想什么 | 为何错 |
|---|---|
| "这很简单, 我直接在 main 线程写码" | 派 `trellis-implement` 是廉价路径; 跳过它会诱使你在 main 线程写码而丢失 spec 上下文 —— sub-agent 被注入 `implement.jsonl`, 你没有 |
| "我已在 plan 模式想清楚了" | plan 模式输出存于记忆 —— sub-agent 看不到; 必须持久化到 prd.md |
| "我已经知道 spec 了" | spec 可能自你上次读后已更新; sub-agent 拿到新副本, 你可能没有 |
| "先写码, 后检查" | `trellis-check` 暴露你自己注意不到的问题; 越早越省 |

### 加载步骤详情

每一步运行此命令获取详细指引:

```bash
python3 ./.trellis/scripts/get_context.py --mode phase --step <step>
# 例: python3 ./.trellis/scripts/get_context.py --mode phase --step 1.1
```

---

## Phase 1: Plan

目标: 想清楚要构建什么, 产出清晰的需求文档和实现所需的上下文。

#### 1.0 创建任务 `[required · once]`

创建任务目录 (状态进入 `planning`, 会话身份可用时活动任务指针自动指向新任务):

```bash
python3 ./.trellis/scripts/task.py create "<task title>" --slug <name>
```

`--slug` 仅为人类可读名。**不要**含 `MM-DD-` 日期前缀; `task.py create` 自动加该前缀。

此命令成功后, 每轮面包屑自动切到 `[workflow-state:planning]`, 提示 AI 进入 brainstorm + jsonl 整理阶段。

⚠️ **这里只跑 `create` —— 不要同时跑 `start`**。`start` 把状态翻为 `in_progress`, 会在 brainstorm + jsonl 完成前把面包屑切到实现阶段 —— AI 会静默跳过它们。把 `start` 留到步骤 1.4, jsonl 整理完之后。

当 `python3 ./.trellis/scripts/task.py current --source` 已指向某任务时跳过。

#### 1.1 需求探索 `[required · repeatable]`

加载 `trellis-brainstorm` skill, 按 skill 指引与用户交互式探索需求。

brainstorm skill 会引导你:
- 一次问一个问题
- 优先调研而非问用户
- 优先提供选项而非开放式问题
- 每次用户回答后立即更新 `prd.md`

每当需求变化时回到此步并修订 `prd.md`。

#### 1.2 调研 `[optional · repeatable]`

调研可在需求探索的任意时刻进行。不限于本地代码 —— 可用任何可用工具 (MCP server、skill、web search 等) 查外部信息, 包括第三方库文档、行业实践、API 参考等。

派调研 sub-agent:

- **Agent type**: `trellis-research`
- **Task description**: Research <具体问题>
- **Key requirement**: 调研输出 MUST 持久化到 `{TASK_DIR}/research/`

**调研产物约定**:
- 每个调研主题一个文件 (例 `research/auth-library-comparison.md`)
- 第三方库用法示例、API 参考、版本约束记入文件
- 记下你发现的相关 spec 文件路径供后续参考

brainstorm 与 research 可自由交错 —— 暂停去调研一个技术问题, 再回来与用户交谈。

**关键原则**: 调研输出必须写入文件, 不能只留在对话里。对话会被压缩, 文件不会。

#### 1.3 配置上下文 `[required · once]`

整理 `implement.jsonl` 和 `check.jsonl`, 使 Phase 2 sub-agent 得到正确的 spec 上下文。这些文件在 task create 时以单条自描述 `_example` 行播种; 你这一步的工作是填入真实条目。

**位置**: `{TASK_DIR}/implement.jsonl` 和 `{TASK_DIR}/check.jsonl` (已存在)。

**格式**: 每行一个 JSON 对象 —— `{"file": "<path>", "reason": "<why>"}`。路径相对仓库根。

**放什么**:
- **Spec 文件** —— 与本任务相关的 `.trellis/spec/<package>/<layer>/index.md` 及具体指南文件 (`error-handling.md`、`conventions.md` 等)
- **Research 文件** —— sub-agent 需查阅的 `{TASK_DIR}/research/*.md`

**不放什么**:
- 代码文件 (`src/**`、`packages/**/*.ts` 等) —— 这些由 sub-agent 在实现时读取, 不在此预注册
- 你即将修改的文件 —— 同理

**两文件分工**:
- `implement.jsonl` → implement sub-agent 正确写码所需的 spec + research
- `check.jsonl` → check sub-agent 所需 spec (质量指南、检查约定, 需要时同 research)

**如何发现相关 spec**:

```bash
python3 ./.trellis/scripts/get_context.py --mode packages
```

列出每个包 + 其 spec 层及路径。挑选匹配本任务领域的条目。

**如何追加条目**:

直接在编辑器里改 jsonl 文件, 或用:

```bash
python3 ./.trellis/scripts/task.py add-context "$TASK_DIR" implement "<path>" "<reason>"
python3 ./.trellis/scripts/task.py add-context "$TASK_DIR" check "<path>" "<reason>"
```

真实条目存在后删除 seed `_example` 行 (可选 —— 消费方会自动跳过它)。

跳过条件: `implement.jsonl` 已有 agent 整理过的条目 (单独 seed 行不算)。

#### 1.4 激活任务 `[required · once]`

prd.md 完成且 1.3 jsonl 整理好后, 把任务状态翻为 `in_progress`:

```bash
python3 ./.trellis/scripts/task.py start <task-dir>
```

此命令成功后, 面包屑自动切到 `[workflow-state:in_progress]`, 余下 Phase 2 / 3 随之进行。

若 `task.py start` 报会话身份错误 (无法从 hook 输入、`TRELLIS_CONTEXT_ID` 或平台原生会话环境变量获得上下文键), 按错误中的提示设置会话身份后重试。

#### 1.5 完成标准

| 条件 | 必需 |
|------|:---:|
| `prd.md` 存在 | ✅ |
| 用户确认需求 | ✅ |
| 已运行 `task.py start` (status = in_progress) | ✅ |
| `research/` 有产物 (复杂任务) | 推荐 |
| `info.md` 技术设计 (复杂任务) | 可选 |
| `implement.jsonl` 有 agent 整理过的条目 (非仅 seed 行) | ✅ |

---

## Phase 2: Execute

目标: 把 prd 变成通过质量检查的代码。

#### 2.1 实现 `[required · repeatable]`

派 implement sub-agent:

- **Agent type**: `trellis-implement`
- **Task description**: 按 prd.md 实现需求, 参考 `{TASK_DIR}/research/` 下材料; 结束时跑项目 lint 与 type-check
- **Dispatch prompt guard**: 告知被派 agent 它已是 `trellis-implement` sub-agent, 必须直接实现, 不要再派 `trellis-implement` / `trellis-check`。

平台 hook/plugin 自动处理:
- 读 `implement.jsonl` 并把引用的 spec 文件注入 agent prompt
- 注入 prd.md 内容

#### 2.2 质量检查 `[required · repeatable]`

派 check sub-agent:

- **Agent type**: `trellis-check`
- **Task description**: 对照 spec 与 prd review 所有代码改动; 直接修复发现项; 确保 lint 与 type-check 通过
- **Dispatch prompt guard**: 告知被派 agent 它已是 `trellis-check` sub-agent, 必须直接 review/fix, 不要再派 `trellis-check` / `trellis-implement`。

check agent 的工作:
- 对照 spec review 代码改动
- 自动修复发现的问题
- 跑 lint 和 typecheck 验证

#### 2.3 回滚 `[on demand]`

- `check` 揭示 prd 缺陷 → 回 Phase 1, 修 `prd.md`, 再重做 2.1
- 实现出错 → 回退代码, 重做 2.1
- 需更多调研 → 调研 (同 Phase 1.2), 把发现写入 `research/`

---

## Phase 3: Finish

目标: 确保代码质量, 沉淀教训, 记录工作。

#### 3.1 质量验证 `[required · repeatable]`

加载 `trellis-check` skill 做最终验证:
- Spec 合规
- lint / type-check / tests
- 跨层一致性 (当改动跨层时)

发现问题 → 修复 → 重检, 直到全绿。

#### 3.2 调试复盘 `[on demand]`

若本任务涉及反复调试 (同一问题被多次修复), 加载 `trellis-break-loop` skill 以:
- 分类根因
- 解释为何早先的修复失败
- 提出预防

目的是沉淀调试教训, 使同类问题不再复发。

#### 3.3 Spec 更新 `[required · once]`

加载 `trellis-update-spec` skill, review 本任务是否产生了值得记录的新知识:
- 新发现的模式或约定
- 你踩到的坑
- 新技术决策

据此更新 `.trellis/spec/` 下文档。即便结论是"无需更新", 也走一遍判断。

#### 3.4 提交改动 `[required · once]`

AI 驱动本任务代码改动的批量提交, 以便 `/finish-work` 之后能干净运行。目标: 先产出工作提交, 再落账 (archive + journal) 提交 —— 绝不交错。

**分步**:

1. **检查脏状态**:
   ```bash
   git status --porcelain
   ```
   快照每个脏路径。工作树干净则跳到 3.5。

2. **从近期历史学提交风格** (使草拟消息融入):
   ```bash
   git log --oneline -5
   ```
   记下前缀约定 (`feat:` / `fix:` / `chore:` / `docs:` ...)、语言 (中文/English)、长度风格。

3. **把脏文件分两组**:
   - **本会话 AI 改过** —— 本会话你经 Edit/Write/Bash 工具改/写的文件。你知道改了什么、为什么。
   - **不认识** —— 本会话你没碰的脏文件 (可能是用户手改、上次会话遗留 WIP, 或无关工作)。不要静默纳入。

4. **草拟提交计划**。把 AI 改过的文件分组为逻辑提交 (每个连贯改动单元一个提交, 非每文件一个提交)。每条: `<commit message>` + 文件列表。不认识的文件单列底部。

5. **一次性呈现计划, 求一次确认**。格式:
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

6. **确认后**: 对每批按序跑 `git add <files>` + `git commit -m "<msg>"`。不 amend。不 push。

7. **被拒后** (用户回 "不行" / "我自己来" / "manual" / 任何对计划的反对): 停止。不要尝试第二个计划。用户会手动提交; 待其确认后你跳到 3.5。

**规则**:
- 任何地方都不用 `git commit --amend` —— 三段三提交流 (工作提交 → archive 提交 → journal 提交)。
- 此步绝不 push 到远程。
- 若用户想要不同消息措辞但接受文件分组, 改消息并重新确认一次 —— 但若他们拒绝分组, 退到手动模式。
- 批量计划是一个 prompt; 不要逐提交提示。

#### 3.5 收尾

<!-- trellisx:start:finish_force -->
⛔ **强制自动收尾 (不是提醒)**: check 通过后, AI **必须**运行
`python3 .trellis/scripts/task.py finish` (或 `/trellis:finish-work`)。
`after_finish` hook **自动**执行: 提交 worktree → 合并 --no-ff 回主分支 → archive → 销毁 worktree。
**worktree 删除与合并是必须的, 非可选, 非「提醒用户去做」。**
- **收尾两层, 责任不同 (step⓪ AI 层先于 ① git 层)**:
  - **⓪ AI 层 (脚本做不到, 必须 AI 主动)**: 跑收尾脚本前, 先确认本 task 的 **Workflow / 后台 agent 任务已全部终止** —— 用 `TaskList` 查有无悬挂的后台 Workflow/agent (Claude Code Workflow 或退化的 agent 流水线), 有残留则 `TaskStop` 逐个关闭, 再跑 finish。**`trellisx-finish.py` 只销 worktree, 不关 Workflow/Task** —— 关闭悬挂任务是 AI 层职责, 脚本不代劳。**禁 `sleep`/轮询等 workflow 跑完** —— Workflow 异步, 完成自动回 `<task-notification>`; 调用 workflow 后结束本回合, notification 回来再进入本收尾流程, 禁 `Bash(sleep N && ...)` 阻塞 main 空等。
  - **① git 层 (确定性脚本 `trellisx-finish.py`)**: commit → merge --no-ff 各子分支 (子先主后) → 销毁 worktree → archive。冲突则 abort + 报清单, 不强解。
  - 顺序: 先 ⓪ AI 层清悬挂任务 → 再 ① git 层 finish。悬挂任务未清就收尾 = worktree 被合并/销毁时仍有进程在写 = 流程错误。
- 合并冲突 → 收尾脚本 abort + 报冲突 + 非 0 退出 (hook 失败仅 WARN, `task.py finish` 仍返 0)。
  AI **MUST 检 finish 输出有无 `trellisx-finish` 冲突告警**, 有则转手动解决, 禁强解 / 禁当成功。
- check 未过禁跑 finish; 未 archive (worktree 仍在) = 流程未闭环, 禁宣告 Done。
- 📄 **finish 前 spec 自动 sediment 判定 (AI 软约束, 非硬门)**: 跑收尾脚本前, AI **主动判**本 task 有无 "spec 增量需求" —— 即任务过程中产生了非平凡的**可复用契约 / 踩坑教训 / 反复犯错的规则** (见 trellisx-spec sediment 模式判定)。有增量 → 走 `/trellisx-spec` sediment 模式 (提案 → AskUserQuestion 审批 → 写盘) 再 finish; 无增量 (纯执行无新发现) → 跳过, 直接 finish。"如需" = AI 判定, 不强制每次都跑。目的: 学习沉淀不丢, 但不为无增量的 task 增加无谓流程。
- commit 为 owner 授权的强制动作 (脚本直接提交); 需会话 journal 时用 `/trellis:finish-work` 入口。
- 手动兜底: 链路异常时可直接跑 `python3 .trellis/scripts/trellisx-finish.py [--task <tid>]` (幂等可重入)。
<!-- trellisx:end:finish_force -->

---

## 定制 Trellis (供 fork 者)

本节面向想修改 Trellis 工作流本身的开发者。所有定制都通过编辑本文件完成; 脚本只是解析器。

### 改一个步骤的含义

编辑上方 Phase 1 / 2 / 3 章节中对应步骤的 walkthrough 正文。**关键约束**: 若你改一个步骤的 `[required · once]` 标记或新增 `[required · once]` 步骤, 你 MUST 同时在该 Phase 的 `[workflow-state:STATUS]` 标签块中加一条匹配的强制行 —— 否则每轮面包屑漏掉强化, AI 会静默跳过该步骤。回归测试对此有断言。

全部 4 个标签块位于上方 `## Phase 索引` 章节, 紧随每个 phase 摘要之后:

| 作用域 | 对应标签 |
|---|---|
| 无活动任务 (Phase 1 之前) | `[workflow-state:no_task]` (Phase 索引 ASCII art 之后) |
| 整个 Phase 1 (任务已创建 → 准备实现) | `[workflow-state:planning]` (Phase 1 摘要之后) |
| Phase 2 + Phase 3.1–3.4 (实现 + 检查 + 收尾) | `[workflow-state:in_progress]` (Phase 2 摘要之后) |
| Phase 3.5 之后 (已归档) | `[workflow-state:completed]` (Phase 3 摘要之后; **当前 DEAD**) |

### 改每轮提示文本

直接编辑对应 `[workflow-state:STATUS]` 块的正文。编辑后, 运行 `trellis update` (若你是模板维护者) 或重启 AI 会话 (若你在定制自己的项目) —— 无需改脚本。

### 新增自定义 status

加一个新块:

```
[workflow-state:my-status]
你的每轮提示文本
[/workflow-state:my-status]
```

约束:
- STATUS 字符集: `[A-Za-z0-9_-]+` (允许下划线和连字符, 如 `in-review`、`blocked-by-team`)
- 必须有一个生命周期 hook 把 `task.json.status` 写为你的自定义值, 否则该标签永不会被读
- 生命周期 hook 位于 `task.json.hooks.after_*`, 绑定到 `after_create / after_start / after_finish / after_archive` 之一

### 新增生命周期 hook

在你的 `task.json` 加一个 `hooks` 字段:

```json
{
  "hooks": {
    "after_finish": [
      "your-script-or-command-here"
    ]
  }
}
```

支持事件: `after_create / after_start / after_finish / after_archive`。注意 `after_finish` ≠ 状态变更 (它只清活动任务指针); "任务完成"通知用 `after_archive`。

### 完整契约

工作流状态机的运行时契约、所有 status writer 位置、伪状态 (`no_task` / `stale_<source_type>`)、hook 可达性矩阵及其他深层细节, 见:

- `.trellis/spec/cli/backend/workflow-state-contract.md` —— 运行时契约 + writer 表 + 测试不变量
- `.trellis/scripts/inject-workflow-state.py` —— 实际解析器 (只读 workflow.md, 无内嵌文本)
