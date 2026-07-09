# Research: MiniMax Coding Plan 调研（国内版）

- **Query**: 调研 MiniMax Coding Plan（海螺 MiniMax 国内版）真实 API 信息，为新增 `minimax_coding` 独立协议提供数据支撑
- **Scope**: external（官方文档 + GitHub 官方仓库）
- **Date**: 2026-07-10

## 核心结论（先读）

**MiniMax 没有 GLM 那样的「coding plan 专属 API endpoint」。**

MiniMax 历史上有过 "Coding Plan" 概念，现已**升级并更名为 "Token Plan"**（订阅配额套餐）。Token Plan 是**纯计费/配额层**，与普通 API 共用同一套 endpoint / base_url / 模型，不存在 `/coding/` 这样的路径前缀（对比 GLM 普通版 `/api/paas/v4` vs coding 版 `/api/coding/paas/v4`）。鉴权用同一把 MiniMax API Key（国内版 `platform.minimaxi.com` 申请）。

→ **按 GLM Coding Plan 同模板新增 `minimax_coding` 独立协议，没有真实数据支撑。** 推荐方案见文末「失败处理 / 替代方案」。

## 调研字段

### base_url

Token Plan 与普通 API **完全相同**（无 `/coding/` 变体）：

| 客户端 | 国际版 | 国内版 |
|---|---|---|
| Claude Code (anthropic 协议) | `https://api.minimax.io/anthropic` | `https://api.minimaxi.com/anthropic` |
| Codex (openai 协议) | `https://api.minimax.io/v1` | `https://api.minimaxi.com/v1` |

国内版 host = `api.minimaxi.com`（多一个 `i`）。这是 aidog 现有 `minimax` 协议（`platform-presets.json:171`）已在用的 base_url。

来源：官方文档 `platform.minimax.io/docs/token-plan/claude-code` 与 `/docs/token-plan/codex`。

### provider_api_path

- openai 协议：`/chat/completions`（标准，base_url 已含 `/v1`）
- anthropic 协议：base_url 末尾即 `/anthropic`，其后走 Anthropic 标准消息路径

**没有任何 coding 专属 path 变体。** grep 官方文档全文（`llms.txt` 索引）无 `/coding/`、`coding/paas`、`coding/v1` 之类条目。

### cp 独占模型

**无 cp 独占模型。** Token Plan 与按量付费共用同一模型列表，订阅期间用同一把 quota。当前主推 coding 模型：

- `MiniMax-M3`（旗舰，1M context，Claude Code 内别名 `MiniMax-M3[1m]` 启用 1M 上下文）
- 兼容旧：`MiniMax-M2.7` / `MiniMax-M2.7-highspeed` / `MiniMax-M2.5` 系列（aidog `minimax` preset model_list 已收录）

官方 Codex 配置样例：`model = "MiniMax-M3"`，`model_context_window = 1000000`。Claude Code 配置：`ANTHROPIC_MODEL=MiniMax-M3[1m]`，sonnet/opus/haiku 全部映射到 `MiniMax-M3[1m]`。

### client_type

官方同时支持两类客户端（与普通版一致，无偏好）：
- `claude_code`（anthropic 协议）
- `codex_tui`（openai 协议，Codex desktop / `wire_api = "responses"`）

Codex 用的是 OpenAI **Responses API**（`wire_api = "responses"`），而非 chat completions —— 与 aidog 现有 minimax preset 标 `protocol=openai` + `client_type=codex_tui` 的设定一致。

### 鉴权

- 单一 **MiniMax API Key**（国内版在 `platform.minimaxi.com/user-center/basic-information/interface-key` 申请，国际版 `platform.minimax.io/...interface-key`）
- Claude Code 走 `ANTHROPIC_AUTH_TOKEN = <MINIMAX_API_KEY>`（Bearer）
- Codex 走 `experimental_bearer_token = "<MINIMAX_API_KEY>"`
- key 与 host 必须**区域对齐**，否则 `Invalid API key`（官方 MCP README 明确警告）
- 文档提到订阅用 "Subscription Key"，但调用 API 仍用同一 API Key（Subscription Key 是后台绑定套餐的概念，非请求头里的另一把 key）

### 与普通版区别

| 维度 | 普通 API（按量） | Token Plan（原 Coding Plan） |
|---|---|---|
| base_url | api.minimaxi.com/v1 / /anthropic | **完全相同** |
| API path | 相同 | **相同** |
| 模型列表 | 全部 M 系列 | **相同**（订阅期间享 quota） |
| 鉴权 | API Key | **相同 API Key**（账户绑订阅） |
| 计费 | 按 token 付费 | 月订阅（Plus $20 / Max $50 / Ultra $120 国际价）+ 5h 滚动配额窗口，超额走 Credits 包 |
| 配额 | 无 | 5-hour rolling + weekly windows，多模态/agent 共享同一池 |

**关键区别只在账户计费层，API 层完全无差异。** 这与 GLM Coding Plan（独立 `/api/coding/paas/v4` endpoint + cp 独占模型 + 高峰期 3x 倍率）的模式**本质不同**。

## 来源（URL）

- 官方 Coding Plan MCP（含区域 host 对照表 + 历史定位）：https://github.com/MiniMax-AI/MiniMax-Coding-Plan-MCP
- Token Plan 总览（"extends upon our former Coding Plan"）：https://platform.minimax.io/docs/token-plan/intro
- Claude Code 接入（含国内/国际 base_url 对照 + 模型映射）：https://platform.minimax.io/docs/token-plan/claude-code
- Codex 接入（`base_url=.../v1`，`wire_api=responses`）：https://platform.minimax.io/docs/token-plan/codex
- Token Plan 定价（Plus/Max/Ultra + Credits）：https://platform.minimax.io/docs/guides/pricing-token-plan.md
- 迁移指南（"Coding Plan → Token Plan"）：https://platform.minimax.io/docs/token-plan/migration
- 国内平台 key 申请：https://platform.minimaxi.com/user-center/basic-information/interface-key
- 文档总索引：https://platform.minimax.io/docs/llms.txt

## 结论（推荐字段值）

### 判断：不应新增 `minimax_coding` 独立协议

MiniMax 的 coding plan（现 Token Plan）**不是独立 endpoint 协议**，无法套用 GLM Coding Plan 的「独立 base_url + cp 独占模型」模板。强行加 `minimax_coding` 协议会**复制一份与 `minimax` 完全相同的 base_url/模型**，引入双显冗余（正是 CLAUDE.md 2026-07-08 起 8 协议删 cp 分支要根治的问题）。

### 替代方案（按优先级）

1. **保持现状**（推荐）：`minimax` preset 的 endpoints 已含 `coding_plan: false` flag（`platform-presets.json:171-172`）。用户若买了 Token Plan 订阅，直接用现有 `minimax` 协议（同一 endpoint / 同一 key）即可，quota 在账户侧自动生效。无需任何 preset 改动。
2. **走 endpoint 级 flag**（与 CLAUDE.md 记的 kimi/minimax/... 旧机制一致）：现有 preset 已满足，default 端点对象内可手工把 `coding_plan` 改 true 标记「此端点账户绑了订阅」，UI 侧 PlatformCard 出 "Code" 徽标。无需新协议。
3. **若坚持要独立协议**（不推荐）：值照抄 `minimax` 即可，但违反 CLAUDE.md 「`injectProtocolHosts` 派生自单一真值，禁抄第二份」约束。
   ```json
   "minimax_coding": {
     "is_coding_plan": true,
     "client_type": "codex_tui",
     "endpoints": {"default": [
       {"protocol":"openai","base_url":"https://api.minimaxi.com/v1","client_type":"codex_tui","coding_plan":true},
       {"protocol":"anthropic","base_url":"https://api.minimaxi.com/anthropic","client_type":"claude_code","coding_plan":true}
     ]},
     "models": {"default": {"default":"MiniMax-M3","opus":"MiniMax-M3","sonnet":"MiniMax-M3","haiku":"MiniMax-M3"}},
     "model_list": {"default": ["MiniMax-M3"]},
     ...
   }
   ```
   注意：此值与普通 `minimax` **完全重复**，仅 `is_coding_plan:true` + `coding_plan:true` flag 区分。

## Caveats / 未决

- 推测: Token Plan 是否会对 M3 在高峰期引入类似 GLM 的倍率？迁移指南只提"quota protection"，未提倍率。需关注：若官方后续加高峰倍率，应落到 `peak_hours` 字段（参照 `glm_coding` preset line 109-118 的结构）。
- 需要: 是否要给 Token Plan 订阅用户在 UI 上做配额查询？官方"Subscription Key + 5h 滚动窗口"模式目前 aidog 的 `quota.rs` 未对接 MiniMax —— 若需要，得另查 `platform.minimaxi.com` 是否有 quota query API（本次未调研，Token Plan 概览页未提及公开配额查询端点）。
- 国内版 `platform.minimaxi.com` 文档站是纯 SPA（JS 渲染），curl 取不到正文；以上国内版 base_url（`api.minimaxi.com`）来自国际版文档的明确中/国对照陈述 + 现有 preset 已用值，可信度高。
