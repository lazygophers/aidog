---
name: skill-smith
description: 把你的常用技能/重复工作流沉淀成 skill，并优化已有 skill 的自包含元工具。CREATE 模式：捕获一段重复工作流→生成贴本项目约定的 SKILL.md 骨架（frontmatter+触发词、失败模式表、🔴 检查点、反例黑名单）。OPTIMIZE 模式：用内嵌 6 维 rubric 对已有 skill 做 git 棘轮优化（只保留分数提升的改动）。常用技能识别支持「用户口述」+「历史挖掘」(scripts/mine-workflows.py 扫 session jsonl+git log)。自包含、无外部 skill 依赖、可跨机分发。触发词：做个 skill、封装成 skill、沉淀、把这个流程做成 skill、优化 skill、skill 质量、改进 skill、常用技能、重复操作、skill-smith、造 skill。
when_to_use: 用户想把一段重复做的工作流封装成可复用 skill；想优化/改进某个已有 skill 的质量；想知道自己有哪些高频操作值得沉淀；批量盘点项目 skill 时（盘点用配套 skill-auditor agent）
---

# skill-smith — 技能工厂（自包含）

把「你反复做的事」变成可复用的 skill，并让已有 skill 持续变好。**自包含**：评分 rubric 与脚手架模板都内嵌在本 skill，不依赖全局 darwin/nuwa，复制本目录即可在任何项目用。

两个模式：
- **CREATE** — 捕获一段重复工作流 → 生成 SKILL.md 骨架。
- **OPTIMIZE** — 对已有 skill 做 git 棘轮优化（只留改进）。

盘点全项目 skill 质量 → 用配套 `skill-auditor` agent（只读产报告）。

## 内嵌评分 Rubric（6 维，满分 100）

CREATE 的产物与 OPTIMIZE 的目标都按此打分。每维 1-10 分 × 权重，总分 = Σ/10。

| # | 维度 | 权重 | 标准 |
|---|------|------|------|
| 1 | Frontmatter | 16 | name 规范；description 含「做什么+何时用+触发词」；有 `when_to_use`；禁结尾空话尾巴 |
| 2 | 工作流清晰度 | 18 | 步骤有序号、每步有输入/输出、可直接执行 |
| 3 | 失败模式编码 | 20 | 有 if-then 失败分支表（触发/一线修复/兜底）；只写正向流程扣 ≥3 |
| 4 | 检查点 | 12 | 关键/高影响动作前有**显性** 🔴 CHECKPOINT / 🛑 STOP；仅「建议…」措辞不算 |
| 5 | 可执行具体性 | 22 | 有具体命令/参数/路径/示例；「建议/可以考虑/视情况」≥3 处扣 ≥3 |
| 6 | 反例黑名单 | 12 | 有独立「不要做什么」清单；只写「应该做 X」扣 ≥3 |

> 设计借鉴 darwin-skill 的 9 维 rubric（已压缩为可自包含的 6 维结构维度，去掉需子 agent 实测的效果维度），自含可独立运行。

---

## CREATE 模式：沉淀新 skill

### Step 0：确定要沉淀什么（口述 / 挖掘）

**口述（默认）**：用户说「把 X 流程做成 skill」→ 直接进 Step 1。
**挖掘（可选）**：用户问「我有哪些值得沉淀的」或想盘点 → 跑：

```bash
python3 .claude/skills/skill-smith/scripts/mine-workflows.py --top 12 --days 60
```

脚本扫本项目 `~/.claude/projects/<cwd 编码>/*.jsonl` + `git log`，输出高频 Bash 命令 / Skill 调用 / 用户请求动作 / commit scope，标 ⭐候选（≥3 次）。把候选展示给用户勾选。

🔴 CHECKPOINT：挖掘只产「候选」，不自动建 skill。必须用户确认要沉淀哪个，再进 Step 1。

### Step 1：抽取工作流要素

对选定的工作流，问清/推断 6 要素（缺则 grep 项目验证，禁编造）：

1. **触发**：什么场景/什么话会用它？（→ 触发词）
2. **输入**：需要什么前提/参数？
3. **步骤**：有序步骤，每步具体命令/操作。
4. **失败点**：哪几步会失败？失败怎么办？（→ if-then 表）
5. **危险动作**：哪步不可逆/高影响？（→ 🔴 检查点）
6. **反例**：用这个流程时容易做错的事？（→ 黑名单）

### Step 2：定位 + 命名

- skill 目录：`.claude/skills/<name>/SKILL.md`（项目级）或 `~/.claude/skills/<name>/`（全局，跨项目复用时）。
- 命名：kebab-case；项目专属冠项目前缀（如 `aidog-`），通用则裸名。
- 脚本/参考资料放 skill 目录内（`scripts/` `references/`）——自包含原则，复制目录即可用。

### Step 3：生成 SKILL.md（套骨架）

```markdown
---
name: <kebab-name>
description: <一句做什么> + <何时用> + 触发词：<逗号分隔的中英触发词>
when_to_use: <具体场景，分号分隔>
---

# <name>

<一段：这个 skill 解决什么、核心约束>

## 何时用
- <场景1> ...

## 执行流程
### Step 1：<动作>（输入→输出）
...

🔴 CHECKPOINT：<高影响动作前的确认点>

## 失败模式编码（if-then）
| 触发 | 一线修复 | 仍失败兜底 |
|---|---|---|
| ... | ... | ... |

## 反例黑名单（不要做）
1. ❌ ...

## 相关
- <关联 skill/agent>
```

### Step 4：自检评分 + 交付

按内嵌 6 维 rubric 自评，<70 分的维度补齐再交。展示骨架给用户确认。

🔴 CHECKPOINT：交付前确认 frontmatter 的 description 含触发词（否则 skill 不会被自动激活——本项目反复栽的坑）。

---

## OPTIMIZE 模式：优化已有 skill

棘轮机制：每轮只改一个最弱维度，git 记录，只保留分数提升的改动。

### Step 1：基线评分

读目标 `SKILL.md` 全文，按 6 维逐项打分，记下最低维度 + 理由。

### Step 2：单维度改进

针对最低维度改一处（见「策略库」）。**一轮只改一个维度**（多变量同改无法归因）。

### Step 3：git 提交 + 重评

```bash
git add <skill路径> && git commit -m "optimize <skill>: <改了什么>"
```
重新按 6 维打分。

### Step 4：棘轮决策

- 新分 > 旧分 → keep，继续下一最弱维度。
- 新分 ≤ 旧分 → **`git revert HEAD`**（禁 `reset --hard`，保留可追溯链），该 skill 到瓶颈，停。
- 连续 2 轮 Δ<2 分 → 见好就收，停（禁为凑分加冗余）。

🔴 CHECKPOINT：每个 skill 优化完展示 diff + 分数变化给用户确认，再动下一个。用户说不好 → revert。

### 优化策略库（按优先级）

| 优先级 | 短板 | 改法 |
|---|---|---|
| P0 | description 缺触发词 | 补中英触发词（决定能否被激活） |
| P0 | 只有正向流程无失败分支 | 加 if-then 失败模式表（dim3 权重最高） |
| P1 | 高影响动作无检查点 | 插显性 🔴 CHECKPOINT（视觉标记，非「建议」措辞） |
| P1 | 步骤模糊 | 换成具体命令/参数/路径 |
| P2 | 无反例清单 | 加「不要做什么」黑名单 |
| P2 | 软化措辞泛滥 | 删「建议/可以考虑/视情况」，改硬指令 |

## 失败模式编码（if-then）

| 触发 | 一线修复 | 仍失败兜底 |
|---|---|---|
| 挖掘脚本无会话数据 | 确认 cwd 是仓库根 + `~/.claude/projects/` 有本项目目录 | 退回纯口述模式 |
| 不在 git 仓库（OPTIMIZE 无法 revert） | 改前 `cp SKILL.md SKILL.md.bak` 手动备份 | 提醒用户 git init |
| 生成的 skill 不被激活 | 检查 description 是否含触发词 | 触发词前置到 description 开头 |
| 优化后分数没涨反跌 | `git revert HEAD` 回滚 | 标该 skill 到瓶颈，停 |
| 同 session 自评有乐观偏差 | 重要 skill 派 `skill-auditor` agent 独立评分 | 至少隔开 context 重读再评 |

## 反例黑名单（不要做）

1. ❌ 挖掘出候选就自动建 skill —— 必用户勾选确认。
2. ❌ 一轮改多个维度 —— 无法归因，每轮一个。
3. ❌ 用 `git reset --hard` 回滚 —— 丢工作树，必用 `git revert`。
4. ❌ 触顶后为凑分加废话段落 —— 见好就收。
5. ❌ description 不含触发词就交付 —— skill 不会被激活。
6. ❌ 把脚手架写成空泛模板（「处理数据」「视情况而定」）—— 必具体命令/路径。
7. ❌ 编造工作流要素 —— 不确定就 grep 项目或问用户。

## 相关

- 全项目 skill 质量盘点：`skill-auditor` agent（本 skill 配套）
- 造人物/主题思维 skill（重，带 web 调研）：全局 `huashu-nuwa`
- 重度 9 维优化 + 成果卡片：全局 `darwin-skill`
