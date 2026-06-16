# 生成脚本 sh→python+uv(PEP723) + 移到 ~/.aidog/scripts/

## Goal

把 aidog 生成的全部 hook/statusline shell 脚本（`.sh`）改为 Python 脚本，用 `uv run --script` + PEP723 内联依赖声明执行（uv 缺失时 fallback `python3`）；脚本从 `~/.aidog/` 直接存放改为 `~/.aidog/scripts/` 子目录。uv 不存在时由 aidog 在生成时 UI 询问是否自动安装。

## 硬约束 / 决策（brainstorm 定）

- **转换范围**：**全部生成的 .sh**——通知 hook（`aidog-notify-complete.sh` / `aidog-notify-waiting.sh`）+ statusline（`aidog-statusline.sh`）+ subagent statusline（`aidog-subagent-statusline.sh`）。
- **执行方式**：脚本用 `uv run --script` + PEP723 内联依赖头（即使现仅 stdlib，预留+隔离）；uv 缺失 → 生成/调用走 `python3 <script>`。
- **uv 检测 + 询问时机**：在 **aidog 生成/注入脚本时**（有 UI 的一侧）检测 uv：
  - uv 存在 → 脚本 shebang/调用用 uv。
  - uv 不存在 → aidog UI 弹「自动安装 uv?」→ 是：aidog 安装 uv（官方 install 脚本）后用 uv；否：生成用 `python3` 的脚本。
  - hook 脚本被 Claude Code/Codex **非交互**调用，脚本内不能弹窗 → 询问只能在 aidog 侧。
- **脚本位置**：`~/.aidog/scripts/`（新建子目录），不再 `~/.aidog/` 根。

## ⚠️ 串行依赖（重要）

本任务与 **`06-14-notif-hook-default-inject`**（进行中）**共享文件**：`gateway/hooks.rs`(build_hook_script)、`lib.rs`(generate_hook_scripts / generate_statusline_script / 脚本路径)。
→ **本任务 exec 必须等 notif-hook-default-inject 合并回 master 后**在更新基线上进行，禁并行（否则 hooks.rs/lib.rs 必冲突）。

## Requirements

### R1 — 脚本内容 .sh → .py
- `gateway/hooks.rs build_hook_script`：bash → Python（notify：读 env `ANTHROPIC_BASE_URL` 推导 `/api/notify` + `ANTHROPIC_AUTH_TOKEN` Bearer，POST，project=cwd basename；用 stdlib `urllib`/`json`/`os`，无第三方依赖）。文件名 `.sh`→`.py`。
- statusline / subagent statusline 脚本生成（`generate_statusline_script` 等）：bash → Python，保持原输出契约（statusline 读 stdin JSON、输出渲染行；逐任务 JSONL 逻辑见 [[subagent-statusline-native]]）。
- 每脚本头加 PEP723 块（`# /// script` … `# ///`，deps 现为空列表，预留）。

### R2 — uv / python3 执行选择
- aidog 生成时检测 uv（`which uv` / `uv --version`）。脚本调用形式二选一：
  - uv 可用：hook 配置里命令用 `uv run <script.py>`（或 shebang `#!/usr/bin/env -S uv run --script`）。
  - uv 不可用：命令用 `python3 <script.py>`（或 shebang `#!/usr/bin/env python3`）。
- 选择结果影响：Claude Code `hooks.Stop/Notification` 的 command 串、Codex `notify=[...]`、statusLine `command`。
- 新增后端：uv 检测 command + (可选)uv 自动安装 command；前端 UI 询问流程。

### R3 — 脚本目录 ~/.aidog → ~/.aidog/scripts/
- 所有脚本写入 `~/.aidog/scripts/`（`create_dir_all`）。
- 更新所有引用脚本绝对路径处（hook command 串、statusLine command、Codex notify）指向新目录。
- 旧 `~/.aidog/*.sh` 清理（生成新脚本时删旧 .sh，避免残留；或一次性迁移）。

### R4 — uv 询问 UI（aidog 侧）
- 注入 hook / statusline 时若 uv 缺失 → 前端弹询问「检测到未安装 uv，是否自动安装？（否则用 python3）」。
- 是 → 调后端安装 uv command；否 → 标记用 python3 生成。
- 该选择可持久化（避免每次问），或每次注入时问——实施时定（建议持久化到 settings）。

## Acceptance Criteria

- [ ] 生成的脚本全为 `.py`，位于 `~/.aidog/scripts/`
- [ ] 脚本含 PEP723 头；uv 可用走 uv run、不可用走 python3
- [ ] notify 脚本功能等价（POST /api/notify 成功）；statusline 输出契约不变
- [ ] hook/statusLine/Codex notify 的 command 串指向新 .py 路径 + 正确执行器
- [ ] uv 缺失时 aidog UI 询问自动安装；否则 python3
- [ ] 旧 ~/.aidog/*.sh 不残留
- [ ] cargo clippy 无新 warning + cargo test 绿；yarn build 绿；check-i18n 零缺失；8 locale parity

## Definition of Done

- cargo clippy 无 warning + cargo test 绿；yarn build 绿；check-i18n 零缺失
- 改动落 worktree（基于 notif-hook 合并后的 master），闭环 check→commit(merge)→archive
- 更新 [[notification-module]] / [[subagent-statusline-native]]（脚本改 python+uv+新目录）

## Technical Approach

- 后端 `gateway/hooks.rs` + statusline 生成（lib.rs `generate_statusline_script`/`generate_hook_scripts`）：脚本内容改 Python 字符串；路径常量 `~/.aidog/scripts/`；文件名 `.py`。
- uv 检测：`std::process::Command which uv`；安装：调官方 `curl -LsSf https://astral.sh/uv/install.sh | sh`（用户同意后）。
- command 串构造抽函数：`script_invoker(uv_available) -> "uv run" | "python3"`，hook/statusLine/codex 共用。
- 前端：注入流程加 uv 检测结果 + 询问 modal；api 封装；i18n。
- ⚠️ 实施安全：写用户真实 `~/.aidog/scripts/`；worktree cargo test 用构造断言（脚本内容/command 串/路径），**禁真跑写用户 HOME / 真装 uv**。

## Out of Scope

- 通知/statusline 的功能逻辑变更（仅换语言+执行器+路径）
- 默认注入开关（notif-hook-default-inject 任务负责）

## Technical Notes

- 现状脚本：build_hook_script(hooks.rs:53, bash) / generate_hook_scripts(lib.rs:1850) / generate_statusline_script(lib.rs:1824) / SCRIPT_COMPLETE/WAITING(hooks.rs:38)
- 脚本被非交互调用：CC hooks.Stop/Notification(stdin JSON)、Codex notify($1 JSON)、statusLine(stdin JSON)
- PEP723: `# /// script` + `# dependencies = []` + `# ///`
- 串行前置：notif-hook-default-inject 合并后基线
- 参考 [[notification-module]] / [[subagent-statusline-native]] / [[statusline-persistence-flow]]

## 范围调整（实施中决策, 用户确认）

- statusline / subagent statusline 脚本（~1300 行 shell, 字节级输出契约）**拆出为独立任务** `statusline-py-uv` 转 Python（配 golden-output 回归测试）。
- **本任务交付收窄为**: notify hook(complete/waiting) → Python+uv+PEP723 + **全部脚本(含 statusline)迁移到 `~/.aidog/scripts/`** + 旧 .sh 清理 + uv 检测/安装/询问 UI + 执行器选择基建(scripts.rs ScriptInvoker)。statusline 内容暂保留 bash, 仅迁路径。
- 用户已确认 statusline 也必须转 Python(后续独立 task)。
