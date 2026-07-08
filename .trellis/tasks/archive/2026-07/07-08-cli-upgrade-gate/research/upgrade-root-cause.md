# Research: CLI 升级按钮点击无反应根因

- **Query**: 定位 About 页 Codex/Claude CLI 升级按钮"点击无反应"根因（只读诊断，不改码）
- **Scope**: mixed（内部代码 + 本地实测 claude/codex update 子命令行为）
- **Date**: 2026-07-08

## 摘要 / 根因判定（按可能性排序）

### ★ 最可能：挂载期 `cli_check_versions` 阻塞 → 升级按钮处于 disabled 态被静默忽略

**机制**：
- `src/pages/About.tsx:39-43` `useEffect` 进入页面就调 `handleCliCheck()` → `setCliBusy("check")` (About.tsx:90)
- 升级按钮的 disabled 条件 = `cliBusy !== "" || isPending`（About.tsx:382, 394）
- 当 `cliBusy === "check"` 时，**所有 CLI 按钮（包括每个工具的"升级"）一律 disabled**，按钮 label 不变（仍叫"升级"），用户肉眼看不出禁用态 → 误以为"点击无反应"
- 用户进入 About 页后立即点"升级" → 此时 `cli_check_versions` 尚未返回 → 点击被 React 静默吞掉（disabled button 不触发 onClick）

**为何 `cli_check_versions` 慢**（`commands/cli_env.rs:282-307`）：
- 同步 Tauri command（`pub fn`，非 async），Tauri 在专用线程跑但前端 invoke 仍 await
- 单次调用串行 spawn 链路（POSIX，每 spawn ~100-200ms）：
  - `probe_version("claude")` = 1×`claude --version` + 1×`which claude`（cli_env.rs:117, 121）
  - `which_all("claude")` = 1×`which -a claude`（cli_env.rs:159-182）
  - 若 ≥2 路径 → `enumerate_installations("claude")` = N×`canonicalize` + N×`claude --version`（cli_env.rs:230-267）
  - 同上对 codex 再来一遍
- 本机 `which -a claude` 返 **2 路径**（`/Users/luoxin/.local/share/mise/installs/node/latest/bin/claude` + `/Users/luoxin/.local/bin/claude`），触发 enumerate 分支 → 单次 check ≈ 8-10 spawns × 100-200ms ≈ **1-2 秒**
- 期间前端按钮全程 disabled。若 brew 锁 / 网络慢 / `which -a` 在某些 PATH 配置下卡顿，禁用窗口进一步拉长。

**为何这一定就是症状**：
- 升级完成后 `handleCliUpgrade` finally 块前还会 `await handleCliCheck()`（About.tsx:128）→ 第二次进入"禁用窗口"
- 诊断、安装按钮同理，但用户最常点的就是"升级"

### 次可能：`cli_upgrade` 内 `.output()` 无超时，子进程卡住致 invoke 永不返回

**机制**：`commands/cli_env.rs:361, 381, 393, 396, 441` 全部用 `Command::new(...).output()` 同步阻塞，**无超时包装**。
- 一旦 `claude update` / `codex update` / `npm i -g` 因网络、brew 文件锁、DNS 挂起 → 永远 await → 前端按钮卡"升级中…"（不是"无反应"，是"卡住"）
- 但本机实测三条命令都秒级返回（见下），所以这是**潜在风险**而非当前观测症状

**实测（本机，stdin=/dev/null，alarm 10s）**：
| 命令 | 耗时 | exit | 输出摘要 |
|---|---|---|---|
| `claude update` | ~0.5s | 0 | `Claude Code is up to date (2.1.204)` |
| `codex update` | ~3s | 0 | `Updating Codex via 'brew upgrade --cask codex'... 🎉 Update ran successfully!` |
| `npm i -g @anthropic-ai/claude-code@latest` | ~5s | 0 | `added 2 packages in 5s` |

注意 `codex update` 内部会触发 `brew upgrade --cask codex`（要联网 + brew 锁），是三命令里最慢且最易挂的。

### 第三：async fn + blocking I/O 占用 tokio worker thread

`cli_upgrade` 标 `pub async fn`（cli_env.rs:350）但 body 全是阻塞 `.output()`。tokio 默认多线程 runtime 不会因单任务阻塞而死锁，但：
- 若并发触发多个 sync-blocking async 命令，worker pool 饱和 → 整体 IPC 延迟上升
- 正确写法应是 `tokio::process::Command::output().await` 或 `tokio::task::spawn_blocking` 包裹 `std::process::Command`

### 不成立：交互式 TTY / stdin 阻塞

- Rust `Command::output()` 文档明确：**stdin 不继承父进程，子进程读 stdin 立即收到 EOF**（Rust std lib `std::process::Command::output`），不会因 stdin 阻塞
- 实测 `claude update` / `codex update` 在 `< /dev/null` 下都正常 exit 0
- 即两 CLI 的 update 子命令均为非交互式友好，**无需** 额外 `--non-interactive` / `--yes` flag

### 不成立：新 command 未注册（dev 模式）

- `startup.rs:234-237` 已注册全部 4 个 cli_env 命令
- 若用户 `yarn tauri dev` 未重启（见 memory `tauri-rust-command-needs-restart`），invoke 会 reject → 走 catch 分支 setCliErr → 红色 toast，**不是**"无反应"
- 故排除

### 不成立：PATH 找不到 claude/codex

- `gateway/skills/env.rs:23-31` 启动期 `ensure_runtime_path()` 已并入登录 shell PATH（`app_setup.rs:22` 调用）
- 即便 spawn 找不到 binary，`Command::output()` 返 Err → 走 npm 兜底（claude）或 npm 重装（codex），最终返 Ok 或 Err，**不会静默**

## Files Found

| File Path | Description |
|---|---|
| `src/pages/About.tsx:39-43` | mount 期自动调 `handleCliCheck()`，设置 cliBusy="check" |
| `src/pages/About.tsx:89-101` | handleCliCheck：setCliBusy("check") → invoke checkVersions → finally 还原 |
| `src/pages/About.tsx:120-135` | handleCliUpgrade：setCliBusy("upgrade") → invoke upgrade → 成功后串调 handleCliCheck |
| `src/pages/About.tsx:370, 382, 394` | 升级/修复按钮 disabled = `cliBusy !== "" \|\| isPending` |
| `src/services/api/system.ts:56` | `upgrade: invoke<void>("cli_upgrade", { tool })` 绑定正确 |
| `src-tauri/src/commands/cli_env.rs:282-307` | `cli_check_versions` 同步串行 spawn（10+ 次） |
| `src-tauri/src/commands/cli_env.rs:230-267` | `enumerate_installations` 对每条 PATH 跑 `--version`（cli_env.rs:242-256） |
| `src-tauri/src/commands/cli_env.rs:348-401` | `cli_upgrade` async 但内全 blocking `.output()`，无超时 |
| `src-tauri/src/commands/cli_env.rs:439-457` | `run_and_check` 同样 `.output()` 阻塞无超时 |
| `src-tauri/src/gateway/skills/env.rs:23-31` | `ensure_runtime_path` 启动已修复 GUI 极简 PATH 问题 |
| `src-tauri/src/startup.rs:234-237` | 4 个 cli_env 命令均已注册 |
| `src-tauri/src/app_setup.rs:22` | 启动调 `ensure_runtime_path()` |

## 外部行为证据（实测）

```
$ which -a claude            # 2 路径 → 触发 enumerate_installations
/Users/luoxin/.local/share/mise/installs/node/latest/bin/claude
/Users/luoxin/.local/bin/claude

$ which -a codex             # 1 路径 → 不触发 enumerate
/opt/homebrew/bin/codex

$ claude update < /dev/null  # exit 0, ~0.5s, 非交互友好
Current version: 2.1.204
Claude Code is up to date (2.1.204)

$ codex update < /dev/null   # exit 0, ~3s, 内部调 brew upgrade --cask codex
Updating Codex via `brew upgrade --cask codex`...
🎉 Update ran successfully! Please restart Codex.

$ npm i -g @anthropic-ai/claude-code@latest < /dev/null   # exit 0, ~5s
added 2 packages in 5s
```

## Rust std 行为引用

- `std::process::Command::output()`：**stdin 不继承父进程，子进程读 stdin 立即收 EOF**；stdout/stderr 被捕获。即子进程不会因 stdin 阻塞。
- `Command::output()` **无内置超时**，需调用方用 `tokio::time::timeout` 或独立 thread + `wait_timeout` 包裹。

## 修复方向（仅建议，本任务不改码）

按性价比从高到低：

1. **解耦 mount 自动检查与按钮禁用态**（治"无反应"症状）
   - 引入独立 `checkBusy` state，仅"检查版本/诊断冲突"按钮用 `checkBusy`，升级/安装按钮不依赖它
   - 或：把 mount 自动检查改为只跑一次轻量 `probe_version`（不跑 enumerate_installations），conflict 探测改为按需触发
   - 预期收益：升级按钮立即可点，不再受初始检查拖累

2. **`cli_upgrade` / `run_and_check` 加超时**
   - 用 `tokio::time::timeout(Duration::from_secs(60), async { tokio::process::Command::new(..).output().await })` 替换 `std::process::Command::output()`
   - 或在专用 thread 上 `spawn_blocking` + `wait_timeout`（cross-platform 需自己实现或引 crate）
   - 即便子命令真挂，前端也保证在超时阈值内拿到 Err → 显示 toast，不卡"升级中…"

3. **`cli_check_versions` 增量返回 / 缓存**
   - 单工具并发 `tokio::join!` 探测两个工具，而非串行 `.map().collect()`
   - 复用 `ENV_CACHE` 模式做 `OnceLock<Vec<CliToolStatus>>` 会话缓存（首屏 0 spawn）
   - 预期收益：mount 检查从 1-2s 降到 <300ms

4. **`async fn` 全部改用 `tokio::process`** 或 **全部改 sync `pub fn`** 让 Tauri 自动放 blocking pool
   - 当前混用 async 签名 + 同步阻塞 body 是反模式，易在并发场景拖累 runtime

5. **`codex update` 内部 brew 锁兜底**：`codex update` 失败时回退 npm 重装路径已有（cli_env.rs:389-397），保留即可；但若 brew 卡死（非失败），output() 仍会等。需配合修复方向 2 才有效。

## Caveats / 未决项

- **症状归因依赖用户使用时机**：若用户进入 About 后**等 2-3 秒**再点升级，根因 A（disabled 窗口）就不再成立；需用户复现确认点击时序。**建议**：main 向用户追问「点击时是按钮完全不响应（无视觉变化），还是变成"升级中…"后一直不返回」以区分根因 A vs B。
- 未在真实 Tauri dev 运行环境实测 IPC 时延（仅静态代码分析 + 裸 CLI 命令实测）。
- `enumerate_installations` 中 `canonicalize_path` 在某些 symlink 循环下可能阻塞，未深查。
- Windows 分支未测（本机为 macOS）。
