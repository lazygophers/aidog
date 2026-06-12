# Claude Code statusLine 输入 JSON Schema 完整清单

**源**: 官方文档 https://code.claude.com/docs/en/statusline  
**更新日期**: 2026-06-12  
**版本**: Claude Code v2.1.90+

## 一、完整字段树

### 1. 模型信息 (`model`)

| 字段 | 类型 | 含义 | 始终存在 |
|------|------|------|--------|
| `model.id` | string | 模型标识符（如 `"claude-opus-4-8"`) | ✓ |
| `model.display_name` | string | 模型显示名（如 `"Opus"`) | ✓ |

### 2. 工作目录与项目信息 (`workspace` / 顶层)

| 字段 | 类型 | 含义 | 始终存在 | 备注 |
|------|------|------|--------|-------|
| `cwd` | string | 当前工作目录（绝对路径） | ✓ | 与 `workspace.current_dir` 相同 |
| `workspace.current_dir` | string | 当前工作目录（绝对路径） | ✓ | **推荐**，与 `cwd` 一致 |
| `workspace.project_dir` | string | 项目启动目录（可能与 `cwd` 不同） | ✓ | 若运行中改变工作目录，此字段保持初始值 |
| `workspace.added_dirs` | array<string> | `/add-dir` 或 `--add-dir` 添加的目录列表 | ✓ | 无时为空数组 `[]` |
| `workspace.git_worktree` | string | Git worktree 名称（如 `"feature-xyz"`) | ✗ | **缺失时**: 不在 git worktree 中 |
| `workspace.repo.host` | string | Git 仓库主机（如 `"github.com"`) | ✗ | **缺失条件**: 不在 git 仓库 OR 无 origin remote |
| `workspace.repo.owner` | string | 仓库所有者（如 `"anthropics"`) | ✗ | 同上 |
| `workspace.repo.name` | string | 仓库名（如 `"claude-code"`) | ✗ | 同上 |

### 3. 成本与执行统计 (`cost`)

| 字段 | 类型 | 含义 | 始终存在 | 备注 |
|------|------|------|--------|-------|
| `cost.total_cost_usd` | number | 本次会话估算成本（USD） | ✓ | 客户端计算，可能与实际账单有差 |
| `cost.total_duration_ms` | number | 会话总墙钟时间（毫秒） | ✓ | 从启动至当前 |
| `cost.total_api_duration_ms` | number | API 响应等待总时间（毫秒） | ✓ | 仅计 API 调用阻塞时间 |
| `cost.total_lines_added` | number | 本次会话新增代码行数 | ✓ |  |
| `cost.total_lines_removed` | number | 本次会话删除代码行数 | ✓ |  |

### 4. 上下文窗口 (`context_window`)

| 字段 | 类型 | 含义 | 始终存在 | 备注 |
|------|------|------|--------|-------|
| `context_window.total_input_tokens` | number | 当前上下文中的输入 token 数 | ✓ | v2.1.132+ 改为当前值（非累积） |
| `context_window.total_output_tokens` | number | 当前上下文中的输出 token 数 | ✓ | v2.1.132+ 改为当前值（非累积） |
| `context_window.context_window_size` | number | 最大上下文窗口大小（token） | ✓ | 默认 200000，扩展模型 1000000 |
| `context_window.used_percentage` | number \| null | 上下文已用百分比 (0-100) | ✗ | 可为 `null`（早期会话） |
| `context_window.remaining_percentage` | number \| null | 上下文剩余百分比 (0-100) | ✗ | 可为 `null`（早期会话） |
| `context_window.current_usage` | object \| null | 最新 API 调用的 token 分解 | ✗ | 可为 `null`（第一次调用前，/compact 后） |

#### 4.1 `context_window.current_usage` 结构

| 字段 | 类型 | 含义 |
|------|------|------|
| `current_usage.input_tokens` | number | 本次输入 token 数 |
| `current_usage.output_tokens` | number | 本次输出 token 数 |
| `current_usage.cache_creation_input_tokens` | number | 缓存写入 token 数 |
| `current_usage.cache_read_input_tokens` | number | 缓存读取 token 数 |

### 5. Token 超限标志

| 字段 | 类型 | 含义 | 始终存在 |
|------|------|------|--------|
| `exceeds_200k_tokens` | boolean | 总 token 数（输入+缓存+输出）是否超 200k | ✓ | 固定阈值，与实际上下文大小无关 |

### 6. 推理与思考模式 (`effort`, `thinking`)

| 字段 | 类型 | 含义 | 始终存在 | 备注 |
|------|------|------|--------|-------|
| `effort.level` | enum | 推理努力等级 | ✗ | **缺失条件**: 模型不支持 effort 参数 |
| `thinking.enabled` | boolean | 是否启用扩展思考模式 | ✓ |  |

#### 6.1 `effort.level` 枚举值

- `"low"` - 低推理  
- `"medium"` - 中推理（默认）  
- `"high"` - 高推理  
- `"xhigh"` - 极高推理（Ultracode 报告为 `xhigh`）  
- `"max"` - 最大推理  

> 注：值反映实时会话设置，支持 `/effort` 中途更改。

### 7. 速率限制 (`rate_limits`)

| 字段 | 类型 | 含义 | 始终存在 | 备注 |
|------|------|------|--------|-------|
| `rate_limits.five_hour.used_percentage` | number | 5 小时窗口已用百分比 (0-100) | ✗ | **缺失条件**: Claude.ai Pro/Max 用户，或首次 API 调用前 |
| `rate_limits.five_hour.resets_at` | number | 5 小时窗口重置时间（Unix 秒） | ✗ | 同上 |
| `rate_limits.seven_day.used_percentage` | number | 7 天窗口已用百分比 (0-100) | ✗ | 同上 |
| `rate_limits.seven_day.resets_at` | number | 7 天窗口重置时间（Unix 秒） | ✗ | 同上 |

> 每个窗口可独立缺失，使用 jq 条件访问：`.rate_limits.five_hour.used_percentage // empty`

### 8. 会话信息

| 字段 | 类型 | 含义 | 始终存在 | 备注 |
|------|------|------|--------|-------|
| `session_id` | string | 唯一会话标识符 | ✓ |  |
| `session_name` | string | 自定义会话名称 | ✗ | **缺失条件**: 未用 `--name` 或 `/rename` 设置 |
| `transcript_path` | string | 会话记录文件路径 | ✓ |  |

### 9. 版本与输出风格

| 字段 | 类型 | 含义 | 始终存在 |
|------|------|------|--------|
| `version` | string | Claude Code 版本（如 `"2.1.90"`) | ✓ |
| `output_style.name` | string | 当前输出风格名称 | ✓ |

### 10. Vim 模式

| 字段 | 类型 | 含义 | 始终存在 | 备注 |
|------|------|------|--------|-------|
| `vim.mode` | enum | 当前 vim 模式 | ✗ | **缺失条件**: vim 模式未启用 |

#### 10.1 `vim.mode` 枚举值

- `"NORMAL"` - 普通模式  
- `"INSERT"` - 插入模式  
- `"VISUAL"` - 可视模式  
- `"VISUAL LINE"` - 行可视模式  

### 11. Agent 信息

| 字段 | 类型 | 含义 | 始终存在 | 备注 |
|------|------|------|--------|-------|
| `agent.name` | string | agent 名称 | ✗ | **缺失条件**: 未用 `--agent` 或 agent settings 配置 |

### 12. 拉取请求信息 (`pr`)

| 字段 | 类型 | 含义 | 始终存在 | 备注 |
|------|------|------|--------|-------|
| `pr.number` | number | PR 编号 | ✗ | **缺失条件**: 无开放 PR，不在 git 仓库，或 PR 已合并/关闭 |
| `pr.url` | string | PR 链接 | ✗ | 同上 |
| `pr.review_state` | enum | PR 审查状态 | ✗ | 可独立缺失，即使 `pr` 存在 |

#### 12.1 `pr.review_state` 枚举值

- `"approved"` - 已批准  
- `"pending"` - 待审查  
- `"changes_requested"` - 请求变更  
- `"draft"` - 草稿  

### 13. Worktree 信息 (`worktree`)

| 字段 | 类型 | 含义 | 始终存在 | 备注 |
|------|------|------|--------|-------|
| `worktree.name` | string | Worktree 名称 | ✗ | **缺失条件**: 非 `--worktree` 会话 |
| `worktree.path` | string | Worktree 绝对路径 | ✗ | 同上 |
| `worktree.branch` | string | Worktree Git 分支名 | ✗ | 同上；hook-based worktree 缺失 |
| `worktree.original_cwd` | string | 进入 worktree 前的目录 | ✗ | 同上 |
| `worktree.original_branch` | string | 进入 worktree 前的分支 | ✗ | 同上；hook-based worktree 缺失 |

---

## 二、原子化分解建议

### 当前"组合"字段 → 建议原子化

#### 1. **Cost 群组** — 拆为 3 个独立段

**当前组合**：`cost.total_cost_usd` + `cost.total_duration_ms` + `cost.total_api_duration_ms`

**建议原子段**：

| 段名 | 提取字段 | 含义 |
|------|---------|------|
| `cost` | `cost.total_cost_usd` | 估算成本（USD） |
| `duration` | `cost.total_duration_ms` | 会话总耗时 |
| `api_duration` | `cost.total_api_duration_ms` | API 等待时间 |

**为何拆分**：成本、总时间、API 时间是三个独立的监控维度，用户可能只关心某个，分段让 statusline 设计更灵活。

---

#### 2. **Lines Changed** — 独立为单段或合并为 `changes`

**当前**：`cost.total_lines_added` + `cost.total_lines_removed`

**建议**：

**方案A（保持独立）**：
- 段 `lines_added` → `cost.total_lines_added`  
- 段 `lines_removed` → `cost.total_lines_removed`  

**方案B（合并）**：
- 段 `changes` → `cost.total_lines_added` 和 `cost.total_lines_removed` 合算（+N-M 或 Δ+N-M）

**推荐**：方案 B（合并），因为这两个字段通常一起展示。

---

#### 3. **Context Window** — 拆为 4 段

**当前组合**：`context_window.used_percentage` + `total_input_tokens` + `total_output_tokens` + `context_window_size`

**建议原子段**：

| 段名 | 提取字段 | 含义 | 原因 |
|------|---------|------|------|
| `context_pct` | `context_window.used_percentage` | 上下文使用百分比（0-100） | 快速进度条 |
| `context_tokens` | `total_input_tokens`, `total_output_tokens` | 输入/输出 token 数分解 | 细致 token 计数 |
| `context_max` | `context_window_size` | 最大窗口大小 | 上下文容量 |
| `context_cache` | `current_usage.cache_*_tokens` | 缓存写入/读取 token 数 | prompt cache 监控 |

---

#### 4. **Rate Limits** — 分窗口拆段

**当前组合**：两个窗口混在一起

**建议原子段**：

| 段名 | 提取字段 | 含义 |
|------|---------|------|
| `rate_limit_5h` | `rate_limits.five_hour.*` | 5 小时窗口（已用 % + 重置时间） |
| `rate_limit_7d` | `rate_limits.seven_day.*` | 7 天窗口（已用 % + 重置时间） |

**原因**：两个窗口独立重置时间，应分开展示。

---

#### 5. **Git 信息** — 从 `workspace.repo` 拆为 3 段

**当前**：`workspace.repo` 嵌套对象

**建议原子段**：

| 段名 | 提取字段 | 含义 |
|------|---------|------|
| `git_host` | `workspace.repo.host` | Git 主机（github.com 等） |
| `git_owner` | `workspace.repo.owner` | 仓库所有者 |
| `git_repo` | `workspace.repo.name` | 仓库名 |

**可选合并**：`git_repo_full` = `${owner}/${name}`

---

#### 6. **Worktree 信息** — 分情境拆段

**当前**：`worktree.*` 组合

**建议原子段**：

| 段名 | 提取字段 | 用途 |
|------|---------|------|
| `worktree_name` | `worktree.name` | Worktree 标识 |
| `worktree_branch` | `worktree.branch` | 当前工作分支 |
| `worktree_original_branch` | `worktree.original_branch` | 回源分支 |

---

#### 7. **其他单字段** — 已原子，无需拆分

- `model.display_name` → 段 `model`  
- `cwd` / `workspace.current_dir` → 段 `cwd`  
- `session_id` → 段 `session_id`  
- `pr.number`, `pr.url`, `pr.review_state` → 段 `pr`  
- `vim.mode` → 段 `vim_mode`  
- `agent.name` → 段 `agent`  
- `effort.level` → 段 `effort`  
- `thinking.enabled` → 段 `thinking`  
- `exceeds_200k_tokens` → 段 `token_warn`  

---

## 三、缺失字段处理

| 条件 | 字段 | 处理方式 |
|------|------|---------|
| 早期会话（第一次 API 调用前） | `context_window.used_percentage`, `context_window.remaining_percentage`, `context_window.current_usage`, `rate_limits.*` | 设默认值或隐藏段 |
| `/compact` 后至下次 API 调用 | `context_window.current_usage` | 为 `null`，需条件判断 |
| 非 git 仓库 | `workspace.repo.*` | 整个对象缺失 |
| 无开放 PR | `pr.*` | 整个对象缺失 |
| 非 `--worktree` 会话 | `worktree.*` | 整个对象缺失 |
| 模型不支持 effort | `effort.*` | 缺失（如 Haiku） |
| 非 Claude.ai Pro/Max 用户 | `rate_limits.*` | 缺失 |
| vim 未启用 | `vim.mode` | 缺失 |

**推荐处理**：在段定义中统一用 jq 条件：
```bash
jq -r '.context_window.used_percentage // "-" | tostring'
```

---

## 四、源引用

- **官方文档**：https://code.claude.com/docs/en/statusline
  - 字段表：`## Available data`  
  - JSON schema：`Full JSON schema` 折叠框（行 42-146）  
  - Context window 细节：`### Context window fields`  
- **文档版本**：Claude Code v2.1.90+
- **关键版本变更**：
  - v2.1.132：`total_input_tokens`, `total_output_tokens` 从累积改为当前值  

---

## 五、实现参考

### 结构化段编辑器输入格式示例

```typescript
interface StatusLineSegment {
  name: string;           // 段唯一标识（如 "cost", "context_pct"）
  label?: string;         // 显示标签
  jq_expr: string;        // jq 提取表达式
  format?: string;        // 输出格式（如 "%.2f", "ms", "%")
  condition?: string;     // jq 条件表达式（缺失时不显示）
  separator?: string;     // 段间分隔符
}
```

### 具体例

```json
[
  {
    "name": "model",
    "jq_expr": ".model.display_name",
    "separator": " • "
  },
  {
    "name": "context_pct",
    "label": "ctx",
    "jq_expr": ".context_window.used_percentage",
    "format": "%d%%",
    "condition": ".context_window.used_percentage != null"
  },
  {
    "name": "cost",
    "label": "$",
    "jq_expr": ".cost.total_cost_usd",
    "format": "%.4f"
  },
  {
    "name": "git_repo_full",
    "jq_expr": "\"\\(.workspace.repo.owner)/\\(.workspace.repo.name)\"",
    "condition": ".workspace.repo",
    "separator": " "
  }
]
```

---

## 附录：完整 JSON 示例

```json
{
  "cwd": "/Users/luoxin/persons/lyxamour/aidog",
  "session_id": "abc123xyz789",
  "session_name": "statusline-atoms",
  "transcript_path": "/Users/luoxin/.claude/projects/aidog/session.jsonl",
  "model": {
    "id": "claude-haiku-4-5-20251001",
    "display_name": "Haiku 4.5"
  },
  "workspace": {
    "current_dir": "/Users/luoxin/persons/lyxamour/aidog",
    "project_dir": "/Users/luoxin/persons/lyxamour/aidog",
    "added_dirs": [],
    "repo": {
      "host": "github.com",
      "owner": "luoxin",
      "name": "aidog"
    }
  },
  "version": "2.1.90",
  "output_style": {
    "name": "default"
  },
  "cost": {
    "total_cost_usd": 0.08234,
    "total_duration_ms": 285000,
    "total_api_duration_ms": 15300,
    "total_lines_added": 412,
    "total_lines_removed": 87
  },
  "context_window": {
    "total_input_tokens": 89500,
    "total_output_tokens": 12400,
    "context_window_size": 200000,
    "used_percentage": 51,
    "remaining_percentage": 49,
    "current_usage": {
      "input_tokens": 45000,
      "output_tokens": 12400,
      "cache_creation_input_tokens": 20000,
      "cache_read_input_tokens": 12100
    }
  },
  "exceeds_200k_tokens": false,
  "thinking": {
    "enabled": false
  },
  "rate_limits": {
    "five_hour": {
      "used_percentage": 34.2,
      "resets_at": 1718186400
    },
    "seven_day": {
      "used_percentage": 62.5,
      "resets_at": 1718745600
    }
  }
}
```

