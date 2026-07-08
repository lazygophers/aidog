# Research: aidog (Tauri) 执行 shell 命令的可行性

- **Query**: aidog 现有 shell 执行基础设施能否支撑「检测 / 安装 / 升级 CLI」
- **Scope**: internal（aidog 仓库代码 + Cargo.toml + capabilities）
- **Date**: 2026-07-08

## Findings

### 已用的外部命令调用方式

aidog 仓库内 `Command::new` 共 11 处命中（含测试），分三类：

| 模式 | 文件 | 用途 |
|---|---|---|
| **后端直接 `std::process::Command`**（主路径） | `commands/script_executor.rs:51` | `install_uv`：`sh -c "curl -LsSf https://astral.sh/uv/install.sh \| sh"` |
| | `shared.rs:157` | 检测 `uv` 二进制 |
| | `gateway/import_export/skills_sync.rs:141` | spawn `npx` 同步 skills |
| | `gateway/notification/tts.rs:47/91/110/116/128` | macOS `say` / `afplay`、Windows `powershell`、Linux `paplay` |
| | `gateway/mitm/ca.rs:361/376/386/1191/1214` | macOS `/usr/bin/security` / `certutil` / `/bin/sh` / `/usr/bin/osacompile`（CA 装信任库探测，非执行） |
| | `gateway/skills/env.rs:38/80/89` | 探测 `$SHELL -ilc 'echo $PATH'` / `node --version` / `npx --version` |
| | `gateway/skills/catalog.rs:65/217`, `npx.rs:50/103`, `test_*.rs` | spawn `npx` 跑 skills CLI |
| **tauri-plugin-shell v2**（已装） | `startup.rs:26` | `.plugin(tauri_plugin_shell::init())` |
| **前端 `@tauri-apps/plugin-shell`**（已用） | `components/settings/MitmConfig.tsx:16,78` | `Command.create(spec.name, spec.args).execute()` 装 CA |

**结论**：aidog 同时具备两条路径——后端 Rust 直接 spawn（主路径）+ 前端 invoke plugin-shell（capability 限定）。两者都在用，模式成熟。

### Cargo.toml 依赖确认

```
src-tauri/Cargo.toml:
  tauri-plugin-process = "2"
  tauri-plugin-shell = "2"
```

`tauri-plugin-shell` **已装并 init**（`startup.rs:26`），无需新增依赖。

### capabilities 配置

`src-tauri/capabilities/` 下两个文件：

- **`default.json`**：主窗口 + popover 的通用权限。`process:default` + `process:allow-restart`（仅重启），**未开放 shell:allow-execute**（注释明确：`scope 在 capabilities/mitm-ca.json 限定仅装 CA 命令，默认 capability 不开放 shell`）。
- **`mitm-ca.json`**：单独 capability，`shell:allow-execute` + 5 个命名命令（macos-trust-ca / macos-remove-ca / windows-trust-ca / windows-remove-ca / linux-shell-ca），每个命令的 args 用 **regex validator** 锁死（如 `do shell script "/usr/bin/security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System\\.keychain /.+\\.pem" with administrator privileges`）。注释强调：

> Key must be `cmd` (NOT `command` — tauri-plugin-shell-2.3.5 #[serde(rename="cmd")] silently drops `command` → ProgramNotAllowed).

### mitm.rs install_ca_prepare 的模式（参考实现）

`commands/mitm.rs:140` `mitm_install_ca_prepare` 是 aidog 现有「后端准备命令 spec → 前端执行」的范本：

1. 后端写文件到 `~/.aidog/mitm-ca.pem`（数据目录）
2. 后端返 `CaCommandSpec { name, args, ca_pem_path, manual_display }`：
   - `name`：capability `mitm-ca.json` 里的命名命令 key（按 OS 选 `macos-trust-ca` / `windows-trust-ca` / `linux-shell-ca`）
   - `args`：已含路径 + 提权 wrapper（macOS osascript admin / Windows Start-Process RunAs / Linux pkexec）
   - `manual_display`：兜底手动装的 sudo 终端命令（提权失败时前端弹窗给用户复制）
3. 前端 `Command.create(spec.name, spec.args).execute()` 触发 OS 原生提权弹窗
4. 前端按 exit code 调 `mitm_set_ca_installed(true/false)` 落账

**关键**：mitm-ca 的命令需要 sudo（写 `/Library/Keychains/System.keychain`），所以必须走 plugin-shell + capability（OS 原生提权弹窗）。**CLI 安装 / 升级不需要 sudo**（npm 全局装在用户 home，curl install.sh 装到 `~/.local`），可以走后端直接 spawn（更简单、更安全、不需要扩 capability）。

### `gateway/skills/env.rs` 的 PATH 修复（核心痛点已解）

`ensure_runtime_path()`（env.rs:23-31）已经解决 cc-switch 反复强调的「GUI 进程 PATH 与登录 shell PATH 不对称」根因：

```rust
pub fn ensure_runtime_path() {
    PATH_FIXED.get_or_init(|| {
        if let Some(merged) = probe_login_path() {
            std::env::set_var("PATH", &merged);
        }
    });
}
```

`probe_login_path`（env.rs:36-52）spawn `$SHELL -ilc 'echo $PATH'`，合并登录 PATH 在前。这是 OnceLock 幂等守卫，启动时调一次即可，覆盖全部后续子进程。

**aidog 启动时已经调用过 `ensure_runtime_path`**（推测：skills 子系统依赖它）。新加的 CLI 检测 / 升级命令**自动受益**，无需重复修 PATH。

### `resolve_home_env` 注入（claude 检测必备）

env.rs:115-123 `resolve_home_env()` 用 `dirs::home_dir()` 解析 home，比继承父 env 可靠（launchd 极简 env 下 HOME 可能缺失）。`apply_home_env(cmd)` 把 HOME + CLAUDE_CONFIG_DIR 注入子进程。**这是检测 claude 时必须做的**——claude-code agent 检测依赖 `claudeHome = CLAUDE_CONFIG_DIR || ~/.claude`，只看 HOME env 无 getpwuid 兜底。

### `script_executor.rs::install_uv` 的成熟模式（直接可抄）

```rust
#[cfg(unix)]
{
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg("curl -LsSf https://astral.sh/uv/install.sh | sh")
        .output()
        .map_err(|e| format!("spawn uv installer: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("uv install failed: {}", stderr.trim()));
    }
    // ...
}
#[cfg(not(unix))]
{
    Err("auto-install uv is only supported on Unix; please install uv manually".to_string())
}
```

**这就是「安装 CLI 工具」的成熟模板**：`sh -c "<curl install.sh>"` + `.output()` 阻塞到结束 + 按 exit code 返 Result。换成 claude / codex 只需替换命令字符串。

### 执行 `npm install -g` 需要的权限

- **macOS / Linux**：npm 全局装到 `~/.npm-global` / `~/.local/bin` / `/opt/homebrew/lib/node_modules` / nvm 路径，**全在用户可写目录，不需要 sudo**。Homebrew node 的 npm 全局目录 `/opt/homebrew/lib/node_modules` 也对 admin group 可写（macOS 默认）。
- **Windows**：装到 `%APPDATA%\npm`（用户目录），无 admin 需求。
- **PATH 问题**：见上面 `ensure_runtime_path`，已解。
- **curl 网络问题**：aidog 已有 `gateway/http_client.rs` 共享 reqwest client（含代理配置），但 shell 命令里的 curl 走系统网络栈，不自动用应用代理——这可能是问题（用户开代理时 install.sh 失败）。可选：检测代理环境变量注入子进程 `HTTPS_PROXY`。

### 前端能否直接 invoke shell？

**两种方式都可行**：

| 方式 | 优势 | 劣势 | aidog 适配度 |
|---|---|---|---|
| 后端 Rust command 包裹（推荐） | 命令字符串不暴露给前端，无注入面；可任意复杂逻辑；不依赖 capability | 必须新增 Tauri command | **强**（已有 `install_uv` / `mitm_*_prepare` 模式） |
| 前端 plugin-shell + capability | OS 原生提权弹窗（需要 sudo 时必选） | 命令必须能被 regex validator 描述；每加一个命令要改 capability JSON | 中（CLI 安装不需要 sudo，没价值） |

**结论：走后端 Rust command 包裹**，完全不动 capability，复用 `install_uv` 模板。前端只 `invoke('install_claude')` / `invoke('upgrade_codex')`。

### 唯一需要 plugin-shell 的场景

**安装 CA 信任库**（已实现）—— 写 `/Library/Keychains/System.keychain` 必须提权。CLI 安装 / 升级**不需要提权**，禁混用模式。

## 关键结论（5 条）

1. **基础设施全具备**：`tauri-plugin-shell` 已装但用不上；`std::process::Command` 已在 8 个文件用；`install_uv` 就是 CLI 安装的成熟模板。
2. **PATH 不对称痛点已解**：`gateway/skills/env.rs::ensure_runtime_path` 在启动时已合并登录 shell PATH，新加的子进程自动受益。
3. **HOME 注入已有 helper**：`resolve_home_env` / `apply_home_env` 直接复用，claude 检测必备。
4. **不需要扩 capability**：CLI 安装走后端 Rust command 包裹（非 plugin-shell），不动 `capabilities/mitm-ca.json` / `default.json`。
5. **Windows 抑制窗口的模式没现成**：`CREATE_NO_WINDOW = 0x08000000` 在 aidog 现有代码里**未实现**（mitm-ca 走 plugin-shell，由 plugin 处理）；如果 Windows 要在后端直接 spawn（如 `cmd /C npm i -g ...`），需要抄 cc-switch 的 `creation_flags(CREATE_NO_WINDOW)`，否则会闪现控制台窗口。

## 对 aidog PRD 的建议

- **走后端 Rust command 模式**（抄 `install_uv`），新增 commands/cli_lifecycle.rs：
  - `get_cli_versions()` → Vec<{name, version, latest_version, installed_but_broken}>
  - `install_cli(tool)` → Result<(), String>（spawn install.sh / npm i -g）
  - `upgrade_cli(tool)` → Result<(), String>（spawn `<bin> update` / npm i -g）
  - `probe_cli_installations(tool)` → Vec<{path, version, source, is_path_default}>（冲突诊断）
- **启动时复用 `ensure_runtime_path`**：新代码无需重复修 PATH，但**必须确认 `ensure_runtime_path` 真在启动时调用了**（推测：skills 子系统入口调用，需 grep 确认 `ensure_runtime_path()` 的调用点；若没在启动早期调，CLI 检测会撞 PATH 缺失）。
- **claude / codex HOME 注入**：复用 `gateway::skills::env::resolve_home_env` / `apply_home_env`，别重复造。
- **Windows CREATE_NO_WINDOW**：新增 `commands/cli_lifecycle.rs` 里 Windows 分支必须加 `creation_flags(CREATE_NO_WINDOW)`，否则 `cmd /C npm i -g` 会闪现黑色控制台窗口（cc-switch misc.rs:240 模式）。
- **代理透传**：检测 `~/.aidog/settings.json` 里的代理配置，注入子进程 `HTTPS_PROXY` / `HTTP_PROXY` env（避免用户开代理时 curl install.sh 失败）。
- **不动 capability**：CLI 命令字符串全在后端，前端只 invoke command name + tool name，无命令注入面。
