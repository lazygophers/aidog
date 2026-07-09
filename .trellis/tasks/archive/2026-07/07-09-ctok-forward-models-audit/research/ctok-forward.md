# Research: ctok 聚合转发站（CTok.ai）— 全协议完整性审计

- **Query**: 核实 ctok 全协议（anthropic/openai/gemini）模型清单完整性 + base_url 准确性 + models slot 映射合理性，产出 diff 表 + 补齐建议，不改 JSON
- **Scope**: mixed（内部 JSON 现状 + 外部官方/实测核验）
- **Date**: 2026-07-09

## 1. 现有态（platform-presets.json ctok 条目完整抄录）

> 真值源：`src-tauri/defaults/platform-presets.json` `protocols.ctok`（worktree 当前 HEAD，ST7 已 apply）

### endpoints.default（三协议 + client_type）

| protocol | base_url | client_type |
|---|---|---|
| anthropic | `https://api.ctok.ai` | `claude_code` |
| openai | `https://api.ctok.ai/v1` | `codex_tui` |
| gemini | `https://api.ctok.ai` | `default` |

顶层 `client_type: "default"`。

### models.default（slot 映射）

| slot | 值 |
|---|---|
| `default` | `claude-opus-4-8`（ST7 补） |
| `opus` | `claude-opus-4-8` |
| `sonnet` | `claude-sonnet-4-6` |
| `haiku` | `claude-haiku-4-5` |

### model_list.default（全集，8 项）

```
claude-opus-4-8, claude-sonnet-5, claude-sonnet-4-6, claude-haiku-4-5,
claude-opus-4-7, claude-opus-4-6, claude-opus-4-5, claude-sonnet-4-5
```

### 其他元数据

- `homepage`: `https://ctok.ai`
- `source_urls.docs`: `https://api.ctok.ai/`
- `source_urls.pricing`: `https://api.ctok.ai/`
- `logo_url`: 空

## 2. 官方 source（WebFetch / curl 实测，每条标 URL + 状态）

### 2.1 CTok 主站（Cloudflare 拦截，需 JS challenge）

| 候选 URL | 实测状态 | 备注 |
|---|---|---|
| `https://ctok.ai/` | **HTTP 403**（`cf-mitigated: challenge`，Cloudflare "Just a moment"） | curl/UA 伪装均无法绕过，需真实浏览器 |
| `https://ctok.ai/docs` | HTTP 403 | 同上 |
| `https://docs.ctok.ai/` | HTTP 403 | 同上 |

> ctok.ai 域名是 **VitePress 教程站**（标题 `CTok | Claude Code 教程与资源` / `Claude Code 拼车社群`），非 API 文档站。绕 CF 后内容是 Claude Code 安装/拼车教程，无静态模型清单。

### 2.2 Wayback Machine 命中（绕过 CF，原始内容可读）

通过 CDX API（`https://web.archive.org/cdx/search/cdx?url=ctok.ai/*`）查得 49 个捕获，仅 2 个内容页有静态文本：

| 归档 URL | 状态 | 关键信息 |
|---|---|---|
| `https://web.archive.org/web/20251105153240id_/https://ctok.ai/claude-code-setup-ctok` | HTTP 200 / 66 KB | **CTok 官方接入教程**（捕获于 2025-11-05） |
| `https://web.archive.org/web/2id_/https://ctok.ai/claude-code-carpool-guide.html` | HTTP 200 / 22 KB | CTok 拼车产品介绍 |

**关键摘录（setup-ctok 页，唯一公开的接入文档）**：

> 获取 Auth Token：`ANTHROPIC_AUTH_TOKEN`：会单独发给你，类似格式：`"cr_..."`
> API 地址：`ANTHROPIC_BASE_URL`：给你的连接点 url 是本站的 API 服务地址，与主站地址相同

**关键摘录（carpool-guide 页）**：

> 高速专线服务器：必须使用高速专线服务器，网络配置要优化，直接将请求转发到 Anthropic 官方 endpoint

**结论**：CTok 官方教程**只文档化 Anthropic 协议**（`ANTHROPIC_AUTH_TOKEN` + `ANTHROPIC_BASE_URL`），且 base_url 是**用户专属分配**（"给你的连接点 url"），非单一公开 URL。CTok 自我定位为 **Claude Code 拼车（group-buy）转发服务**，token 格式 `cr_...` 为自研中转令牌（非真实 Anthropic `sk-ant-`）。**官方教程完全未提及 OpenAI / Gemini 协议**。

### 2.3 api.ctok.ai 实测（OneAPI/NewAPI fork 后端，三协议 live 探测）

通过 curl 直接命中 `api.ctok.ai`（此域无 CF，SPA + JSON API），确认三协议端点**实际存活**：

| 端点 | 方法 | 实测响应 | 结论 |
|---|---|---|---|
| `/v1/messages`（Anthropic） | POST + 假 key | `401 {"code":"INVALID_API_KEY"}` | ✅ 路由存在 |
| `/v1/chat/completions`（OpenAI） | POST + 假 key | `401 {"code":"INVALID_API_KEY"}` | ✅ 路由存在 |
| `/v1beta/models`（Gemini） | GET 无 key | `401 {"error":{"code":401,"message":"API key is required","status":"UNAUTHENTICATED"}}` | ✅ 路由存在（Google 风格错误体） |
| `/v1/models`（OpenAI 模型列表） | GET 无 key | `401 {"code":"API_KEY_REQUIRED","message":"API key is required in Authorization header (Bearer scheme), x-api-key header, or x-goog-api-key header"}` | ✅ 路由存在，**需有效 key 才能拉清单** |

> 认证支持 `Authorization: Bearer`、`x-api-key`、`x-goog-api-key` 三种头（统一鉴权层）。

### 2.4 公开模型清单 — 不可获取

| 尝试 | 结果 |
|---|---|
| `api.ctok.ai/` 根 SPA HTML（44 KB） | 仅含 `<title>CTok.ai - AI API Gateway</title>` + SPA 引导脚本，**无静态模型枚举** |
| `api.ctok.ai/assets/index-CYH4G1TS.js`（162 KB）+ 3 vendor chunk | grep `claude-*/gpt-*/gemini-*` 均 0 命中 — **模型动态从管理后台 API 加载**（典型 OneAPI/NewAPI 行为） |
| `/announcements` `/settings/public` `/pricing` `/docs` | 全部返回相同 44 KB SPA 壳（路由前端处理，无静态数据） |
| `/api/models` `/api/v1/models` `/api/v1/notice` `/api/setting` `/api/option`（NewAPI 常见公开端点） | 全部 **404**（CTok 后端不暴露无鉴权公开模型 API） |

**`需要: CTok 有效 API key 调用 `GET https://api.ctok.ai/v1/models` 拉真实清单**。或用户提供管理后台模型定价页截图。

## 3. Diff 表（现有 JSON vs 官方/实测）

### 3.1 base_url（三协议） — **全部正确**

| 协议 | 现状 base_url | 项目 URL 拼接规则 | 实测最终 URL | 判定 |
|---|---|---|---|---|
| anthropic | `https://api.ctok.ai` | converter path `/v1/messages` 硬编码 → `base_url + /v1/messages` | `https://api.ctok.ai/v1/messages` → 401 INVALID_API_KEY（路由存在） | ✅ 维持 |
| openai | `https://api.ctok.ai/v1` | `provider_api_path` 仅返 `/chat/completions`，`base_url` 须含版本前缀（项目硬规） | `https://api.ctok.ai/v1/chat/completions` → 401（路由存在） | ✅ 维持 |
| gemini | `https://api.ctok.ai` | converter 硬编码 `/v1beta/models/{model}:streamGenerateContent` → `base_url + /v1beta/...` | `https://api.ctok.ai/v1beta/models` → 401 UNAUTHENTICATED（路由存在） | ✅ 维持（与官方 gemini `https://generativelanguage.googleapis.com` 同模式，host 根 + converter 拼 /v1beta） |

> 三协议 base_url **与项目 URL 构造约定（CLAUDE.md「URL 构造」节）完全一致**，且与同类转发站（aihubmix/therouter/cherryin/cubence/aigocode 等均用 host 根作 gemini base_url）对齐。

### 3.2 model_list 遗漏（官方有 / 现无）

| 模型 | 是否应补 | source / 依据 |
|---|---|---|
| `claude-fable-5` | **可选补** | 上游 Anthropic 已发（见 `research/anthropic.md`）；同类 claudeapi / runapi 转发站已纳入；CTok 是否实际转发未知 |
| `claude-mythos-5` | 可选补 | 同上（anthropic.md 标记为官方新模型） |
| `claude-opus-4-5-20251101`（带日期版） | 不补 | 现用 alias `claude-opus-4-5`，与同类转发站对齐 |
| GPT 系（`gpt-5.5` 等） | **不建议补** | CTok 官方教程只推 Claude；OneAPI 后端虽支持 openai 协议但**无证据表明 CTok 实际开 GPT 渠道**。补了是臆测 |
| Gemini 系（`gemini-3-flash` 等） | **不建议补** | 同上；CTok 定位是 Claude Code 拼车，非多模型聚合 |

### 3.3 model_list 臆造（现有官方无） — **无臆造**

逐项对照 `research/anthropic.md` 官方清单，CTok 现有 8 个 id 全部是 Anthropic 真实模型（无日期 alias 形式，与项目惯例一致）。**无 gemini-3.5-flash 类臆造 id**。

### 3.4 model_list 与同类 Claude 转发站横向对比（佐证）

| 转发站 | model_list（Claude 部分） |
|---|---|
| apikeyfun / claudecn / micu / relaxycode / compshare_coding / aicodemirror | 标准 7 项：opus-4-5/4-6/4-7/4-8 + sonnet-4-5/4-6 + haiku-4-5 |
| **ctok（现状）** | 标准 7 项 **+ claude-sonnet-5**（ST7 补）= 8 项 |
| claudeapi / runapi | 标准 7 + sonnet-5 + fable-5（+ runapi 多 sonnet-4-6-thinking） |

> CTok 现状与「标准 7 + sonnet-5」对齐，比头部转发站少 fable-5。补不补取决于 CTok 是否开通 fable-5 渠道，**需 key 实测**。

### 3.5 models slot 映射问题

| 项 | 现状 | 判定 |
|---|---|---|
| `default = claude-opus-4-8` | 与 opus slot 一致 | ✅ 合理（caller 取默认拿最高档，与 anthropic 官方 preset 一致） |
| opus / sonnet / haiku 三 slot 齐全 | 全有值 | ✅ 合理 |
| **无 `gpt` slot**（虽有 openai endpoint） | 现状 | ⚠️ **不对称**：endpoint 含 openai 协议但 models 无 gpt 映射；codex_tui client 走 openai 协议会拿到 model_list 第一个 = `claude-opus-4-8`（首项）。若 CTok 实际不开 GPT 渠道，**这是正确行为**（不应硬塞 gpt slot）；若开了则缺。**需 key 验证** |
| **无 gemini-default slot**（虽有 gemini endpoint） | 现状 | ⚠️ 同上不对称。gemini 协议无显式 slot 时走 model_list 首项 `claude-opus-4-8`，会向 gemini 端点发 Claude 模型 id — **若 CTok gemini 端点不识别 Claude 模型会 404**。`需要: CTok gemini 端点是否做模型名 alias 映射` |

## 4. 补齐建议（每条带 source）

### 高置信度（基于官方教程 + 实测）

1. **三协议 base_url 全部维持**（实测路由均 live，URL 拼接符合项目约定）。source: 本文 §2.3 实测 + §3.1 拼接推导。
2. **models.default 三 slot（opus/sonnet/haiku）+ default 维持**（与 anthropic 官方 preset 对齐）。source: `research/anthropic.md`。
3. **model_list 现有 8 项维持**（无臆造，对照 anthropic.md 官方清单）。source: §3.3。

### 中置信度（官方未明示，建议但非阻塞）

4. **可选补 `claude-fable-5`**（与头部转发站 claudeapi/runapi 对齐，上游已发）。source: `research/anthropic.md` + 同类 preset 对比。**CTok 是否实际开此渠道需 key 验证**。
5. **不补 GPT / Gemini 模型到 model_list**（CTok 官方教程只文档化 Claude；OneAPI 后端虽支持但无证据开非-Claude 渠道；补了违反"禁臆造 id"）。source: §2.2 官方教程。

### 需 main 决策（涉及不对称）

6. **openai/gemini endpoint 是否保留**：CTok 官方只推 Anthropic，但后端三协议均 live。建议**保留**（OneAPI 后端确实支持，用户若有 GPT/Gemini key 可用；删了反而缩减能力）；但应在 `desc` 注明"主推 Claude，OpenAI/Gemini 协议可用性以实际账号权限为准"。source: §2.3 实测。

### source_urls 元数据改进（低优先级）

7. `source_urls.docs` / `source_urls.pricing` 现状均为 `https://api.ctok.ai/`（指向 SPA 根，无文档价值）。建议：
   - docs 改 `https://ctok.ai/claude-code-setup-ctok`（官方教程，需 CF 通过）
   - pricing 维持或清空（CTok 拼车定价不公开，私域运营）
   source: §2.1 + §2.2。

## 5. Caveats / `需要:` 标记

- **`需要: CTok 有效 API key`** 调用 `GET https://api.ctok.ai/v1/models` 拉真实模型清单（含 Claude / GPT / Gemini 实际开通渠道）。无 key 无法 100% 验证 model_list 完整性。
- **`需要: 用户确认 CTok gemini 端点行为`**：CTok 现状 model_list 全是 Claude id，gemini 端点 + claude id 组合的行为未知（是否做 alias 映射 / 直接 404）。
- **`需要: CTok 是否给每个用户分配独立子域/路径`**：官方教程说"给你的连接点 url"，暗示可能有 per-user endpoint。preset 用通用 `api.ctok.ai` 可能只是默认入口。
- **CTok 是 Claude Code 拼车（group-buy）服务**，非通用聚合 API：模型清单**随车队 / 上游动态变化**，preset 应理解为"最佳effort 快照"。
- **CF 拦截阻断直读官方教程**：本研究的官方教程内容来自 Wayback 2025-11-05 快照，若 CTok 之后更新文档需复测。
- **无 mcp__exa / WebFetch 工具**（本会话仅 Read/Write/Bash/Skill 可用），外部搜索全靠 curl + Wayback；DuckDuckGo/Bing HTML 搜索均返回 JS 壳无结果。
