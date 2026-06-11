# PRD: Claude Code 状态栏配置模块

> 2026-06-11 · nico
> v3 — 路径修正：所有文件均在 `~/.aidog/` 下

## 背景

Claude Code 支持通过 `statusLine` 和 `subagentStatusLine` 配置自定义状态栏。当前 aidog Settings 页面的 "状态" section 将两者作为**纯文本 string** 字段渲染，而实际应为 JSON 对象。

用户期望的使用模式：
- **启用内置** → aidog 根据用户选择的模板生成脚本文件，将脚本绝对路径写入 `~/.aidog/settings.{group}.json`
- **不启用内置** → aidog 不触碰 `statusLine` / `subagentStatusLine` 字段，用户自行通过 Claude Code 原生方式配置

### 存储架构（已有）

aidog 不写入 `~/.claude/`。所有配置在 `~/.aidog/` 下：

```
~/.aidog/
├── settings.{group_name}.json   ← 每分组一份 Claude Code 配置（含 statusLine）
├── aidog.db                     ← SQLite（settings 表存 base config）
├── aidog-statusline.sh          ← [新增] 生成的 statusline 脚本
└── aidog-subagent-statusline.sh ← [新增] 生成的 subagent statusline 脚本
```

**同步流程**（已有，`lib.rs:753 do_sync_group_settings`）：
1. DB `settings` 表 `scope=global, key=claude_code` 存 base config
2. 每次保存时，base config → clone → 注入 proxy env → 写入 `~/.aidog/settings.{group}.json`
3. `statusLine` 作为 base config 的一部分，自动同步到所有分组配置

参考文档：https://code.claude.com/docs/zh-CN/statusline

## 核心交互

```
用户在 Settings → 状态 页面：
  ├─ StatusLine
  │   ├─ [ ] 使用内置状态栏 ← 默认关闭
  │   │   关闭时：statusLine 字段不变（保留用户原有自定义配置或 undefined）
  │   │   开启时：
  │   │     1. 用户选择模板 / 自定义字段组合
  │   │     2. 前端生成脚本内容
  │   │     3. 后端写入 ~/.aidog/aidog-statusline.sh + chmod +x
  │   │     4. base config 写入 statusLine: { type, command: "~/.aidog/aidog-statusline.sh", ... }
  │   │     5. do_sync_group_settings 自动同步到所有 settings.{group}.json
  │   │
  │   └─ [高级] 直接编辑 JSON（手动模式）
  │
  └─ SubagentStatusLine
      ├─ [ ] 使用内置子代理状态栏 ← 默认关闭
      │   同上流程，生成 ~/.aidog/aidog-subagent-statusline.sh
      │
      └─ [高级] 直接编辑 JSON
```

## 数据模型

### ~/.aidog/settings.{group}.json 中的 statusLine（目标格式）

```jsonc
{
  "statusLine": {
    "type": "command",
    "command": "/Users/<user>/.aidog/aidog-statusline.sh",  // 绝对路径
    "padding": 2,
    "refreshInterval": 5,
    "hideVimModeIndicator": false
  },
  "subagentStatusLine": {
    "type": "command",
    "command": "/Users/<user>/.aidog/aidog-subagent-statusline.sh"
  },
  "env": {
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:9123/proxy",
    "ANTHROPIC_AUTH_TOKEN": "my-group"
  }
}
```

### aidog DB 内部存储（_aidog_ 前缀）

存在 `claude_code` config 对象中，仅 aidog 读取：

```jsonc
{
  "_aidog_statusline": {
    "enabled": true,
    "template": "multi-line",
    "padding": 2,
    "refreshInterval": 5,
    "hideVimModeIndicator": false,
    "options": {
      "showGit": true,
      "showContext": true,
      "showCost": false,
      "showRateLimits": false
    }
  },
  "_aidog_subagent_statusline": {
    "enabled": true,
    "template": "default",
    "options": {}
  }
}
```

## 功能需求

### F1. StatusLine 配置面板

**条件启用开关**：
- Toggle: "使用内置状态栏"
- 默认关闭
- 关闭时：`statusLine` 字段**完全不动**
- 开启时：根据配置生成脚本并写入 base config

**模板选择**（开启后才显示）：

| 模板 ID | 名称 | 输出效果 |
|---------|------|----------|
| `context-bar` | 上下文进度条 | `[Opus] ▓▓▓▓▓▓░░░░ 65%` |
| `git-status` | Git 状态 | `main ✓3 Δ2` |
| `cost-tracker` | 成本追踪 | `$0.12 · 2m 35s` |
| `multi-line` | 多行综合（推荐） | L1: Git + 模型 / L2: 进度条 + 成本 |
| `minimal` | 极简 | `Opus 65%` |
| `custom` | 自定义组合 | 用户自选字段 |

**模板参数面板**（`multi-line` 或 `custom` 时显示）：
- 勾选：模型名称 / 上下文进度 / Git 状态 / 成本 / 速率限制 / Effort level / Vim 模式

**通用选项**：
- padding: 0-20
- refreshInterval: 0-300 秒（0=仅事件驱动）
- hideVimModeIndicator: Toggle

**脚本预览**：折叠区，只读 pre 展示生成的脚本内容

**脚本路径显示**：`~/.aidog/aidog-statusline.sh`（只读）

### F2. SubagentStatusLine 配置面板

简化版：
- 条件启用开关
- 模板选择（subagent 专用）

| 模板 ID | 名称 | 说明 |
|---------|------|------|
| `default` | 默认 | 任务名 + 状态 + token 数 |
| `compact` | 紧凑 | 仅任务名 |
| `detailed` | 详细 | 名 + 描述 + 耗时 + tokens |

### F3. 脚本生成与部署（后端）

**Tauri command**：`generate_statusline_script`

```rust
#[tauri::command]
fn generate_statusline_script(
    script_type: String,  // "statusline" | "subagent"
    content: String,
) -> Result<String, String> {
    // 1. 路径: ~/.aidog/aidog-statusline.sh 或 aidog-subagent-statusline.sh
    // 2. 写入文件
    // 3. chmod +x (Unix)
    // 4. 返回绝对路径
}
```

**保存流程**：
1. 根据 `_aidog_statusline.template` + `options` → 前端生成脚本内容字符串
2. `invoke("generate_statusline_script", { scriptType, content })` → 得到绝对路径
3. `config.statusLine = { type: "command", command: 绝对路径, padding, refreshInterval, hideVimModeIndicator }`
4. `config._aidog_statusline = { enabled: true, template, options, ... }`
5. `settingsApi.set("global", "claude_code", config)` → 触发 `do_sync_group_settings` → 所有分组配置更新

### F4. 可用数据参考面板

可折叠面板，列出 stdin JSON 全部可用字段（分组显示）。

### F5. 禁用时清理

关闭 Toggle 时：
- `config.statusLine = undefined`（从 base config 移除）
- 保留 `_aidog_statusline`（含 `enabled: false`），下次开启恢复
- 保留脚本文件不删除（避免竞态）

## UI 布局

```
┌─ 状态 ──────────────────────────────────────────────────────┐
│                                                                │
│  ┌─ StatusLine ────────────────────────────────────────┐     │
│  │  使用内置状态栏  [● Toggle]                           │     │
│  │                                                       │     │
│  │  模板:  [●多行综合] [上下文] [Git] [成本] [极简] [自定义] │     │
│  │                                                       │     │
│  │  显示字段:                                            │     │
│  │  [✓] 模型名称  [✓] 上下文进度  [✓] Git 状态            │     │
│  │  [ ] 成本追踪  [ ] 速率限制   [ ] Effort level         │     │
│  │                                                       │     │
│  │  水平间距 [2]    刷新间隔 [5] 秒   隐藏Vim模式 [ ]      │     │
│  │                                                       │     │
│  │  ▶ 脚本预览                                          │     │
│  │  #!/usr/bin/env bash                                  │     │
│  │  input=$(cat)                                         │     │
│  │  ...                                                  │     │
│  │                                                       │     │
│  │  脚本路径: ~/.aidog/aidog-statusline.sh               │     │
│  └───────────────────────────────────────────────────────┘     │
│                                                                │
│  ┌─ SubagentStatusLine ────────────────────────────────┐     │
│  │  使用内置子代理状态栏  [○ Toggle]                     │     │
│  └───────────────────────────────────────────────────────┘     │
│                                                                │
│  ┌─ File Suggestion ───────────────────────────────────┐     │
│  │  [选择文件或输入路径…                    ] [📁]       │     │
│  └───────────────────────────────────────────────────────┘     │
│                                                                │
│  ▶ 可用数据字段参考                                             │
│  ┌───────────────────────┬──────────────────────────────┐     │
│  │ model.display_name    │ 模型显示名称                  │     │
│  │ context_window.used_% │ 上下文使用百分比              │     │
│  │ cost.total_cost_usd   │ 累计预估成本 ($)              │     │
│  │ ...                   │ ...                          │     │
│  └───────────────────────┴──────────────────────────────┘     │
└────────────────────────────────────────────────────────────────┘
```

## 技术方案

### 前端变更

| 文件 | 变更 |
|------|------|
| `src/services/claude-settings-schema.ts` | `statusLine`/`subagentStatusLine` 标记 `skipGui: true` |
| `src/pages/Settings.tsx` | 新增 `StatusLinePanel` / `SubagentStatusLinePanel` / `DataRefPanel` 组件 + renderSectionContent 分支 |
| `src/services/api.ts` | 新增 `generateStatuslineScript` Tauri invoke 封装 |

新增常量：
- `STATUSLINE_TEMPLATES` — 模板定义（ID、名称、脚本生成函数）
- `STATUSLINE_DATA_FIELDS` — 可用数据字段参考表

### 后端变更

| 文件 | 变更 |
|------|------|
| `src-tauri/src/lib.rs` | 新增 `generate_statusline_script` Tauri command |

Command 实现：
```rust
#[tauri::command]
fn generate_statusline_script(
    script_type: String,  // "statusline" | "subagent"
    content: String,
) -> Result<String, String> {
    let aidog_dir = aidog_data_dir()?;  // ~/.aidog/
    let filename = if script_type == "subagent" {
        "aidog-subagent-statusline.sh"
    } else {
        "aidog-statusline.sh"
    };
    let path = aidog_dir.join(filename);
    std::fs::write(&path, &content).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).map_err(|e| e.to_string())?;
    }
    Ok(path.to_string_lossy().to_string())
}
```

### 脚本生成（前端纯字符串拼接）

```typescript
interface TemplateOptions {
  showModel: boolean;
  showContext: boolean;
  showGit: boolean;
  showCost: boolean;
  showRateLimits: boolean;
  showEffort: boolean;
  showVim: boolean;
}

function generateScript(templateId: string, options: TemplateOptions): string;
```

每个模板 = 一个函数，返回完整 bash 脚本字符串。使用 `jq` 解析 stdin JSON。

## 实现范围

### 首期（必做）

- [ ] 后端：`generate_statusline_script` command
- [ ] 前端：StatusLine 配置面板（开关 + 模板 + 参数 + 预览）
- [ ] 前端：SubagentStatusLine 配置面板
- [ ] 前端：5 + 3 个模板脚本生成函数
- [ ] 前端：可用数据参考面板
- [ ] Schema skipGui 标记
- [ ] 保存/加载流程（_aidog_ 前缀内部字段 + 生成脚本 + 写入 config）
- [ ] i18n

### 后续迭代（可选）

- [ ] 脚本语法校验（sh -n）
- [ ] 模拟测试

## 验收标准

1. Settings → 状态 tab：`statusLine`/`subagentStatusLine` 为结构化配置面板
2. Toggle 关闭时：`statusLine` 字段不被 aidog 修改
3. Toggle 开启 + 选择模板 → `~/.aidog/aidog-statusline.sh` 存在且可执行
4. `~/.aidog/settings.{group}.json` 中 `statusLine.command` 指向 `~/.aidog/aidog-statusline.sh`
5. 重新打开 Settings → 从 `_aidog_statusline` 恢复模板选择和参数
6. `fileSuggestion` 不受影响
7. `make lint` 无新 warning
8. 生成脚本手动执行不报错：`echo '{"model":{"display_name":"Opus"},"context_window":{"used_percentage":42}}' | ~/.aidog/aidog-statusline.sh`
