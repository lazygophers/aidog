# Codex CLI 参数（codex --help 实测，2026-06-12）

codex 已装：`/opt/homebrew/bin/codex`。

## 关键全局参数
- `-c, --config <key=value>`：覆盖 config.toml 值，dotted path，value 按 TOML 解析。例 `-c model="o3"` / `-c 'sandbox_permissions=["disk-full-read-access"]'` / `-c shell_environment_policy.inherit=all`。
- `-p, --profile <PROFILE>`：用 config.toml 里的 profile（**对照 Claude Code `--settings`**：aidog 可为每分组生成 profile，命令 `codex -p <group>`）。
- `-m, --model <MODEL>`。
- `-s, --sandbox <SANDBOX_MODE>`：沙箱策略。
- `-a, --ask-for-approval <POLICY>`：untrusted / on-failure(deprecated) / on-request / **never**。
- `--dangerously-bypass-approvals-and-sandbox`：**跳过所有确认 + 无沙箱（bypass，用户要启用）**。
- `--dangerously-bypass-hook-trust`：跳过 hook trust。
- `--enable <FEATURE>` / `--disable <FEATURE>`：功能开关（= `-c features.<name>=true/false`，可重复）。
- `-C, --cd <DIR>`。

## 子命令
exec(非交互) / review / login / mcp / plugin / sandbox / doctor / features 等。

## 分组「复制 Codex 命令」设计（对照 Claude Code `claude --settings ~/.aidog/settings.{group}.json`）
- 每分组在 `~/.codex/config.toml` 生成 profile（含 model_provider base_url 指向 aidog 本地代理 + 该分组路由），命令：
  `codex -p <group> --dangerously-bypass-approvals-and-sandbox -a never [--enable <feat> ...]`
  （尽可能启用功能 + bypass）。
- 具体 profile/provider 字段待 codex-config.md（config-reference）确认。
