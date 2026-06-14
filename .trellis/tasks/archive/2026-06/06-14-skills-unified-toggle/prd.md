# Skills 列表重构 (统一不分 agent + per-item 启用切换 + 总计样式)

## Goal

Skills 页改为「统一已安装列表」：不再按 agent 切换；一条/skill；每个 skill item 右侧展示各 agent（claude/codex）启用态（启用/未启用样式），点击切换该 agent 的启用/关闭。所有启用/关闭/列表操作**全部走 `npx skills` 命令，禁手动 fs 操作**。总计统计样式重做。

## 硬约束（用户明确）

- ⛔ **所有操作必须用 `npx skills` 命令解决，不手动 fs**（list/enable/disable/update 全走 CLI）。当前后端 `remove()` 用 `fs::remove_dir_all`、`scan_installed()` 用 `fs::read_dir` —— **都要改成 npx**。
- 删 agent 切换 UI。
- 已装列表不分 agent（统一一条/skill）。
- agent 仅在每个 skill item 右侧展示（claude/codex 图标，启用/未启用样式，可点击切换）。

## npx skills 技术模型（已联网+本机核实）

- **统一列表**: `npx skills list --json -g` → `[{name, path, scope, agents:["Claude Code","Codex",...]}]`。一条/skill；`agents[]` 含某 agent 显示名 = 该 agent 已启用该 skill。
- **启用 (enable)**: skill 已在规范存储 `~/.agents/skills/<name>`，source 在锁文件 `~/.agents/.skill-lock.json` 的 `skills[<name>].source`（如 `"ratacat/claude-skills"`）。命令：`npx skills add <source> -s <name> -a <slug> -g -y`。
- **关闭 (disable)**: `npx skills remove -s <name> -a <slug> -g -y`（npx 原生 remove，**替代手动 fs 删**）。
- **agent slug**（`-a` 用）: claude → `claude-code`、codex → `codex`（help 示例 `--agent claude-code cursor`）。**现后端 `SkillAgent::Claude.cli_name()=="claude"` 疑似错，须改 `claude-code`**；display 名映射 "Claude Code"/"Codex"（解析 list json agents[] 用）。
- **scope**: 全局 `-g`；项目级在项目目录内执行（不带 -g）+ 读该项目 `.agents/.skill-lock.json`。
- **update**: `npx skills update -g -y`（保留）。

## Requirements

### R1 — 后端全 npx 化（重写 list/enable/disable）
- `skills_list_installed(scope)`：改调 `npx skills list --json -g`（项目级在项目目录跑），解析为 `Vec<SkillInfo>`，每条含 `name` + `enabled_agents`（哪些目标 agent 启用：claude/codex 子集，从 agents[] 显示名映射）。**不再 fs::read_dir 扫目录**。
- `skills_enable(name, agent, scope)`：读 scope 对应 `.agents/.skill-lock.json` 取 `skills[name].source` → `npx skills add <source> -s <name> -a <slug> [-g] -y`。source 缺失 → 返回明确错误。
- `skills_disable(name, agent, scope)`：`npx skills remove -s <name> -a <slug> [-g] -y`。替换现 `remove()` 的 fs::remove_dir_all。
- `SkillAgent::cli_name()`：claude → `claude-code`（修正）；新增 display 名映射用于解析 agents[]（"Claude Code"/"Codex"）。
- 移除/弃用 `scan_installed`(fs) 与旧 `install`/`remove`(fs) 的直接 fs 路径（或内部改为 npx）。命令签名调整同步 api.ts。

### R2 — 前端统一列表 + per-item agent 切换
- 删除 agent 切换 UI（之前的 agent 图标行作为"切换/筛选"的语义移除）。
- 已装列表统一：一行/skill（name + description）。
- 每行**右侧**展示 claude + codex SVG 图标：启用态（在该 skill 的 enabled_agents 内）= 启用样式（高亮/实心）；未启用 = 未启用样式（灰/描边）。
- 点击某 skill 行右侧的 agent 图标：启用→调 `skills_disable`，未启用→调 `skills_enable`；成功后刷新该行/列表。切换中显示 busy 态，禁并发。
- scope 筛选保留（global 默认 / project 选目录），不分 agent。

### R3 — 总计统计样式重做
- 「已安装总计 72」换更醒目的样式（如大数字卡片 / glass 统计块），可附每 agent 启用数（claude: N、codex: M）。具体视觉走 Liquid Glass，简洁醒目。
- 统计随 enable/disable/scope 变化刷新。

## Acceptance Criteria

- [ ] 列表统一一条/skill，不按 agent 切换；无 agent 切换 UI
- [ ] 每行右侧 claude/codex 图标，启用/未启用样式区分
- [ ] 点击图标切换：启用调 npx add、关闭调 npx remove，列表刷新
- [ ] 后端 list/enable/disable **全走 npx skills**，无 fs::read_dir/remove_dir_all 直接操作
- [ ] agent slug 用 claude-code/codex（修正 claude）
- [ ] 总计样式重做且随操作刷新
- [ ] cargo clippy 无新 warning + cargo test 绿；yarn build 绿；check-i18n 零缺失；8 locale parity

## Definition of Done

- cargo clippy 无 warning + cargo test 绿；yarn build 绿；check-i18n 零缺失
- 改动落 worktree，闭环 check→commit(merge)→archive
- 非平凡发现（npx skills 模型/锁文件 source/agent slug 修正）落 cortex

## Technical Approach

- 后端 `gateway/skills.rs`：list/enable/disable 改 `Command::new("npx").args(["skills",...])`，list 解析 `--json`；enable 读锁文件 source（serde_json 读 `.agents/.skill-lock.json`，仅**读取元数据喂给 npx 命令**，操作仍是 npx，不违反"全 npx"约束）。SkillInfo 加 `enabled_agents: Vec<SkillAgent>`。
- 前端 `pages/Skills.tsx`：统一列表渲染 + 右侧 agent 图标 toggle + 总计卡片；删 agent 切换 state。api.ts 同步类型 + skillsApi.enable/disable。
- ⚠️ **实施安全**：worktree 内验证 npx 行为时**禁对用户真实 72 个 skill 做 enable/disable 破坏性测试**；只读 `list --json` 可跑，写操作靠代码审查 + 单测（mock/参数构造测试），不真跑 add/remove 改用户环境。

## Out of Scope

- catalog 浏览/搜索/安装新 skill（装新需求未提）
- 非 claude/codex 的其他 agent（list json 虽含多 agent，UI 仅显 claude/codex 两个）
- 锁文件写入/skill 下载

## Technical Notes

- npx CLI: `list --json -g [-a]` / `add <pkg> -s -a -g -y` / `remove -s -a -g -y` / `update -g -y`
- 锁文件: `~/.agents/.skill-lock.json` → `{version,skills:{<name>:{source,sourceType,sourceUrl,...}}}`
- 规范存储: `~/.agents/skills/<name>`；agent 目录 `~/.<agent>/skills/<name>` 为 symlink
- agents[] 显示名: "Claude Code"/"Codex"（解析映射）；-a slug: claude-code/codex
- 现状: skills.rs scan_installed(fs)/remove(fs) 待改 npx；Skills.tsx 总计(:235-237)/agent 行(:229-272)/已装列表(:276+)
- 参考 [[skills-management-module]]；merge 注意 [[worktree-stale-base-merge-conflict]]
