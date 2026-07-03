# Research: tauri-plugin-shell scoped command 提权配置可行性

- **Query**: tauri-plugin-shell v2 的 capability scoped command 能否配置成「提权执行 OS 信任库命令」，覆盖 macOS / Windows / Linux 三平台
- **Scope**: external（官方文档 + 插件源码 + 实测） + internal（现有 capability 验证）
- **Date**: 2026-07-03
- **Plugin version**: `tauri-plugin-shell = "2.3.5"`（`src-tauri/Cargo.lock:5322-5323`，Tauri `2.11.2`）

---

## 总览结论（一句话）

**条件可行**。三 OS 都能用 plugin-shell scoped command 触发提权（osascript / Start-Process RunAs / pkexec），plugin-shell 对 `cmd` 路径无白名单限制，validator 可锁任意正则。**但当前 `mitm-ca.json` 存在潜在 bug：用了 `command` 键，插件源码要求 `cmd`** —— 见下方「关键发现 #0」。

---

## 关键发现（影响 PRD 落地）

### #0 ⚠️ 现有 capability 用错键名 `command`，应为 `cmd`（潜在 bug）

**源码依据**（vendored 2.3.5）：

`/Users/luoxin/.cargo/registry/src/rsproxy.cn-e3de039b2554c837/tauri-plugin-shell-2.3.5/src/scope_entry.rs:19-29`：
```rust
#[derive(Deserialize)]
pub(crate) struct EntryRaw {
    pub(crate) name: String,
    #[serde(rename = "cmd")]          // ← JSON 键必须是 "cmd"
    pub(crate) command: Option<PathBuf>,
    #[serde(default)]
    pub(crate) args: ShellAllowedArgs,
    #[serde(default)]
    pub(crate) sidecar: bool,
}
```

`EntryRaw` **无 `deny_unknown_fields`**，故 JSON 写 `"command": "..."` 时：
- serde 把 `"command"` 当未知字段**静默丢弃**
- `#[serde(rename="cmd")] command` 字段因 JSON 无 `cmd` 键 → `None`
- `Entry::deserialize` (scope_entry.rs:38-42) 返错 `The shell scope \`command\` value is required.`
- `ScopeAllowedCommand::deserialize` (scope.rs:67) 把该错包成 `Error` → `commands::execute` 返 `ProgramNotAllowed`

**为什么 `cargo build` 不报错**：`tauri_build::build()` 只校验 capability 顶层结构（identifier / permissions / windows），**不反序列化每个 permission 的 scope**（scope 是 plugin-specific serde 类型，build 期不知）。`src-tauri/gen/schemas/capabilities.json` 把 `command` 原样透传 → 运行时 `ScopeObject::deserialize` 才 lazily 解析 → 首次 invoke `Command.create("macos-trust-ca", ...).execute()` 时才报错。

**这与现状吻合**：`MitmConfig.tsx:77` 注释「Command.create reject（capability 拒绝 / 用户取消 sudo）→ 兜底手动装」，实际是 capability scope 解析失败而非 sudo 取消。

**JSON schema 也确认**：`src-tauri/gen/schemas/desktop-schema.json` 中 `ShellScopeEntry` 定义 `"required": ["cmd", "name"]`，`cmd` 描述 `"The command name. It can start with a variable that resolves to a system base directory."`。

**结论 / 落地动作**：新提权 capability **必须用 `cmd` 键**（不是 `command`）。PRD 改 `mitm-ca.json` 时同步修此 bug。官方文档示例（https://v2.tauri.app/plugin/shell/ Permissions 段）也用 `"cmd": "sh"`。

---

## 5 个未知点逐条回答

### Q1. command 字段能否设任意二进制（osascript / powershell / pkexec）？

**结论：可行，无任何白名单 / 格式限制。**

**源码依据**（vendored 2.3.5）：

`scope.rs:48-52`（`ScopeAllowedCommand`）：
```rust
pub struct ScopeAllowedCommand {
    pub name: String,
    pub command: std::path::PathBuf,   // ← 类型 PathBuf，任意路径
    pub args: Option<Vec<ScopeAllowedArg>>,
    pub sidecar: bool,
}
```

`scope.rs:67-86`（`ScopeObject::deserialize`）：
```rust
let command = if let Ok(path) = app.path().parse(&scope.command) {
    path                                    // 尝试解析 $HOME / $APPDATA 等变量前缀
} else {
    scope.command.clone()                   // ← 解析失败直接用原字符串当路径
};
```
`app.path().parse` 只做 base-dir 变量替换（`$HOME` / `$APPDATA` 等，见 desktop-schema.json `cmd` 描述列出的 24 个变量），**不做白名单校验**。绝对路径（`/usr/bin/osascript`、`/usr/bin/pkexec`、`C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe`）解析失败走 else 分支原样用 → `std::process::Command::new(path)`。

**官方文档佐证**：https://v2.tauri.app/plugin/shell/ Permissions 段示例 `"cmd": "sh"`，`https://v2.tauri.app/security/scope/` 说明 scope 类型 plugin-specific、由 plugin 自行 enforce；shell plugin 的 enforce 仅在 args validator 层（见 Q2），不在 command 路径层。

**三 OS 推荐路径**：

| OS | cmd 值 | 说明 |
|---|---|---|
| macOS | `/usr/bin/osascript` | 系统自带，绝对路径稳定 |
| Windows | `powershell.exe` 或 `C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe` | 用绝对路径防 PATH 投射；Windows 安全机制下绝对路径更稳 |
| Linux | `/usr/bin/pkexec` | Debian/Ubuntu 默认装 policykit-1 时在此；Fedora/Arch 同路径。fallback `/usr/bin/pkexec` 解析失败仍原样用 |

**Caveat / 不确定**：`app.path().parse` 对含空格或特殊字符的路径（Windows `Program Files`）是否误解析变量前缀 —— 推测：`parse` 仅识别 `$VAR/` 前缀，普通绝对路径不受影响。建议人工实测：capability 配 `cmd: "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"` 跑一次 `Command.create().execute()`。

---

### Q2. validator 如何锁含动态 pem 路径 + 引号转义的复杂脚本串？

**结论：可行。用 `{ "validator": "<regex>", "raw": false }` 默认模式，validator 字符串会被自动包 `^...$`；或 `raw: true` 完全自定义。**

**源码依据**（vendored 2.3.5）：

`scope.rs:73-80`（`ShellAllowedArg::Var` 反序列化）：
```rust
crate::scope_entry::ShellAllowedArg::Var { validator, raw } => {
    let regex = if raw {
        validator                            // ← raw:true  原样用，不锚定
    } else {
        format!("^{validator}$")             // ← raw:false 自动 ^...$ 锚定
    };
    let validator = Regex::new(&regex)
        .unwrap_or_else(|e| panic!("invalid regex {regex}: {e}"));
    crate::scope::ScopeAllowedArg::Var { validator }
}
```

**匹配语义**（`scope.rs` `ShellScope::_prepare` 用 `validator.is_match(&value)`）：
- Rust `regex` crate（RE2 语法）的 `is_match` 默认**搜索任意位置**，不锚定
- `raw: false`（默认）→ 自动 `^...$` 全串匹配
- `raw: true` → 不锚定，validator 自己写 `^...$` 或用 `(?s)^...$`

**关键约束**：
1. validator 是 **Rust regex crate** 语法（RE2，**不支持回溯 / lookahead**）。JSON 字符串里反斜杠要双重转义：`\\.` `\\$` `\\\"` 等。
2. `ShellAllowedArg` enum 有 `#[serde(untagged, deny_unknown_fields)]`（scope_entry.rs:67-77）→ **只允许 `validator` + 可选 `raw` 两个键**，多写一个键整个 capability 解析失败。
3. validator 锁的是**单 arg 字符串**（一个 args 数组位置的值），不是整条命令行。

#### 三 OS 可落地的 capability JSON 片段（含 validator）

##### macOS（osascript 单 `-e` 参数，内含动态 pem 绝对路径 + 引号转义）

osascript 的 `-e` 参数是一整段 AppleScript：
```
do shell script "/usr/bin/security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain <pem>" with administrator privileges
```
内层双引号在 AppleScript 里用 `\"` 转义，最终作为**单个 argv 字符串**传给 osascript（plugin-shell 走 `std::process::Command::arg`，不做 shell 解析，引号原样透传）。

```json
{
  "name": "macos-trust-ca",
  "cmd": "/usr/bin/osascript",
  "args": [
    "-e",
    {
      "validator": "do shell script \"/usr/bin/security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System\\.keychain /.+\\.pem\" with administrator privileges"
    }
  ]
}
```
> validator 默认 `raw: false` → 实际 regex = `^do shell script "...System\.keychain /.+\.pem" with administrator privileges$`。`/.+\.pem` 锁绝对路径 pem 文件。AppleScript 内 `\"` 在 JSON 里写 `\\\"`（JSON 转义 + AppleScript 转义两层），见下方「验证」段示例。

**实测支撑**（macOS 本机，2026-07-03）：
```bash
$ osascript -e 'do shell script "echo \"hello world\""'
hello world                              # ← -e 单 arg 含 \" 转义 OK
$ osascript -e 'do shell script "exit 42"'
0:25: execution error: 该命令退出时状态为非零。 (42)
$ echo $?
1                                         # ← osascript exit 恒 1，原 exit code 在括号
```
→ AppleScript 单 arg 引号转义路径走通。

##### Windows（powershell `-Command` 单参数，ArgumentList 含动态 pem）

```json
{
  "name": "windows-trust-ca",
  "cmd": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
  "args": [
    "-Command",
    {
      "validator": "Start-Process -FilePath certutil -ArgumentList '-addstore','-f','Root','/.+\\.pem' -Verb RunAs -Wait -PassThru"
    }
  ]
}
```
> PowerShell 单引号串内不转义；pem 路径用 `/.+\.pem`（Windows 也能用绝对 Unix 风格路径，但更稳用 `[A-Z]:\\\\.+\\.pem` —— 注意 JSON 双反斜杠 + regex 转义）。建议：`[A-Za-z]:\\\\.+\\.(pem|crt)`。

##### Linux（pkexec 两固定 arg + 一动态 `-c` 脚本串）

```json
{
  "name": "linux-shell-ca",
  "cmd": "/usr/bin/pkexec",
  "args": [
    "/bin/sh",
    "-c",
    {
      "validator": "^cp /.+\\.pem /usr/local/share/ca-certificates/aidog-ca\\.crt && update-ca-certificates$|^rm -f /usr/local/share/ca-certificates/aidog-ca\\.crt && update-ca-certificates --fresh$",
      "raw": true
    }
  ]
}
```
> 这里用 `raw: true` + 显式 `^...$`（与现有 mitm-ca.json 的 linux-shell-ca validator 字面一致，仅 command 改 pkexec，前置 `/bin/sh` ` -c` 两 fixed arg）。raw:true 避免插件再包一层 `^...$` 破坏 `|` union（推测：默认 `^A$|^B$` 包成 `^^A$|^B$$` 会出错；raw:true 最稳）。

**Caveat**：
- macOS validator 里的 `\"` 在 JSON 字符串中要写 `\\\"`（JSON 转义 `\` → `\\`，再加 AppleScript 的 `"`）—— 容易写错，建议人工跑一次 `Command.create("macos-trust-ca", ["-e", "..."]).execute()` 确认 validator 接受实际产出的 arg 串。
- ca.rs::trust_ca_command 的 macOS 分支产出的 args[1] 是 `do shell script \"...<pem>...\" with administrator privileges`（含字面 `\"`），validator 必须与该字面**完全镜像**（现有 ca.rs Linux validator 测试 ca_linux_capability_validator_matches_commands 已示范双向锁模式，ca.rs:617-647）。

---

### Q3. Windows `Start-Process -Verb RunAs -Wait` 能否回传被提权进程的 exit code？

**结论：可行，但必须加 `-PassThru`，且需 try/catch 兜 UAC 取消异常。**

**依据**：

1. **`-PassThru`** 才返回 `System.Diagnostics.Process` 对象，其 `.ExitCode` 属性在 `-Wait` 结束后反映被启动进程的 exit code。Microsoft Learn `Process.ExitCode` 文档（https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.process.exitcode）：*"Gets the value that the associated process specified when it terminated."*

2. **`-Verb RunAs`** 走 ShellExecute 提权（弹 UAC）。UAC 通过后，新进程是独立 elevated 进程；`-PassThru -Wait` 拿到它的 Process 对象 + 等它退出 + 读 `.ExitCode`。**不加 `-PassThru`，`Start-Process` 不返回对象，exit code 丢失**，PowerShell 自身 exit code 是 0（除非异常）。

3. **plugin-shell 怎么收到**：plugin-shell `execute` 跑 `powershell.exe -Command "<Start-Process 脚本>"`，`commands.rs:206` `output.status.code()` 拿的是 **powershell.exe 的 exit code**，不是被提权进程的。要让 powershell.exe 的 exit code = 被提权进程 exit code，脚本末尾必须显式传播：
   ```powershell
   $p = Start-Process -FilePath certutil -ArgumentList '-addstore','-f','Root','<pem>' -Verb RunAs -Wait -PassThru
   exit $p.ExitCode
   ```
   `exit $p.ExitCode` 让 powershell.exe 以该 code 退出 → plugin-shell `ChildProcessReturn.code` 拿到。

4. **UAC 用户点「否」**：ShellExecute 返 Win32 error 1223 (`ERROR_CANCELLED`)，PowerShell 抛 terminating error `System.InvalidOperationException` 或 `System.ComponentModel.Win32Exception`，powershell.exe exit code 非 0（通常 1）。stderr 含 `1223`。

**推荐 capability args[1] 字面**（修正 PRD 缺 `-PassThru` + `exit`）：
```
$p = Start-Process -FilePath certutil -ArgumentList '-addstore','-f','Root','<pem>' -Verb RunAs -Wait -PassThru; exit $p.ExitCode
```

**Caveat / 不确定**：
- `-PassThru` 在 `-Verb RunAs` 路径下是否 100% 返回有效 Process 对象 —— 推测可行（ShellExecute 提权后 PowerShell 仍能拿到新进程句柄），但**未在 Windows 实测**。建议人工实测：配 capability，跑 `Command.create("windows-trust-ca", ["-Command", "..."]).execute()`，确认 `code` 字段 = certutil 真实 exit code（成功 0，失败通常 0 或具体码）。
- 若 UAC 取消导致 PowerShell 抛异常而非 exit，plugin-shell `execute` 的 `code` 可能是 1 且 stderr 含异常栈。前端按「code != 0 或 stderr 含 1223」判取消。

---

### Q4. Linux pkexec polkit agent 缺失（纯 WM）时的 fallback？

**结论：pkexec 自带 textual authentication agent 兜底，但 GUI app 无 tty → 仍失败 → exit 127（可检测，可降级）。无需自建 GUI agent（PRD 明确 YAGNI 不做）。**

**依据**（Ubuntu manpage pkexec(1)，https://manpages.ubuntu.com/manpages/jammy/man1/pkexec.1.html）：

> **RETURN VALUE**
> Upon successful completion, the return value is the return value of PROGRAM. If the calling process is not authorized or an authorization could not be obtained through authentication or an error occured, pkexec exits with a return value of **127**. If the authorization could not be obtained because the user dismissed the authentication dialog, pkexec exits with a return value of **126**.

> **AUTHENTICATION AGENT**
> pkexec, like any other PolicyKit application, will use the authentication agent registered for the calling process. However, if no authentication agent is available, then pkexec will register **its own textual authentication agent**. This behavior can be turned off by passing the `--disable-internal-agent` option.

**三场景行为**：

| 环境 | 行为 | exit code |
|---|---|---|
| GNOME/KDE/etc（有 polkit-gnome / polkit-kde-agent） | 桌面环境 agent 弹 GUI 密码框 → 用户输密码 → 成功则跑命令返命令 exit | 命令 exit（成功 0） |
| 纯 WM（i3/sway）无桌面 agent | pkexec 启 internal textual agent，从 stdin 读密码 → plugin-shell execute **不接 tty stdin**（commands.rs CommandOptions 默认 env clear，stdin 是关闭的 pipe） → auth 失败 → **127** | 127 |
| 用户在密码框点「取消」 | agent 返 dismissed → pkexec **126** | 126 |

**降级路径**：plugin-shell `execute` 收到 `code=127` → 前端判「Linux 无 polkit agent / auth 失败」→ 走 MitmConfig.tsx 兜底弹窗（展示真实 sudo 命令，PRD 改动文件表第 5 行）。`code=126` → 「用户取消」。

**自建 GUI polkit agent 的 Rust 方案**（PRD 标 YAGNI 不做，仅记录）：

crates.io 可用 crate：
- `polkit-agent` 0.19.0（high-level libpolkit-agent-1 绑定）
- `polkit-agent-rs` 0.3.0
- `zbus-polkit-agent` 0.4.3（纯 Rust，走 zbus D-Bus，无 C 依赖，最适合嵌入 Tauri GUI）
- `badged` 0.1.0（专为 Linux WM 写的 polkit agent，参考实现）

这些 crate 允许 Tauri 进程内注册 polkit agent，弹自己的密码框（GTK relm 或原生 widget）。但需 D-Bus + polkitd 运行，且增加 ~500KB+ 依赖。PRD 决策不做（L69），合理。

**Caveat**：纯 WM 用户占比极小（推测 < 5% Linux 桌面用户），且这类用户技术水平高，兜底弹窗展示 `sudo cp <pem> ...` 命令可自行执行。YAGNI 成立。

---

### Q5. 用户在密码框点「取消」时，plugin-shell 收到的 exit code / stderr 形态（三 OS）？

**结论：三 OS exit code 各异，stderr 文本含可解析标记区分「取消 vs 密码错 vs 命令失败」。**

#### macOS（osascript admin cancel）

**实测**（macOS 本机，2026-07-03，`osascript -e 'display dialog'` 强制 cancel）：
```
execution error: “System Events”遇到一个错误：用户已取消。 (-128)
```
exit code = **1**。

`do shell script "..." with administrator privileges` 用户在系统密码框点「取消」时：
- AppleScript 抛 error **-128**（`userCanceledErr`）
- osascript stderr 含 `(-128)` + 本地化「用户已取消」/「User canceled」
- osascript exit code **恒为 1**（与命令失败同 exit code，**仅靠 stderr 区分**）

**密码错**：osascript 在密码框停留重试（系统密码框自带重试），3 次后报 `-128` 或 authorization 错，stderr 含 `Authorization` 相关串。
**命令本身失败**（如 security 写 keychain 失败）：stderr 含 `该命令退出时状态为非零。 (N)`，N 是命令真实 exit code。

**前端区分策略**（macOS）：
| stderr 模式 | 含义 |
|---|---|
| 含 `(-128)` | 用户取消 |
| 含 `状态为非零。 (N)` 且 N != 0 | 命令本身失败，N 是真实 exit code |
| 含 `Authorization` / `authorization` | 密码错 / 鉴权拒绝 |
| exit code == 0 | 成功 |

#### Windows（UAC cancel）

**依据**：Win32 `ERROR_CANCELLED = 1223`，ShellExecute `-Verb RunAs` 在用户点「否」时抛此错。

PowerShell `Start-Process -Verb RunAs` 在 UAC 拒绝时：
- 抛 `System.ComponentModel.Win32Exception` 或 `InvalidOperationException`
- stderr 含 `1223` / `操作被用户取消` / `The operation was canceled by the user`
- powershell.exe exit code **1**（异常未 catch）或脚本 `exit` 指定的码

**前端区分策略**（Windows）：
| stderr 模式 | 含义 |
|---|---|
| 含 `1223` / `canceled` | UAC 取消 |
| `Start-Process` 抛异常但无 1223 | 命令本身未启动（路径错等） |
| `$p.ExitCode` 非 0（certutil 真实 exit） | 命令本身失败 |
| exit code == 0 | 成功 |

#### Linux（pkexec cancel / auth fail）

**依据**：pkexec(1) manpage（Q4 引）。

| exit code | 含义 |
|---|---|
| **126** | 用户在密码框点「取消」（dismissed） |
| **127** | auth 失败（密码错超限）/ 无可用 agent / 其他错误 |
| 命令真实 exit code（成功 0） | 鉴权通过且命令执行完 |

stderr 含 polkit 本地化错误串（如 `Not authorized` / `Authentication failed` / `Could not get owner of ...`）。

**前端区分策略**（Linux）：
| code | 含义 |
|---|---|
| 0 | 成功 |
| 126 | 用户取消 |
| 127 | auth 失败 / 无 agent → 兜底手动装 |
| 其他非 0 | 命令本身失败（鉴权通过但 cp/update-ca-certificates 失败） |

**plugin-shell ChildProcessReturn 字段**（commands.rs:194-199）：`{ code: Option<i32>, signal: Option<i32>, stdout, stderr }` —— 前端 `out.code` + `out.stderr` 双判即可。

---

## 总结推荐方案（直接喂 PRD）

### capability `mitm-ca.json` 完整重构（三 OS trust + untrust，含提权 + cmd 键修正）

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "mitm-ca",
  "description": "MITM Root CA install/uninstall via OS-native elevation (osascript admin / Start-Process RunAs / pkexec). All commands user-triggered, scope-locked via regex.",
  "windows": ["main", "popover"],
  "permissions": [
    {
      "identifier": "shell:allow-execute",
      "allow": [
        {
          "name": "macos-trust-ca",
          "cmd": "/usr/bin/osascript",
          "args": [
            "-e",
            { "validator": "do shell script \"/usr/bin/security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System\\.keychain /.+\\.pem\" with administrator privileges" }
          ]
        },
        {
          "name": "macos-remove-ca",
          "cmd": "/usr/bin/osascript",
          "args": [
            "-e",
            { "validator": "do shell script \"/usr/bin/security delete-certificate -Z [0-9A-Fa-f]+\" with administrator privileges" }
          ]
        },
        {
          "name": "windows-trust-ca",
          "cmd": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
          "args": [
            "-Command",
            { "validator": "\\$p = Start-Process -FilePath certutil -ArgumentList '-addstore','-f','Root','[A-Za-z]:\\\\.+\\.(pem|crt)' -Verb RunAs -Wait -PassThru; exit \\$p\\.ExitCode" }
          ]
        },
        {
          "name": "windows-remove-ca",
          "cmd": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
          "args": [
            "-Command",
            { "validator": "\\$p = Start-Process -FilePath certutil -ArgumentList '-delstore','Root','[0-9A-Fa-f]+' -Verb RunAs -Wait -PassThru; exit \\$p\\.ExitCode" }
          ]
        },
        {
          "name": "linux-shell-ca",
          "cmd": "/usr/bin/pkexec",
          "args": [
            "/bin/sh",
            "-c",
            {
              "validator": "^cp /.+\\.pem /usr/local/share/ca-certificates/aidog-ca\\.crt && update-ca-certificates$|^rm -f /usr/local/share/ca-certificates/aidog-ca\\.crt && update-ca-certificates --fresh$",
              "raw": true
            }
          ]
        }
      ]
    }
  ]
}
```

### `ca.rs` `trust_ca_command` / `untrust_ca_command` 三 OS 分支改返提权命令

**macOS trust**（args[1] 是完整 AppleScript 单串，内层 `\"` 转义）：
```rust
(
    "/usr/bin/osascript".to_string(),
    vec![
        "-e".to_string(),
        format!(
            "do shell script \"/usr/bin/security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain {ca_pem_path}\" with administrator privileges"
        ),
    ],
)
```

**Windows trust**（args[1] 是 PowerShell 单串，`exit $p.ExitCode` 传播）：
```rust
(
    r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".to_string(),
    vec![
        "-Command".to_string(),
        format!(
            "$p = Start-Process -FilePath certutil -ArgumentList '-addstore','-f','Root','{ca_pem_path}' -Verb RunAs -Wait -PassThru; exit $p.ExitCode"
        ),
    ],
)
```

**Linux trust**（pkexec 前置，原 /bin/sh -c 串不变）：
```rust
(
    "/usr/bin/pkexec".to_string(),
    vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        format!(
            "cp {ca_pem_path} /usr/local/share/ca-certificates/aidog-ca.crt && update-ca-certificates"
        ),
    ],
)
```
untrust 三 OS 同理（macOS `delete-certificate -Z <sha1>`、Windows `certutil -delstore Root <sha1>`、Linux rm + `--fresh`），都包进对应提权 wrapper。

### 前端 `MitmConfig.tsx` 错误区分逻辑（基于 Q5）

```ts
function classifyTrustError(os: string, code: number | null, stderr: string): 'cancel' | 'auth_fail' | 'cmd_fail' | 'no_agent' {
  if (os === 'linux') {
    if (code === 126) return 'cancel';
    if (code === 127) return stderr.includes('agent') || stderr.includes('polkit') ? 'no_agent' : 'auth_fail';
    return 'cmd_fail';
  }
  if (os === 'macos') {
    if (stderr.includes('(-128)')) return 'cancel';
    if (/(Authorization|authorization|鉴权)/.test(stderr)) return 'auth_fail';
    return 'cmd_fail';
  }
  // windows
  if (stderr.includes('1223') || /cancel/i.test(stderr)) return 'cancel';
  return 'cmd_fail';
}
```

---

## 需人工实测验证的不确定项

| 项 | 不确定点 | 建议实测命令 |
|---|---|---|
| Windows `-PassThru -Verb RunAs` exit 传播 | PowerShell 能否从 RunAs 进程拿 `.ExitCode` 并 `exit` 传播给 plugin-shell | Win 上配 capability 跑 `Command.create("windows-trust-ca", ["-Command", "..."]).execute()`，断言 `out.code === 0`（装成功）|
| macOS validator `\"` 转义层 | JSON `\\\"` → Rust 字面 `\"` → AppleScript 解析 `"` 是否与 ca.rs 产出对齐 | cargo test 扩 ca_linux_capability_validator_matches_commands 模式加 macos 分支字面锁 |
| `app.path().parse` 对 Windows 绝对路径 cmd | 含 `\\` 的 cmd 是否被当变量前缀误解析 | 人工跑 `Command.create("windows-trust-ca", ...)` 看是否 `ProgramNotAllowed` |
| Linux pkexec 在 GUI 桌面真实弹框 | GNOME 实机测 pkexec `/bin/sh -c "..."` 是否弹框 + 成功 exit 0 | Ubuntu GNOME 实机跑 |

---

## 引用清单

| 来源 | URL / 路径 | 用途 |
|---|---|---|
| Tauri Shell Plugin v2 文档 | https://v2.tauri.app/plugin/shell/ | Permissions 段示例确认 `cmd` 键 + validator 结构 |
| Tauri Scope 文档 | https://v2.tauri.app/security/scope/ | scope 由 plugin 自行 enforce |
| 插件源码 scope.rs | `/Users/luoxin/.cargo/registry/src/rsproxy.cn-e3de039b2554c837/tauri-plugin-shell-2.3.5/src/scope.rs:48-86` | command 字段类型 PathBuf + parse 不做白名单 + validator raw 锚定逻辑 |
| 插件源码 scope_entry.rs | 同上 `scope_entry.rs:19-51` | **`#[serde(rename = "cmd")]` 确认 JSON 键必须 `cmd`** |
| 插件源码 commands.rs | 同上 `commands.rs:182-215` | `execute` 走 `command.output()` 返 `output.status.code()` = 直接子进程 exit |
| 插件源码 process/mod.rs | 同上 `src/process/mod.rs` | ExitStatus.code / signal 字段 |
| 项目 schema | `src-tauri/gen/schemas/desktop-schema.json` ShellScopeEntry 定义 | `"required": ["cmd", "name"]` |
| 项目 capability | `src-tauri/capabilities/mitm-ca.json` | 现状用 `command` 键（潜在 bug） |
| 项目 ca.rs | `src-tauri/src/gateway/mitm/ca.rs:336-432` | trust_ca_command / untrust_ca_command 三 OS 分支 |
| 项目 MitmConfig.tsx | `src/components/settings/MitmConfig.tsx:64,77` | 前端 invoke + 兜底路径 |
| pkexec manpage | https://manpages.ubuntu.com/manpages/jammy/man1/pkexec.1.html | RETURN VALUE 126=取消 127=auth失败；internal textual agent |
| MS Learn Process.ExitCode | https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.process.exitcode | `-PassThru` 后 `.ExitCode` 反映子进程退出码 |
| 实测 osascript | 本机 macOS 2026-07-03 | exit 恒 1，原 code 在 stderr 括号；`-128`=cancel |
| crates.io polkit | https://crates.io/api/v1/crates?q=polkit | polkit-agent 0.19 / zbus-polkit-agent 0.4.3 / polkit-agent-rs 0.3 可选自建 agent |

---

## Caveats / 失败处理

- **若 Windows `-PassThru -Verb RunAs` 实测拿不到 exit code**：fallback 改 `exit $LASTEXITCODE`（Start-Process 不 -PassThru，靠 `$LASTEXITCODE` 自动变量 —— 但 `-Verb RunAs` 路径下 `$LASTEXITCODE` 不一定更新，仍需 -PassThru 最稳）。再不行 fallback 走 `WScript.Shell` ShellExecute + 等待，或退 Rust 侧 `std::process::Command` + `runas` verb。
- **若全方案 Windows 实测不可行**：main 转 plan B（Rust 侧 `std::process::Command::new("powershell")` 直接调，不经 plugin-shell scope，但失去 capability 沙箱 —— 需自建命令白名单）。PRD 失败处理段已预案此路径。
- **`cmd` 键修正**是本次研究最大副作用发现：修 `command` → `cmd` 后，**现有 ST1 测试 `ca_trust_command_returns_os_specific` 仍过**（测的是 Rust fn 返值，与 JSON 键无关），但建议加一个 capability JSON 加载冒烟测试（启动时尝试解析 scope）防回归。
