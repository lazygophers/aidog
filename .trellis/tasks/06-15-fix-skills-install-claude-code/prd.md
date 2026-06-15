# PRD: skills 安装 claude code 未生效

## 症状
用户报：aidog Skills 页安装 skill，选 claude code + codex（全局），装完只有 codex 生效，claude code 没生效。
- 范围：全局
- 影响面：所有新安装的 skill
- UI 展示：该 skill 的 enabled_agents 不含 claude（UI 显示没 claude）
- app 实际使用：未测试（不确定 claude code app 是否真能用该 skill）
- 重启：试过，无效
- 安装时报错：无

## 排查结论（已确认正常的点，证据见引用）

### 1. skills CLI `-a claude-code` 路径正确
- `skills.rs:878-884` install_args 构造 `add <id> -a claude-code [-g] -y`
- SkillAgent::Claude.cli_slug() = "claude-code"（`skills.rs:72-74`，单测 `agent_slug_and_display` 守护）
- CLI 源码（`~/.npm/_npx/5606f1555d02ef53/node_modules/skills/dist/cli.mjs`）runAdd: 显式 `-a` 走 `targetAgents = options.agent` 分支，**无 detectInstalled 过滤**，claude-code 与 codex 一视同仁

### 2. claude-code 全局安装目标路径正确
- CLI agents.claude-code: `globalSkillsDir = join(claudeHome, "skills")`，claudeHome = `CLAUDE_CONFIG_DIR || ~/.claude`
- 单 agent（只选 claude-code）→ copy 模式写 `~/.claude/skills/<name>`（`uniqueDirs.size<=1 → installMode="copy"`）
- 多 agent（claude+codex）→ symlink 模式：canonical `~/.agents/skills/<name>` + symlink `~/.claude/skills/<name>` → canonical；codex（universal）直接用 canonical
- **实证**：`ls ~/.claude/skills/` 已有 44 个 symlink（→ `../../.agents/skills/<name>`）+ 4 个真目录（copy），证明历史安装 claude-code 成功

### 3. list 解析正确
- `skills.rs:399-404` parse_list_json: `enabled_agents = SkillAgent::all().filter(|a| agent_names.contains(&a.display_name()))`，display_name "Claude Code"/"Codex" 精确匹配 CLI JSON 输出
- **实证 1**（完整 env）：`npx skills list --json -g` → 45 skill，44 含 Claude Code（仅 `skill-anything` 缺，它是 codex/opencode 独立目录边缘 case）
- **实证 2**（aidog dev 进程 env）：同上结果，44/45 含 Claude Code → **dev env 下完全正常**

### 4. 缓存正确
- 磁盘缓存 `~/.aidog/skills-cache.json` global scope 45 items，仅 `skill-anything` 缺 claude，其余 44 全有 → 缓存数据正确，非陈旧
- invalidate 逻辑（`skills.rs:370`）：内存 remove + persist 同步删磁盘 scope → 重启后重跑 list，不会卡旧缓存

### 5. spawn env 不设 PATH / 不 env_clear
- `skills.rs:1551-1556` 仅 apply_proxy_env（代理 URL），**不主动设 PATH，继承父进程 env**
- dev 进程 env（终端启动）含完整 mise PATH → 正常
- 打包版（Finder 启动）env 极简（launchd 默认），但用户 codex 成功 = npx 在 PATH → claude-code 理论同路径该成功

## 无法排除的点（需用户验证）

**根因尚未定位**。所有 dev env 可测的链路都正常。剩余唯一变量：**打包版 GUI env**（我从终端无法精确复现 Finder 启动的 env）。

## 决定性验证（用户跑）

在**打包版 aidog**（Finder 启动）Skills 页装一个**新的**全局 skill（选 claude code + codex），装完**立即**（不重启）跑：

```bash
SKILL=<刚装的 skill name>
# 1. 文件是否落到 claude 目录？
ls -la ~/.claude/skills/$SKILL 2>/dev/null || echo "NO CLAUDE DIR/FILE"
# 2. CLI list 是否含 Claude Code？（终端完整 env 跑）
npx skills list --json -g 2>/dev/null | python3 -c "import json,sys;[print(s['name'],'->',s['agents']) for s in json.load(sys.stdin) if '$SKILL' in s['name']]"
```

**分支**：
- 若 (1) 有文件/symlink **且** (2) 含 "Claude Code" → 安装与 CLI 都正常，根因 = aidog 前端 SWR 缓存/UI 渲染 bug（非安装问题）
- 若 (1) 无文件 → 根因 = install 步骤在打包版 env 下 claude-code 静默失败（codex 成功但 claude-code 步骤 error 被吞）→ 修复方向：install 后端不吞 claude-code 步骤 stderr，前端展示逐 agent 成败
- 若 (1) 有文件 但 (2) 不含 "Claude Code" → 根因 = 打包版 env 下 list 的 detectInstalledAgents 漏 claude（HOME/CLAUDE_CONFIG_DIR 异常）→ 修复方向：spawn npx 时显式注入 HOME=$(真实 home)

## 状态
**阻塞**：等用户跑决定性验证。无验证结果不进 exec（避免盲改）。

