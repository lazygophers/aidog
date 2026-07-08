# Research: cc-switch「关于 > 本地环境」功能拆解

- **Query**: 拆解 farion1231/cc-switch 的 CLI 工具版本检查 / 安装 / 升级实现
- **Scope**: external（GitHub 源码静态分析）
- **Date**: 2026-07-08
- **仓库版本**: cc-switch v3.16.5（main 分支克隆，commit ~ 2026-07）

## Findings

### 仓库定位

- cc-switch 是 Tauri 2.0 + React + TypeScript 应用（与 aidog 同栈），pnpm workspace。
- 「关于 > 本地环境」实现在 `src/components/settings/AboutSection.tsx`（1265 行）+ 后端 `src-tauri/src/commands/misc.rs`（5329 行，含全平台逻辑）。
- 后端命令三件套（注册在 `src-tauri/src/lib.rs:1417-1419`）：

| 命令 | 用途 |
|---|---|
| `get_tool_versions(tools, wsl_shell_by_tool)` | 检测本地版本 + 拉远程 latest |
| `run_tool_lifecycle_action(tools, action, wsl_shell_by_tool)` | install / update 静默执行 |
| `probe_tool_installations(tools)` | 多处安装冲突枚举 + 锚定命令规划 |

### 检测的 CLI 工具（前端硬编码 6 个）

`AboutSection.tsx:62-69` `TOOL_NAMES = ["claude", "codex", "gemini", "opencode", "openclaw", "hermes"]`。aidog 只需前两个。

### 版本检查实现（misc.rs:991-1036 `try_get_version`）

非 Windows：用 `$SHELL -lic '{tool} --version'`（登录交互式 shell，等同 `env.rs::probe_login_path` 思路）。Windows：不走 PATH shell，强制 `scan_cli_version` 只跑已定位到的真实 .exe/.cmd（避免 App Execution Alias / 协议处理器误触发）。

退出码语义三态（misc.rs:982-989 `ShellProbe`）：
- `Found(v)` — exit 0，stdout/stderr 用 `VERSION_RE = \d+\.\d+\.\d+(-[\w.]+)?` 提取版本号。
- `FoundButFailed(e)` — exit ≠ 0 且 ≠ 127，**装了但 `--version` 自身报错**（如 Node 版本不够、平台二进制损坏）。前端独立 `installed_but_broken` 字段，禁匹配 error 文案反推。
- `NotFound` — exit 127（command not found）或 spawn 失败。落到 `scan_cli_version` 兜底扫常见路径。

「latest 版本」拉取（misc.rs:759-777）：
- claude → npm registry `@anthropic-ai/claude-code` 的 dist-tags
- codex → `@openai/codex`
- 仅 claude 启用预发布通道补查（`next` tag，仅当本地严格领先 latest 才纳入比较，misc.rs:800-805）
- opencode 双源：npm 失败回退 GitHub releases `anomalyco/opencode`
- hermes → PyPI `hermes-agent`

### 「安装」按钮的命令（misc.rs:439-498, 598-663）

非 WSL 平台按工具分发（misc.rs:434-451 注释：官方 installer 都不用 `curl | bash` pipe 形式，先下载到 mktemp 再交 bash 执行，避免 WSL 子 shell `set -o pipefail` 不继承）：

| 工具 | install 命令（POSIX） |
|---|---|
| **claude** | `bash -c 'tmp=$(mktemp) && curl -fsSL https://claude.ai/install.sh -o $tmp && bash $tmp; status=$?; rm -f $tmp; exit $status'` \|\| `npm i -g @anthropic-ai/claude-code@latest` |
| **codex** | `npm i -g @openai/codex@latest`（无 native installer） |
| gemini | `npm i -g @google/gemini-cli@latest` |
| opencode | 官方 installer \|\| `npm i -g opencode-ai@latest` |
| openclaw | `npm i -g openclaw@latest` |
| hermes | 官方 installer（curl install.sh） |
| Windows 原生 | 全部走 `npm i -g`（install.sh 是 bash 脚本，Windows 跑不了）；hermes 走 PowerShell `irm ... \| iex`（base64 EncodedCommand 包裹） |

Windows 执行（misc.rs:228-244 `run_tool_lifecycle_silently`）：把多行命令拼成 .bat 临时文件（`\r\n` 分隔），`cmd /C` 调用，`CREATE_NO_WINDOW = 0x08000000` flag 抑制控制台窗口闪现。

POSIX 执行（misc.rs:213-223）：`bash -c "<脚本>"` 强制用 bash（避免用户默认 fish/zsh 时 `set -e` 语义不一致），`set -e` + `set -o pipefail` 任一工具失败即中止整批。

### 「升级」实现（misc.rs:514-575, 2370-2392）

**关键设计：锚定升级（anchored update）**。misc.rs:644-662 注释明确：

> update 锚定到命令行实际命中的那处（写回同一个 node / brew / 原生安装器），而非裸 `npm` 落到 PATH 第一个 npm。

流程：
1. `enumerate_tool_installations(tool)` 枚举所有安装位置（见 install-conflict-diagnosis.md）
2. `default_install(installs)` 选 PATH 默认那处（`is_path_default=true`）
3. `installs_anchored_command` 按真身路径推断升级命令：
   - **claude 原生安装器**（`~/.local/share/claude/versions/`）→ `<bin_path 绝对> update`
   - **homebrew formula**（真身在 `Cellar/<formula>/`）→ `<同级 brew> upgrade <formula>`
   - **volta** → `<同级 volta> install <pkg>`
   - **bun** → `<同级 bun> add -g <pkg>@latest`
   - **nvm/fnm/mise/homebrew npm** → `<同级 npm> i -g <pkg>@latest`
   - **system/未知** → None，退到静态 fallback
4. fallback：`{tool} update || npm i -g <pkg>@latest`（claude/codex/hermes 用 `update` 子命令，opencode 用 `upgrade`，openclaw 用 `update --yes`）

**关键不变量（misc.rs:2232-2239）**：返回的命令必须用**绝对路径**调用执行体，不依赖 PATH。原因是 GUI 进程 PATH 由 launchd / Windows Service / systemd 给，通常**不含** `~/.local/bin` / `/opt/homebrew/bin`，而探测时用的是 `$SHELL -lic`（登录 shell 会读 .zshrc/.zprofile）。两者 PATH 不对称 → 裸 `claude update` / `brew upgrade ...` 在 GUI 进程里大概率 `command not found`（exit 127）→ `set -e` 中止 → 用户看到失败 toast 但 UI 显示「将写回原生那处」——欺骗性故障。

### UI 展示（AboutSection.tsx:1001-1211）

每工具卡片：
- 工具图标 + 名称
- env_type 徽标（windows/wsl/macos/linux）+ wsl_distro
- 顶部状态图标：`CheckCircle2` 绿（已装且最新）/ `AlertCircle` 黄（已装但过期或损坏）/ `Loader2`（探测中）
- 「当前版本」`tool.version || tool.error || "未安装"`（installed_butbroken 显示 `t('settings.installedNotRunnable')` 而非「未安装」，避免给无效的安装按钮）
- 「最新版本」`tool.latest_version || "未知"`
- 多处安装冲突块（仅 `toolDiagnostics[tool]` 有数据时渲染）：黄色警告框，列每个 install 的 path/version/source badge
- 按钮：install（outline）/ update（default）/ 无按钮（已最新或 broken）

WSL 工具额外：两个 Select 让用户指定 `wslShell`（sh/bash/zsh/fish/dash）+ `wslShellFlag`（-lic/-lc/-c），影响后端 `$SHELL` 探测。

底部：「批量更新 N 个」按钮（`updatableToolNames`，仅过期工具）+「诊断安装冲突」+「刷新」+ 可展开的「手动安装命令」（一键复制 POSIX / Windows 脚本块，AboutSection.tsx:127-155）。

### Tauri 还是 Electron？怎么拿 shell 权限？

**Tauri 2.0**，与 aidog 同栈。**没有用 tauri-plugin-shell 的 capability scope**，而是直接在后端 Rust command 里 `std::process::Command::new("bash").arg("-c").arg(cmd).output()`（misc.rs:217）。前端 invoke `run_tool_lifecycle_action` → 后端 spawn_blocking 包 blocking subprocess → 阻塞到命令真正结束 → 把 stderr/stdout 末尾 8 行回传给前端 toast。

**这与 aidog 的 mitm-ca 模式相反**：aidog 把命令字符串返回前端，前端用 `Command.create(name, args).execute()` 走 tauri-plugin-shell（capability 锁定）；cc-switch 后端直接 spawn，capability 没开 shell:allow-execute，前端也无法注入任意命令。两种模式的取舍见 tauri-shell-feasibility.md。

### 关键缓存设计（AboutSection.tsx:181-206）

模块级缓存 `toolVersionsCache: { data, at }`，TTL = 10 分钟。Radix Tabs 卸载非激活 Tab，每次切回「关于」若无缓存会重挂全量重查（6 工具 × `--version` 子进程 + 6 个网络请求）。单工具刷新（切 shell / 升级后）只合并进缓存数据，**不重置 at**（避免局部刷新给整体 TTL 续命）。应用自身版本 `appVersionCache` 独立（无网络、毫秒级），与工具探测解耦，避免被压在「全部工具检查完成」之后。

## 关键结论（5 条）

1. **三命令架构**：检测（get_tool_versions）/ 执行（run_tool_lifecycle_action）/ 诊断（probe_tool_installations）分开，前端按需调用，单工具刷新与全量探测共用同一后端入口。
2. **POSIX 强制 bash + Windows .bat 临时文件**：避免 fish/zsh 的 `set -e` 语义差异；Windows 用 `CREATE_NO_WINDOW` 抑制控制台窗口。
3. **锚定升级是核心创新**：升级命令用**绝对路径**调用执行体，绕开 GUI 进程 PATH 与登录 shell PATH 不对称的根因——这一痛点 aidog 已经在 `gateway/skills/env.rs::ensure_runtime_path` 解决过（同根因）。
4. **ShellProbe 三态**：`Found` / `FoundButFailed`（装了跑不起来）/ `NotFound`，避免前端靠 error 文案反推语义，前端 UX 直接读结构化字段。
5. **后端直接 spawn subprocess**，不依赖 tauri-plugin-shell capability scope；aidog 现有模式（mitm-ca 走 plugin-shell + capability）是另一条路，两者都可工作。

## 对 aidog PRD 的建议

- **范围裁剪**：只支持 `claude` + `codex` 两个工具（aidog 当前定位），避免引入 gemini/opencode/hermes 的维护成本。
- **直接照抄三命令架构**：`get_tool_versions` / `run_tool_lifecycle_action` / `probe_tool_installations` 三层分离，前端按需触发。
- **采纳锚定升级**：aidog 已有 `env.rs::ensure_runtime_path` 经验，PATH 不对称是真痛点；升级命令必须用绝对路径（`<sibling npm> i -g` 或 `<bin 绝对> update`），禁裸 `npm`。
- **暂不抄 WSL 探测**：aidog 当前无 Windows WSL 路径管理，先做 macOS + Linux + Windows 原生三平台，WSL 后置。
- **模块级缓存 + 10 分钟 TTL**：避免切 Tab 重查，单工具刷新不重置整体 TTL——这条 UX 优化值得抄。
- **预发布通道补查**先不做（只 claude 一个工具，复杂度不值）。
