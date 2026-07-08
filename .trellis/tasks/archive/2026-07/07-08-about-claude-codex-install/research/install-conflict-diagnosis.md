# Research: 诊断安装冲突

- **Query**: Claude Code / Codex CLI 多安装源冲突的检测与展示，参考 cc-switch 实现
- **Scope**: external（cc-switch 源码深读）+ internal（aidog 适配建议）
- **Date**: 2026-07-08

## Findings

### cc-switch 的「诊断安装冲突」功能（misc.rs:1849-2539）

cc-switch 完整实现了冲突诊断闭环，是直接参考样本。

#### 核心函数：`enumerate_tool_installations(tool)` (misc.rs:1849-1930)

枚举工具在系统中的**所有安装**，不短路。流程：

1. `build_tool_search_paths(tool)` 构建候选目录列表（misc.rs:1489-1616）：
   - **用户级原生路径**：`~/.local/bin`, `~/.npm-global/bin`, `~/n/bin`, `~/.volta/bin`, `~/.local/share/mise/shims` + mise node installs
   - **macOS 系统级**：`/opt/homebrew/bin`, `/usr/local/bin`（hermes 还扫 `~/Library/Python/*/bin`）
   - **Linux 系统级**：`/usr/local/bin`, `/usr/bin`
   - **Windows**：`%APPDATA%\npm`, `C:\Program Files\nodejs` 等
   - **fnm 多 shell**：`~/.local/state/fnm_multishells/*/bin`
   - **nvm 多 node 版本**：`~/.nvm/versions/node/*/bin`
   - **PATH env**：`extend_from_cli_path_env` 把 PATH 里的目录也加进来
2. `resolve_path_default(tool)` (misc.rs:1801-1844)：用 `$SHELL -lic 'command -v {tool}'`（POSIX）/ `cmd /C where {tool}`（Windows）拿命令行实际命中的路径，`canonicalize` 后作为「PATH 默认」锚点
3. 对每个候选目录的每个候选文件（`tool_executable_candidates`，Windows 试 `.cmd` / `.exe` / 裸名，POSIX 直接 `dir/tool`）：
   - `std::fs::canonicalize` 解析软链后**去重**（`HashSet<PathBuf>`，避免 `/opt/homebrew/bin/x` 与 `Cellar/x/.../x` 算两份）
   - 跑 `<tool_path> --version`（POSIX 直接 spawn；Windows `cmd /D /S /C "call <path> --version"` + `CREATE_NO_WINDOW`）
   - 三态判定：成功（version）/ 装了跑不起来（runnable=false + error）/ spawn 失败（error = io::Error）
4. 排序：`is_path_default=true` 排最前（UI 一眼看到「命令行默认用的是哪处」）

#### `ToolInstallation` 数据结构（misc.rs:1731-1752）

```rust
pub struct ToolInstallation {
    path: String,              // 候选入口路径（未解析软链）
    version: Option<String>,   // --version 成功时的版本号
    runnable: bool,            // --version 是否 exit 0
    error: Option<String>,     // 跑不起来时的诊断末尾 4 行
    source: String,            // 由路径前缀推断（nvm/homebrew/volta/...）
    is_path_default: bool,     // 是否为 PATH 解析到的那处
    #[serde(skip)] real: PathBuf, // canonicalize 后真身（去重 + 锚定共用）
}
```

#### `infer_install_source` 路径前缀推断（misc.rs:1756-1789）

纯字符串匹配，顺序敏感：

| 路径片段（小写、`\` → `/`） | 推断 source |
|---|---|
| `/.nvm/` | `nvm` |
| `/homebrew/` 或 `/cellar/` | `homebrew` |
| `/.volta/` 或 `/volta/` | `volta` |
| `fnm_multishells` | `fnm` |
| `/mise/` | `mise` |
| `/.bun/` | `bun` |
| `/pnpm/` | `pnpm` |
| `/scoop/` | `scoop` |
| `/library/python` / `/scripts/` / `/site-packages/` | `pip` |
| 其他 | `system` |

**注意 Homebrew Cellar 真身要先于通用规则命中**（注释 line 1763：顺序敏感）。

#### 冲突判定 `is_conflicting` (misc.rs:2478-2487)

**严阈值**（驱动「诊断按钮」红色警告展示）：

```rust
fn is_conflicting(installs: &[ToolInstallation]) -> bool {
    if installs.len() < 2 { return false; }
    let distinct_versions: HashSet<&Option<String>> = installs.iter().map(|i| &i.version).collect();
    let runnable_mixed = installs.iter().any(|i| i.runnable) && installs.iter().any(|i| !i.runnable);
    distinct_versions.len() > 1 || runnable_mixed
}
```

**关键**：同版本装两份且都能跑**不算冲突**（不打扰用户）。只有以下情况才报警：
- 版本分歧（distinct_versions > 1）
- 运行态混合（有的能跑有的跑不起来）

**宽阈值** `needs_confirmation` (misc.rs:2470-2473)：`installs.len() >= 2` 即触发（升级前确认弹窗，任何多处都该让用户知情，即使版本一致——因为升级会动一处，另一处可能遮蔽）。

### UI 展示（AboutSection.tsx:1152-1169）

冲突块在卡片底部条件渲染（仅 `toolDiagnostics[toolName]` 有数据时）：

```tsx
{conflicts && conflicts.length > 0 && (
  <div className="rounded-lg border border-yellow-500/20 bg-yellow-500/5 p-2.5">
    <div className="text-[11px] font-medium text-yellow-600">冲突标题</div>
    <p className="text-[10px] text-muted-foreground">提示文案</p>
    <ul>
      {conflicts.map((inst) => (
        <li key={inst.path}><ToolInstallRow inst={inst} /></li>
      ))}
    </ul>
  </div>
)}
```

每行 `ToolInstallRow`（src/components/settings/ToolInstallRow.tsx，37 行）展示：路径 + 版本 + source badge + `is_path_default` 标记。

### 触发时机（AboutSection.tsx:509-557）

两条触发路径：

1. **用户主动**：顶部「诊断安装冲突」按钮（`handleDiagnoseAll`，line 532）→ 一次性扫全部 6 工具 → 有冲突的写入各自卡片 state → 全部无冲突给 info toast
2. **自动补诊**（`diagnoseToolSilently`，line 509）：升级后静默后台执行，**有冲突才弹展示，无冲突清掉残留**（外部卸载/修复后冲突可能已消失，不清会一直显示旧列表）。三种场景：
   - 升级后版本未变（`versionUnchangedAfterUpdate`）→ 自动补诊（多半被另一处遮蔽）
   - 升级成功（version 变了）→ 无条件补诊（另一处可能仍在）
   - 装了跑不起来（`notRunnable`）→ 自动补诊（多处安装定位根因）

### 升级前确认弹窗（AboutSection.tsx:740-786）

`handleRunToolAction` 走 preflight：

```ts
const reports = await settingsApi.probeToolInstallations(toolNames);
const needConfirm = reports.filter((r) => r.needs_confirmation);
if (needConfirm.length === 0) {
  await executeRun(toolNames, action);  // 无多处，直接执行
} else {
  setPendingUpgrade({ toolNames, plans: needConfirm });  // 弹确认对话框
}
```

`ToolUpgradeConfirmDialog`（src/components/settings/ToolUpgradeConfirmDialog.tsx，102 行）展示每个工具的所有安装 + 锚定命令，用户 Confirm 后才执行。

### 「装了跑不起来」+ codex 平台二进制损坏（misc.rs:2139-2163）

cc-switch 识别 codex 特有的损坏模式：主包 `@openai/codex` 是纯 JS launcher + 平台二进制 optional 依赖 `@openai/codex-<triple>`，平台二进制缺失时 `--version` 报 `Missing optional dependency`。`enumerate_tool_installations` 标 `runnable=false`，`is_conflicting` 判定为冲突（runnable_mixed），UI 展示损坏诊断。修复命令 `codex_repair_command`（misc.rs:2165-2183）：`<npm> uninstall -g @openai/codex || true; <npm> i -g @openai/codex@latest`（普通 `npm i -g` 是 no-op 修不好）。

### 同类工具对比

cc-switch 是目前唯一观察到「诊断安装冲突」完整实现的同类工具。`claude-code-router` / `aider` / 其他 CLI manager 主要关注配置切换，不做版本管理。**这是 cc-switch 的差异化能力，aidog 抄过来即是行业最佳实践**。

## 关键结论（5 条）

1. **`is_conflicting` 严阈值**：版本分歧 或 运行态混合才报警；同版本装两份且都能跑**不算冲突**（避免打扰）。
2. **`needs_confirmation` 宽阈值**：≥2 处即触发升级前确认（升级只动一处，任何多处都该让用户知情）。
3. **`infer_install_source` 路径前缀推断**：纯字符串匹配，无副作用，UI 徽章驱动（nvm/homebrew/volta/fnm/mise/bun/pnpm/scoop/pip/system）。Homebrew Cellar 必须先于通用规则命中。
4. **去重靠 `canonicalize`**：`/opt/homebrew/bin/x` → `Cellar/...`，nvm shim 等 symlink 入口去重为同一真实文件。
5. **触发时机三场景**：用户主动诊断 + 升级后版本未变 + 装了跑不起来——三者共用 `probe_tool_installations` 命令，避免多套判定逻辑。

## 对 aidog PRD 的建议

### MVP 范围（建议必做）

- **`enumerate_tool_installations`**：抄 cc-switch 的搜索路径列表（macOS + Linux + Windows 三平台），**裁剪为只支持 claude + codex 两个工具**（去掉 hermes Python 路径 / opencode GOPATH 等）。
- **`infer_install_source`**：直接抄路径前缀表（misc.rs:1756-1789），aidog 无需改。
- **冲突判定**：抄 `is_conflicting`（版本分歧 + runnable_mixed）+ `needs_confirmation`（≥2 处）双阈值。
- **UI 展示**：卡片底部黄色警告框，列 path + version + source badge + `is_path_default` 标记。aidog 现有 UI 风格（Liquid Glass）调整为玻璃质感即可。

### 自动检测可做（建议做）

- **升级后版本未变**：自动补诊（多半被另一处遮蔽，是冲突的最强信号）
- **装了跑不起来**（`installed_but_broken=true`）：自动补诊定位根因
- **codex 平台二进制损坏**：检测到 `runnable=false` + npm 来源（nvm/fnm/mise/homebrew）→ 提示跑 uninstall + install 自愈

### 只报告不自动修（MVP 立场）

- **卸载建议**：UI 给「保留 PATH 默认那处，卸载其他」的建议文案，但**不自动 `npm uninstall -g` / `rm`**（破坏性操作，需用户明确同意；aidog 全局禁主动破坏性操作）。
- **手动命令展示**：抄 cc-switch 的 `manual_display` 模式，给用户可复制的卸载命令（如 `npm uninstall -g @anthropic-ai/claude-code` / `brew uninstall claude`），让用户自己执行。
- **PATH 顺序建议**：如果检测到 PATH 默认那处不是用户期望的（如 nvm 的旧版遮蔽了原生安装器的新版），提示「调整 PATH 顺序把 ~/.local/bin 放在 ~/.nvm/versions/node/*/bin 之前」+ 给具体 shell rc 文件修改建议。

### 不建议做（YAGNI）

- **WSL 跨边界诊断**：aidog 当前无 WSL 路径管理，WSL 探测复杂度高，后置。
- **自动修 PATH**：写用户 `.zshrc` / `.bashrc` 风险高，容易破坏现有配置；只报告 + 给建议。
- **预发布通道补查**：只 claude 一个工具有 `next` tag，复杂度不值。
