# Research: Claude Code / Codex CLI 安装与版本查询技术

- **Query**: 两个 CLI 的包名 / 版本查询 / 升级命令 / 官方安装方式 / 跨平台差异
- **Scope**: external（cc-switch 源码 + npm registry 推断）
- **Date**: 2026-07-08
- **依据**: cc-switch `src-tauri/src/commands/misc.rs` 的硬编码命令字符串（这是经过维护者验证的真相源，比文档稳定）

## Findings

### Claude Code CLI

| 项 | 值 | 出处 |
|---|---|---|
| npm 包名 | `@anthropic-ai/claude-code` | misc.rs:1936 `npm_package_for` |
| 版本查询命令 | `claude --version` | misc.rs:1006-1009（POSIX）/ `scan_cli_version` Windows |
| 版本输出格式 | 形如 `2.1.156` / `2.1.156-beta.1`，用正则 `\d+\.\d+\.\d+(-[\w.]+)?` 提取 | misc.rs:961-962 |
| **官方推荐安装（POSIX）** | `bash -c 'tmp=$(mktemp) && curl -fsSL https://claude.ai/install.sh -o $tmp && bash $tmp; status=$?; rm -f $tmp; exit $status'` | misc.rs:439-440 `CLAUDE_INSTALL_UNIX` |
| npm 兜底安装 | `npm i -g @anthropic-ai/claude-code@latest` | misc.rs:492 |
| **官方推荐升级** | `claude update`（CLI 自带子命令） | misc.rs:501-507 `official_update_args` |
| 锚定路径特征 | 原生安装器落在 `~/.local/share/claude/versions/`（misc.rs:2244-2246 注释） | `anchored_command_from_paths` |
| install.sh 形态 | bash 脚本，**不通过 npm 分发**，bin launcher 自带 `update` 子命令内部 dispatch | misc.rs:2244-2248 |
| 预发布通道 | npm dist-tag `next`（仅当本地严格领先 latest 时纳入比较） | misc.rs:802 |

**关键事实**：Anthropic 已把 native installer（claude.ai/install.sh）列为**首推**，npm 列为传统方式（misc.rs:2411-2420 注释：上游推荐方式这一事实）。native installer 装的 claude 在 PATH 里比 nvm/homebrew 更靠前，**用 npm 升级会装到别处且被原生那份遮蔽**——这是锚定升级必须区分来源的根因。

### Codex CLI

| 项 | 值 | 出处 |
|---|---|---|
| npm 包名 | `@openai/codex` | misc.rs:1937 |
| 版本查询命令 | `codex --version` | 同 claude 路径 |
| **官方推荐安装** | `npm i -g @openai/codex@latest`（**无 native installer**，不像 claude） | misc.rs:493 |
| 升级命令 | `codex update`（CLI 自带，但**仅 POSIX 启用**，Windows 不在 `prefers_official_update` 名单） | misc.rs:2121-2136 |
| **平台二进制损坏自愈** | 主包 `@openai/codex` 是纯 JS launcher + 平台二进制 optional 依赖 `@openai/codex-<triple>`（同 esbuild/swc 模式） | misc.rs:2139-2163 `codex_repair_command` |
| 损坏修复命令 | `<npm 绝对> uninstall -g @openai/codex || true; <npm 绝对> i -g @openai/codex@latest` | misc.rs:2180-2182 |
| Windows 已知问题 | EPERM 文件锁 / 版本 bump 残留（openai/codex#21872, #19824），cc-switch 暂不做 Windows 自愈 | misc.rs:2185-2191 |

**关键事实**：codex 走 npm 分发 + optional 依赖平台二进制模式，平台二进制缺失时 `--version` 报 `Missing optional dependency`，普通 `npm i -g @pkg@latest` **是 no-op 修不好**（npm 视 optional 依赖缺失为非致命）。唯一可靠修复是 uninstall + install 清掉残骸再装。

### 跨平台差异

#### macOS / Linux（POSIX）

- 版本查询：`$SHELL -lic '{tool} --version'`（登录 shell，包含用户 PATH）
- 安装脚本：`bash -c '<curl 到 mktemp>&&bash $tmp'` + `set -e` + `set -o pipefail`
- 执行 wrapper：`bash -c "<脚本>"`，强制 bash（fish/zsh `set -e` 语义不同）
- 锚定 npm 升级：`/opt/homebrew/bin/npm i -g ...`（绝对路径，绕开 GUI PATH 缺失）
- Homebrew formula 识别：真身路径含 `Cellar/<formula>/` → `brew upgrade <formula>`（不是 npm）

#### Windows 原生

- 版本查询：禁用 PATH shell，强制 `scan_cli_version` 只跑已定位的真实 .exe/.cmd（避免 App Execution Alias 误触发协议处理器，曾导致 Windows 版整体被禁用 — misc.rs:992-995 注释）
- 安装：全部走 `npm i -g`（install.sh 是 bash 脚本，Windows 跑不了）
- 执行 wrapper：临时 `.bat` 文件（`@echo off` + `call <cmd>` + `if errorlevel 1 exit /b %errorlevel%`），`cmd /C` 调用，`CREATE_NO_WINDOW = 0x08000000` 抑制控制台窗口
- 多字节编码：stdout/stderr 走 OEMCP / ACP fallback（misc.rs:286-347 `decode_windows_command_output`）
- 路径候选扩展名：`.cmd` / `.exe`，无扩展名裸文件需 `windows_runnable_sibling_for_extensionless_tool` 找同目录 `.cmd/.exe` 兜底

#### Windows WSL（cc-switch 特有，aidog 暂不需要）

- 工具路径 override 是 UNC `\\wsl$\<distro>\...`，跨 `wsl.exe` 边界后 Windows 主机绝对路径失效
- 执行 wrapper：`wsl.exe -d <distro> -- <shell> <flag> "<POSIX 命令>"`
- 用户可选 shell（sh/bash/zsh/fish/dash）+ flag（-lic/-lc/-c）

### 不依赖 npm 的安装方式

**Claude Code 有 native installer**：`https://claude.ai/install.sh`（misc.rs:440）。Anthropic 官方首推，装到 `~/.local/share/claude/versions/`，bin launcher 自带 `update` 子命令。

Codex 无 native installer，仅 npm（misc.rs:2126 codex 不在 `prefers_official_update` 的 Windows 名单但 POSIX 在 — POSIX 用 `codex update || npm i -g` 短路链）。

**没有发现 brew / apt / scoop 等系统包管理器的官方 source**——homebrew 上的 claude/codex formula 是社区维护，真身落在 `Cellar/<formula>/`，升级命令必须用 `brew upgrade <formula>` 而非 npm（misc.rs:2199-2202），否则会旁路装第二份。

## 关键结论（5 条）

1. **Claude Code 双安装路径**：native installer（首推，PATH 优先）+ npm（传统）。两者可能并存且版本不一致——锚定升级必须区分。
2. **Codex 是 npm-only + 平台二进制 optional 依赖模式**：损坏时 `npm i -g` 是 no-op，必须 uninstall + install 自愈。
3. **版本号提取统一正则** `\d+\.\d+\.\d+(-[\w.]+)?`：兼容 codex 的 `0.1.2505172116` 时间戳式 patch（用 u64 容纳，misc.rs:809-810）。
4. **Windows 不能走 PATH shell 探测版本**：App Execution Alias 会误触发协议处理器，必须用 `where` + 直接 spawn 已定位的真实 .exe/.cmd。
5. **跨平台命令拼接差异大**：POSIX 用 bash + mktemp + set -e；Windows 用 .bat 临时文件 + cmd /C + CREATE_NO_WINDOW。

## 对 aidog PRD 的建议

- **claude 安装策略**：POSIX 优先 native installer（`https://claude.ai/install.sh`），`|| npm i -g @anthropic-ai/claude-code@latest` 兜底；Windows 直接 npm。
- **codex 安装策略**：全平台 `npm i -g @openai/codex@latest`，不做 native installer。
- **claude 升级**：先锚定到原生安装器路径（`~/.local/share/claude/versions/`）→ `<bin 绝对> update`；否则锚定同级 npm → `<npm 绝对> i -g @anthropic-ai/claude-code@latest`；最后 fallback `claude update`（依赖 PATH，可能失败）。
- **codex 升级**：锚定同级 npm → `<npm 绝对> i -g @openai/codex@latest`；如检测到 `runnable=false` + npm 来源（nvm/fnm/mise/homebrew），跑 uninstall + install 自愈。
- **版本查询**：直接复用 `env.rs::ensure_runtime_path` + `$SHELL -lic 'claude --version'`（aidog 已有这套基础设施）。
- **Windows 版本探测**：抄 cc-switch 的 `where <tool>` + 直接 spawn 真实 .exe/.cmd，禁 PATH shell。
- **latest 版本拉取**：直接 `GET https://registry.npmjs.org/@anthropic-ai/claude-code` 取 `dist-tags.latest`（aidog 已有 reqwest 共享 client，见 `gateway/http_client.rs`）；不引入预发布通道补查。
