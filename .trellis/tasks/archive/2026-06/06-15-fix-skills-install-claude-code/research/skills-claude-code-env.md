# Research: skills claude-code 检测依赖 HOME/CLAUDE_CONFIG_DIR

## 根因（cli.mjs 源码证据）

`~/.npm/_npx/<hash>/node_modules/skills/dist/cli.mjs`：

- 行 939：`const claudeHome = process.env.CLAUDE_CONFIG_DIR?.trim() || join(home, ".claude");`
- 行 1027-1031（claude-code agent 定义）：
  ```js
  "claude-code": {
    skillsDir: ".claude/skills",
    globalSkillsDir: join(claudeHome, "skills"),
    detectInstalled: async () => { return existsSync(claudeHome); }
  }
  ```
- 行 1087-1093（codex agent 定义）：`detectInstalled = existsSync(codexHome) || existsSync("/etc/codex")`

`home = os.homedir()`：Node 先读 `process.env.HOME`，缺失才 fallback `os.userInfo().homedir`（getpwuid）。

## claude vs codex 不对称

| agent | 检测路径 | 备注 |
|-------|---------|------|
| claude-code | `CLAUDE_CONFIG_DIR \|\| ~/.claude` | **仅依赖 HOME env**（无 fallback 兜底路径） |
| codex | `codexHome \|\| /etc/codex` | 有 `/etc/codex` 兜底，容错强 |

打包版 GUI（Finder/launchd 启动）env 极简。若 `HOME` env 缺失或被 launchd 设为异常值 → `os.homedir()` 返回异常 → claudeHome 解析错 → `detectInstalled` 返 false → list 的 `agents[]` 不含 Claude Code → aidog UI 显示该 skill 无 claude。codex 因 `/etc/codex` 兜底或路径不同仍命中。

## list 的 agents[] 如何生成

CLI `skills list --json` 对每 skill 逐 agent 调 `detectInstalled`（或 `existsSync(globalSkillsDir/<name>)`）决定 `agents[]`。aidog `parse_list_json`（skills.rs:399-404）按 displayName 精确匹配映射 enabled_agents。

## 修复方向（分支3）

spawn npx 时**显式注入 `HOME`**（用 `dirs::home_dir()` 即 getpwuid 解析，比继承 env HOME 可靠），`CLAUDE_CONFIG_DIR` 若父 env 已设则透传。覆盖 `run_npx`（skills.rs:791）+ `run_npx_in_scope`（skills.rs:1562）两处 spawn。

## 验证（dev env 无法复现，打包 env 决定性）

修复后打包版重装新 skill，`ls ~/.claude/skills/<name>` + `npx skills list --json -g | grep <name>` 应含 Claude Code。
