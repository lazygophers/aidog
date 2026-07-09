# Research: MiniMax EN（国际版）Coding Plan / Token Plan 调研

- **Query**: 调研 MiniMax 国际版 Coding Plan/Token Plan 真实 API，填齐 preset JSON 6 字段，判断是否需新增 `minimax_en_coding` 独立协议（对标 glm_coding 模式）
- **Scope**: external（官方 platform.minimax.io 文档 + endpoint probe）
- **Date**: 2026-07-10

## 核心结论（先读）

**国际版与国内版结论一致：MiniMax Token Plan（原 Coding Plan）不是独立 endpoint 协议，不应新增 `minimax_en_coding` 独立协议。**

国际版 Token Plan 与按量付费 API **完全共用** `https://api.minimax.io/v1` + `/anthropic` 两套 base_url，无 `/coding/` 路径变体（endpoint probe 已验证 404），无 cp 独占模型（M3/M2.7 全平台共享），无独立鉴权 host。与 GLM Coding Plan 的「独立 `/api/coding/paas/v4` endpoint + cp 独占模型 + 高峰期 3x 倍率」5 维度全异模式本质不同。

**新增发现（国内版调研未覆盖）**：国际版文档明确把 **Subscription Key**（Token Plan 专用，不可与 API Key 互换）作为独立 key 概念，并公开 **quota 查询端点 `GET https://api.minimax.io/v1/token_plan/remains`**（Bearer 鉴权，cross-region 对称，国内 host `api.minimaxi.com` 同路径也存在）。这两个信息可用于未来增强 `minimax_en` 的 quota 集成，但不构成拆独立协议的理由。

## 调研字段（6 项填齐）

### 1. OpenAI base_url（Codex / chat completions）

- **值**：`https://api.minimax.io/v1`
- 来源：官方 Codex 配置文档 `base_url = "https://api.minimax.io/v1"`（platform.minimax.io/docs/token-plan/codex.md）
- 现有 preset 已用（`platform-presets.json:189`），值不变。
- **无 `/coding/` 变体**：endpoint probe `POST https://api.minimax.io/v1/coding/chat/completions` → **HTTP 404**（"404 page not found"）；对照组 `POST https://api.minimax.io/v1/chat/completions` → HTTP 401（"login fail: Please carry the API secret key"，证明路径存在仅缺鉴权）。结论：coding 子路径不存在。

### 2. Anthropic base_url（Claude Code）

- **值**：`https://api.minimax.io/anthropic`
- 来源：官方 Claude Code 文档明确「for international users, use `https://api.minimax.io/anthropic`; for users in China, use `https://api.minimaxi.com/anthropic`」。
- 现有 preset 已用（`platform-presets.json:190`），值不变。

### 3. cp 独占模型列表

- **无 cp 独占模型**。Token Plan 与按量付费共用同一模型列表，订阅期间用同一 quota 池。
- 当前主推：`MiniMax-M3`（旗舰，1M context，Claude Code 内别名 `MiniMax-M3[1m]` 启用 1M 上下文）
- 兼容旧：`MiniMax-M2.7` / `MiniMax-M2.7-highspeed` / `MiniMax-M2.5` 系列（preset `minimax_en` model_list 已收录）
- 官方 Codex 样例：`model = "MiniMax-M3"`，`model_context_window = 1000000`，`wire_api = "responses"`
- 官方 Claude Code 样例：sonnet/opus/haiku 全部映射到 `MiniMax-M3[1m]`

### 4. client_type

- **同时支持两类**（与普通版一致，无偏好）：
  - `claude_code`（anthropic 协议）
  - `codex_tui`（openai 协议，Codex desktop / `wire_api = "responses"` 走 Responses API）
- 与现有 `minimax_en` preset 标 `protocol=openai` + `client_type=codex_tui` 一致。

### 5. Key 入口 URL

国际版 **两类 key，不互通**（官方 FAQ 明确）：

| Key 类型 | 用途 | 申请入口 |
|---|---|---|
| **Subscription Key** | Token Plan 订阅 + Credits 包，按 5h/周 quota 扣减 | `https://platform.minimax.io/user-center/payment/token-plan` |
| **API Key**（pay-as-you-go） | 按量付费，扣账户余额 | `https://platform.minimax.io/user-center/basic-information/interface-key` |

调用 API 时两类 key 都走同一请求头字段：
- Codex：`experimental_bearer_token = "<key>"`
- Claude Code：`ANTHROPIC_AUTH_TOKEN = "<key>"`（Bearer）

文档原文：「The Subscription Key is not interchangeable with pay-as-you-go API Keys」—— 这是**账户计费层**的差异，**不是 endpoint/header 层**的差异。key 必须**区域对齐**：国际版 key 不能用于国内 host，反之亦然（官方 MCP README 警告）。

### 6. quota 查询

- **端点存在**：`GET https://api.minimax.io/v1/token_plan/remains`，header `Authorization: Bearer <Subscription Key>`
- 文档原文（FAQ）：
  ```bash
  curl --location 'https://www.minimax.io/v1/token_plan/remains' \
  --header 'Authorization: Bearer <API Key>' \
  --header 'Content-Type: application/json'
  ```
- **endpoint probe 验证**：
  - `https://api.minimax.io/v1/token_plan/remains`（intl api host） → HTTP 200，body `{"base_resp":{"status_code":1004,"status_msg":"login fail: Please carry the API secret key..."}}`（路径存在，仅缺鉴权）
  - `https://api.minimaxi.com/v1/token_plan/remains`（国内 api host） → HTTP 200，相同 1004 响应（cross-region 对称）
  - 文档示例 host `www.minimax.io` 与 `api.minimax.io` 均可达，aidog 实现建议统一用 `api.minimax.io`
- aidog `gateway/quota.rs` 目前**未对接** MiniMax；此端点为未来增强点（参文末「需要」）。

## 与 glm_coding 5 维度对比

| 维度 | glm_coding（独立协议范本） | minimax_en Token Plan | 是否同构 |
|---|---|---|---|
| base_url 路径 | 普通版 `/api/paas/v4` vs coding 版 `/api/coding/paas/v4`（多 `/coding/`） | `/v1` 一条路径，无 coding 变体（404 已验证） | ❌ 不同构 |
| 独占模型 | glm-5.2 / glm-5-turbo 等 cp 专属 | 无，M3/M2.7 全平台共享 | ❌ |
| key | 独立 Coding Plan key（独立 quota 池） | Subscription Key vs API Key（账户层差异，同 header 字段） | ⚠️ 部分相似（但 endpoint 层无差） |
| 计费 | cp 模型高峰期 **3x 倍率**（peak_hours） | 5h 滚动 + 周窗口 quota，**无倍率** | ❌ |
| quota 查询 | 无公开 quota API | `GET /v1/token_plan/remains` 公开（新增发现） | ❌（minimax 更透明） |

**5 维度 0 同构**——glm_coding 模板套不上 minimax_en。

## 结论：NO，不新增 `minimax_en_coding` 独立协议

### 理由

1. **路径不变**：endpoint probe 证明 `api.minimax.io/v1/coding/...` → 404，coding plan 与普通 plan 共用同一 base_url。
2. **模型不变**：无 cp 独占模型，M3 全平台共享。
3. **header 不变**：Subscription Key 与 API Key 走同一 `Authorization: Bearer` 字段。
4. **强新增会双显**：复制一份与 `minimax_en` 完全相同的 base_url/模型，正是 CLAUDE.md 2026-07-08 起 8 协议删 cp 分支要根治的冗余问题。违反「`injectProtocolHosts` 派生自单一真值，禁抄第二份」约束。

### 推荐方案（按优先级）

1. **保持现状**（强推荐）：`minimax_en` preset 已含 `coding_plan: false` flag（`platform-presets.json:189-190`）。用户买 Token Plan 订阅后，直接用现有 `minimax_en` 协议（同一 endpoint / 把 Subscription Key 填到现有 key 字段即可），quota 在账户侧自动生效。无需任何 preset 改动。
2. **endpoint 级 flag 手工启用**（与 CLAUDE.md kimi/minimax/... 旧机制一致）：用户级 `platform.extra` 或 default 端点对象内可手工把 `coding_plan` 改 true，UI 侧 PlatformCard 出 "Code" 徽标。无需新协议。
3. **未来增强**（独立于本任务）：在 `gateway/quota.rs` 加 MiniMax Token Plan 配额查询分支，调 `GET /v1/token_plan/remains`，前端 PlatformCard 显示 5h/周剩余 quota。这是「增量功能」而非「新协议」。

### 若坚持新增（不推荐，仅作对照）

值照抄 `minimax_en` 即可，仅 `is_coding_plan:true` + `coding_plan:true` flag 区分。**与 `minimax_en` 完全重复**，违反 CLAUDE.md 单一真值约束，不推荐。

## 来源（URL，10 条）

1. 官方 llms.txt 文档总索引：https://platform.minimax.io/docs/llms.txt
2. Token Plan Overview（"extends upon our former Coding Plan"，Subscription Key 概念）：https://platform.minimax.io/docs/token-plan/intro.md
3. Codex 接入（`base_url=https://api.minimax.io/v1`，`wire_api=responses`）：https://platform.minimax.io/docs/token-plan/codex.md
4. Claude Code 接入（intl/China host 对照 + 模型映射）：https://platform.minimax.io/docs/token-plan/claude-code.md
5. Token Plan Migration Guide（M2.7→M3 quota protection，无倍率）：https://platform.minimax.io/docs/token-plan/migration.md
6. Token Plan FAQs（quota 端点 + Subscription Key vs API Key + 5h/周窗口 + 动态限流）：https://platform.minimax.io/docs/token-plan/faq.md
7. Token Plan pricing（Plus $20 / Max $50 / Ultra $120）：https://platform.minimax.io/docs/guides/pricing-token-plan.md
8. Subscription Key 入口：https://platform.minimax.io/user-center/payment/token-plan
9. API Key 入口（pay-as-you-go）：https://platform.minimax.io/user-center/basic-information/interface-key
10. endpoint probe（自证）：
    - `POST https://api.minimax.io/v1/coding/chat/completions` → HTTP 404（路径不存在）
    - `POST https://api.minimax.io/v1/chat/completions` → HTTP 401（路径存在）
    - `GET https://api.minimax.io/v1/token_plan/remains` → HTTP 200 + `base_resp.status_code=1004`（quota 端点存在）
    - 国内 host `api.minimaxi.com/v1/token_plan/remains` → 同 1004（cross-region 对称）
11. 国内版兄弟调研（对照）：`.trellis/tasks/07-10-protocols-json-schema/research/minimax-coding.md`

## Caveats / 未决

- 推测: Subscription Key 与 API Key 在请求层仅 Bearer 字段同形，是否后续官方会在 Subscription Key 上加额外请求头（如 `X-Subscription-Key`）？本次 FAQ/codex/claude-code 文档未见，但不排除后续迭代。需关注 platform.minimax.io/docs/token-plan/faq 更新。
- 需要: 是否要在 aidog `gateway/quota.rs` 加 MiniMax Token Plan quota 查询分支？官方已公开 `GET /v1/token_plan/remains`（Bearer 鉴权），可在 PlatformCard 显示 5h/周剩余 quota。这是「UI 增强」而非「协议新增」，**与本任务（判断是否新增协议）正交**，需 main 转用户确认是否单独立项。
- 需要: 国际版 `platform.minimax.io` 文档站部分页面（如 `/document/Announcement`、`/document/Price`）为 SPA JS 渲染，curl 取不到正文；本调研绕开这些页面，全部用 `.md` 后缀的 docs 端点（platform.minimax.io/docs/<path>.md），可信度高。
- endpoint probe 仅验证路径存在性（404 vs 401 vs 200），未测真实 quota 响应正文（需真实 Subscription Key）。若要对接 quota API，建议 main 协调有真实 key 的用户验证响应 schema（参 qianfan-coding.md 同手法）。
