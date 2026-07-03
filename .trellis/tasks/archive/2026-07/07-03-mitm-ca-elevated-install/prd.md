# mitm-ca-elevated-install — MITM CA 安装主动提权（三 OS）

- **Status**: planning
- **Source**: session:claude_96a0dd46-757b-40db-a9f7-4555767078d3
- **Spec**: `.trellis/spec/backend/proxy-connect-relay.md`（MITM 段）
- **Research**: `research/elevation-feasibility.md`（tauri-plugin-shell 提权配置，已完成）

---

## 根因（research 实证，多处）

1. **capability 用错键名 `command` 应 `cmd`**（research #0，最大发现）— `tauri-plugin-shell-2.3.5/src/scope_entry.rs:27` `#[serde(rename = "cmd")]`，`EntryRaw` 无 `deny_unknown_fields` → JSON 写 `"command"` 被静默丢弃 → scope 解析失败 → `ProgramNotAllowed` → 前端兜底弹窗。这与 MitmConfig.tsx:77 注释「capability 拒绝」现状吻合（实际是 scope 解析失败非 sudo 取消）。`cargo build` 不报错（build 期不反序列化 scope，运行时 lazy 解析）。
2. **plugin-shell execute 不提权** — 即使 `cmd` 键修对，`security add-trusted-cert -k /Library/Keychains/System.keychain` 需 root，普通用户执行 exit≠0 → 兜底弹窗。
3. **兜底展示命名名** — MitmConfig.tsx:234-236 展示 `{name} {args}` = `macos-trust-ca ...`（capability 命名 key 非 shell 命令）→ 零背景用户复制到 zsh → `command not found`。

## 用户决策（已答）

- **范围**：三 OS 同步实装（macOS osascript / Windows UAC / Linux pkexec）
- **兜底展示**：真实 sudo 命令（终端可直接执行）
- **UX**：视所有用户零技术背景，权限问题主动提权自动安装

## 方案（research 总结，三 OS 提权 + cmd 键修正）

### 1. capability `mitm-ca.json` 完整重构
- 所有 `"command"` 键 → `"cmd"`（修 research #0 bug）
- 三 OS trust/untrust 命令加 OS 原生提权包装：
  - **macOS**: `cmd: /usr/bin/osascript`，args `["-e", "do shell script \"...security add-trusted-cert...\" with administrator privileges"]`，validator 锁完整 AppleScript 串（含 `\\.keychain /.+\\.pem`）
  - **Windows**: `cmd: C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe`，args `["-Command", "$p = Start-Process -FilePath certutil -ArgumentList '-addstore','-f','Root','<pem>' -Verb RunAs -Wait -PassThru; exit $p.ExitCode"]`（**-PassThru + exit $p.ExitCode** 传播提权进程 exit code，research Q3）
  - **Linux**: `cmd: /usr/bin/pkexec`，args `["/bin/sh", "-c", {validator, raw:true}]`（原 cp/rm 串不变，前置 pkexec）
- 完整 JSON 片段见 research `总结推荐方案` 段

### 2. ca.rs trust_ca_command / untrust_ca_command 三 OS 分支改返提权命令
- macOS 返 osascript + AppleScript 单串（内层 `\"` 转义）
- Windows 返 powershell + `$p = Start-Process ... -PassThru; exit $p.ExitCode` 串
- Linux 返 pkexec + /bin/sh -c（原串不变）
- untrust 三 OS 同理包进提权 wrapper

### 3. commands/mitm.rs CaCommandSpec 加 manual_display 字段
- 新增 `manual_display: String` — 兜底弹窗展示的**真实 sudo 命令**（如 `sudo security add-trusted-cert ...` / `sudo cp ...`），非命名名+args
- trust_command_spec / untrust_command_spec 同步产出 manual_display（三 OS 各构造真实终端命令）

### 4. 前端 MitmConfig.tsx
- 兜底弹窗（L234-236）展示 `manual_display`（真实 sudo 命令）非 `name + args`
- 加 `classifyTrustError(os, code, stderr)`（research Q5）区分 4 类错误，对应本地化文案：
  - cancel（用户取消密码框）：macOS stderr 含 `(-128)` / Windows 含 `1223` / Linux code=126
  - auth_fail（密码错/鉴权拒绝）：macOS stderr 含 `Authorization` / Linux code=127（非 agent）
  - no_agent（Linux 无 polkit agent）：code=127 + stderr 含 `agent`/`polkit`
  - cmd_fail（命令本身失败）：其他非 0
- 修注释 L6/63（删「sudo 弹窗由 OS 触发」错误假设，改「osascript/pkexec/UAC 提权」）

### 5. api.ts CaCommandSpec TS 加 manual_display；locales ×8 加错误分类文案

## 改动文件

| 文件 | 改动 |
|---|---|
| `src-tauri/capabilities/mitm-ca.json` | 全部 `command`→`cmd` + 三 OS trust/untrust 加提权包装（osascript/powershell/pkexec）+ validator 重锁 |
| `src-tauri/src/gateway/mitm/ca.rs` | trust_ca_command / untrust_ca_command 三 OS 分支改返提权命令；新增 manual_display 真实命令构造 |
| `src-tauri/src/commands/mitm.rs` | CaCommandSpec 加 manual_display；trust/untrust_command_spec 同步产出；命名 key 不变 |
| `src/components/settings/MitmConfig.tsx` | 兜底展示 manual_display；classifyTrustError；修注释 |
| `src/services/api.ts` | CaCommandSpec TS 加 manual_display |
| `src/locales/*.json` ×8 | 错误分类文案（cancel/auth_fail/no_agent/cmd_fail）+ 兜底 hint |

## 验证

```bash
cd src-tauri
cargo test ca_trust_command_returns_os_specific -- --nocapture  # 改后断言提权命令（osascript/powershell/pkexec）
cargo test ca_cleanup_untrust_current_os -- --nocapture
cargo test ca_linux_capability_validator_matches_commands -- --nocapture  # validator 重锁
cargo clippy --all-targets --no-deps  # 0 src warning
cargo build
yarn build && node scripts/check-i18n.mjs
# macOS 实跑（人工）：MITM 启用 → 装 CA → 系统密码框弹 → 输密码 → exit=0 → ca_installed=true
# macOS 取消（人工）：密码框点取消 → stderr 含 (-128) → toast「已取消」非「失败」
```

## 需人工实测（research 标注未实测，本 task 仅 macOS 可测）

- Windows `-PassThru -Verb RunAs` exit 传播（pwsh 未装）—— 代码层按 research 落地，Win 实机 follow-up
- macOS validator `\\\"` 三层转义与 ca.rs args[1] 产出镜像 —— 加 cargo test 字面锁（扩 ca_linux_capability_validator 模式加 macos 分支）
- Linux pkexec GNOME 实机弹框 —— 代码层落地，Linux 实机 follow-up

## 不做

- 自建 GUI polkit agent（YAGNI，纯 WM 用户占比极小 + 兜底弹窗可用）
- mmdb / GeoIP（无关）
- ST3 TLS 解密隧道本体（已归档）

## 调度

单 task，拆 2 subtask 并行（文件集不相交）：
- S1 Rust：mitm-ca.json + ca.rs + commands/mitm.rs（capability 重构 + 提权命令 + manual_display）
- S2 前端：MitmConfig.tsx + api.ts + locales（兜底展示 + classifyTrustError + 文案）

与 mitm-whitelist-clash-rules 文件集不相交（已 finish），可独占 worktree。
