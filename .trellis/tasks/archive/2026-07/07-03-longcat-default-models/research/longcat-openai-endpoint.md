# Research: Longcat openai 端点 + fetchModels 多协议回退先例

- **Query**: Longcat openai 端点实证 + 现有 fetchModels 多协议回退先例 + 401/403/404 错误处理现状
- **Scope**: mixed（外部实证探测 + 内部 grep/Read）
- **Date**: 2026-07-03
- **方法**: 直接 `curl` 探测 `api.longcat.chat` 各路径 + Grep `Platforms.tsx` / `model_fetch.rs` / `passthrough.rs` / 历史 task research

---

## §1 Longcat openai 端点实证

### 关键结论（curl 实测，2026-07-03）

**openai base_url 实证为 `https://api.longcat.chat/openai/v1`**（**非** PRD 推测的 `/v1`，`/v1` 路径全 404）。

| 探测路径（curl，无鉴权） | HTTP | 响应体（节选） | 解读 |
|---|---|---|---|
| `GET https://api.longcat.chat/v1/models` | **404** | `<html>...404 Not Found...openresty` | **PRD 推测的 `/v1` 不存在**（openresty 兜底 404）|
| `GET https://api.longcat.chat/v1/chat/completions` (POST) | **404** | 同上 openresty | `/v1/*` 整路径树不存在 |
| `GET https://api.longcat.chat/v1/messages` (POST) | **404** | 同上 | `/v1/messages`（Anthropic 风格根）也不在根 |
| `GET https://api.longcat.chat/openai/v1/models` | **401** | `{"error":{"code":"invalid_api_key","message":"missing_api_key","type":"authentication_error"}}` | **真·openai 端点存在**，仅缺鉴权 |
| `GET https://api.longcat.chat/openai/v1/models` + `Authorization: Bearer sk-test` | **401** | `{"error":{"code":"invalid_api_key","message":"incorrect api key","type":"authentication_error"}}` | 错误 key → 401（鉴权层确认存在）|
| `POST https://api.longcat.chat/openai/v1/chat/completions` (无鉴权) | **400** | （未抓 body） | chat 端点存在（路由命中才会 400，未命中是 404）|
| `GET https://api.longcat.chat/openai/v1` | **404** | `{"timestamp":"2026-07-03T...","status":404,"error":"Not Found","path":"/openai/v1"}` | spring-boot 兜底 404（注意：错误页是 spring 格式，404 page 直说 path）|
| `GET https://api.longcat.chat/openai/models` | **401** | 同 missing_api_key | `/openai` 也有路由（但标准 openai 客户端走 `/openai/v1`）|
| `GET https://api.longcat.chat/anthropic/v1/models` | **401** | 同 missing_api_key | anthropic 端点 `/anthropic` 下有 `/v1/models`（preset 平台可走 anthropic 协议拉模型，只要有 key）|
| `POST https://api.longcat.chat/anthropic/v1/messages` (无鉴权) | **400** | （未抓） | anthropic chat 端点确认存在 |
| `GET https://api.longcat.chat/openai/v1/files` | **404** | openresty | openai files 端点不存在（仅 chat/models）|

**事实链**:
- openai 端点 host = `api.longcat.chat`，路径前缀 = `/openai/v1`（**与 doubao/volces `ark.*` `/api/coding/v3` 同类：路径前缀非标准 `/v1`**）
- 鉴权 = openai 标准 `Authorization: Bearer <key>`（推测；apply_models_auth 已是此风格，preset 无需改鉴权头注入）
- chat 走 `/openai/v1/chat/completions`，models 走 `/openai/v1/models`
- `/v1/*` 整路径树不存在 → **PRD §目标 D3 推测 `https://api.longcat.chat/v1` 已被证伪**，应改为 `/openai/v1`

### 厂商背景（longcat.ai 落地页 meta）

- og:site_name = `LongCat AI`，title = `LongCat - AI Coding Agent`
- meta description = `1.6万亿总参大模型，训练全程由国产芯片完成`
- 域名 longcat.chat / longcat.ai 双站 200，资源 CDN: `s3plus.meituan.net/aigc-media-resources/longcat/`（**美团系**，sankuai = 美团旧名三快科技，platform docs 错误页 `com.sankuai.friday.longcat.platformdocs` 佐证）
- 错误页 stack: openresty（边缘） + spring-boot（应用层 404 JSON） → 后端为 Java spring 系

### Longcat 模型 id 清单 — 未拿到（SPA + 鉴权墙）

**状态**: **未拿到确切 model id 列表**。

证据:
- `longcat.ai/docs` 与 `longcat.chat/docs` 均为 SPA（React，首屏 HTML 全空壳，需 JS 执行）→ grep 不到 `LongCat-*` 字面量
- `longcat.chat/platform/docs/zh/models` 服务端返回 `ResourceNotFoundException`（`com.sankuai.webxy.staticproxy` 拒绝静态抓取，s3 静态资源 `/platform/docs/zh/models/index.html` 实际不存在）
- models 端点 `/openai/v1/models` 与 `/anthropic/v1/models` 均 401 鉴权墙，无 key 无法枚举
- 与 06-17 历史 research (`models-firstparty.md:143-148`) 同结论：「longcat.chat/platform/docs 与 longcat.ai/docs 全 JS-rendered」「社区常见 `LongCat-Flash-Chat` / `LongCat-Flash-Thinking` 但无官方静态来源佐证」

**用户实测命令**（PRD D3 / D4 落地后用平台 key 试拉）:
```bash
curl https://api.longcat.chat/openai/v1/models \
  -H "Authorization: Bearer <用户 Longcat API key>"
```
返回应为 `{"data":[{"id":"...","object":"model",...},...]}`（openai 标准格式，model_fetch.rs:109-121 已兼容）。

**已知/推测 model id**（仅参考，非静态来源佐证，**禁写入 Platforms.tsx 静态默认列表**）:
- 社区/历史 research 提及: `LongCat-Flash-Chat`、`LongCat-Flash-Thinking`（06-17 archive `.trellis/tasks/archive/2026-06/06-17-platform-model-list/research/models-firstparty.md:146`，标注「推测」）
- og description 提「1.6 万亿总参」→ 旗舰模型命名可能含此参数指向，但无具体 id

### 相关端点 URL 一览（PRD 落地用）

| 用途 | URL |
|---|---|
| Longcat openai base_url（**推荐**）| `https://api.longcat.chat/openai/v1` |
| Longcat openai chat（验证）| `https://api.longcat.chat/openai/v1/chat/completions` |
| Longcat openai models（验证）| `https://api.longcat.chat/openai/v1/models` |
| Longcat anthropic 端点（preset 现状）| `https://api.longcat.chat/anthropic` |
| Longcat anthropic models（验证：preset 端点也有 models）| `https://api.longcat.chat/anthropic/v1/models` |
| 官方文档（SPA，无静态）| `https://longcat.ai/docs` / `https://longcat.chat/docs` |

---

## §2 现有多协议 fetchModels 先例矩阵

### Platforms.tsx preset endpoint 多协议矩阵（Grep `Platforms.tsx:240-360`）

| 平台 | 协议数 | openaiEp 存在？ | openaiEp base_url | 备注 |
|---|---|---|---|---|
| doubao | 4 | 是（`/api/coding/v3`）+ openai_responses | `ark.cn-beijing.volces.com/api/coding/v3` | 多协议最全（含 responses）|
| byteplus | 1 | 否 | — | anthropic-only |
| qianfan (cp=true) | 2 | 是 | `qianfan.baidubce.com/v2/coding` | openai + anthropic |
| xiaomi_mimo | 2 | 是 | `token-plan-cn.xiaomimimo.com/v1` / `api.xiaomimimo.com/v1` | openai + anthropic |
| bailing | 1 | 否 | — | anthropic-only（tbox.cn）|
| **longcat** | **1** | **否** | — | **本任务目标平台，anthropic-only** |
| sensenova | 2 | 是 | `token.sensenova.cn/v1` | openai + anthropic |
| openrouter | 3 | 是 | `openrouter.ai/api/v1` | + gemini |
| siliconflow / _en | 1 | 否 | — | anthropic-only |
| aihubmix | 2 | 是 | `aihubmix.com/v1` | openai + anthropic |
| dmxapi | 2 | 是 | `dmxapi.cn/v1` | openai + anthropic |
| packycode / cubence / aigocode | 3 | 是 | `*/v1` | openai + anthropic + gemini |
| rightcode | 2 | 是 | `right.codes/codex/v1` | openai + anthropic |
| aicodemirror | 3 | 是 | `*/api/codex/backend-api/codex` | openai + anthropic + gemini |
| nvidia | 1 | 是 | `integrate.api.nvidia.com/v1` | openai-only |
| pateway / ccsub / apikeyfun / apinebula | 1 | 否 | — | anthropic-only |

**关键观察**:
- **17 个平台有 openaiEp**（`handleFetchModels:2390` 的 `endpoints.find(ep => ep.protocol === "openai")` 命中）→ 这些平台 openaiEp 优先逻辑已覆盖
- **9 个平台 anthropic-only 且无 openaiEp**（longcat / bailing / byteplus / siliconflow / siliconflow_en / modelscope / shengsuanyun / atlascloud / novita / therouter / cherryin / pateway / ccsub / apikeyfun / apinebula 等）→ 这些平台 fetchModels 走 `getPrimaryBaseUrl(protocol, endpoints)` 回退 anthropic 协议
- **本任务要改造的回退链，一旦上线会同时影响所有 9 个 anthropic-only 平台**（不仅 longcat）—— 设计须通用、向前兼容

### handleFetchModels (Platforms.tsx:2389-2409) 当前逻辑（已读）

```ts
const openaiEp = endpoints.find(ep => ep.protocol === "openai");
const fetchUrl = openaiEp?.base_url || getPrimaryBaseUrl(protocol, endpoints);
// ...
const fetchProtocol: Protocol = openaiEp ? "openai" : protocol;
const modelIds = await platformApi.fetchModels(fetchProtocol, fetchUrl, apiKey);
```

- **单协议单次**：openaiEp 有则用，无则主协议；无 404 回退、无 401/403 区分、无多 endpoint 循环
- 错误处理 (Platforms.tsx:2405-2407)：`catch (e) { setFetchError(e.toString()); }` —— `e` 是后端 `Err(String)` 字符串，无 status_code 透传
- i18n 渲染 (Platforms.tsx:3074-3078)：`fetchError` 直接渲染 `e.toString()`，无鉴权错误专用提示文案
- fetchEmpty 仅在返回空数组时触发（Platforms.tsx:2398-2400）→ 「拉到空列表」≠ 「401/404 拉不到」

### 历史 fetchModels 失败先例（Grep `.trellis/tasks/archive/`）

- **`06-17-platform-model-list/research/models-firstparty.md:143-148`**: longcat 当时即「未找到，留空靠 fetchModels」，且发现文档全 JS-rendered
- **`06-17-platform-model-list/research/models-aggregator.md:283-287`**: 3/12 聚合平台公开端点不可访问（siliconflow_en / therouter「全端点 404 / 需鉴权 / SPA」）→ 同类问题，**fetchModels 在 therouter 上也会失败**，但历史 research 未提及 fetchModels 自身报错行为
- **无任何 task 专门记录 fetchModels 404/401 回退改造**（Grep `404 回退` / `协议回退` 仅命中本任务 prd.md + 06-13 protocol-prefer-native-passthrough 的「原协议透传」研究，主题不同）→ **Longcat 是首个触发 fetchModels 多协议回退链需求的平台**

### build_models_url (passthrough.rs:331-338) 对各 protocol 构造的 URL

```rust
match protocol {
    Protocol::Anthropic => format!("{base}/v1/models"),    // → /anthropic/v1/models（401 有路由）
    Protocol::Bailian => format!("{base}/compatible-mode/v1/models"),
    _ => format!("{base}/models"),                          // openai → /openai/v1/models（401 有路由）
}
```

- **openai**：`base_url = https://api.longcat.chat/openai/v1` → `https://api.longcat.chat/openai/v1/models` ✅（实测 401 有路由）
- **anthropic**：`base_url = https://api.longcat.chat/anthropic` → `https://api.longcat.chat/anthropic/v1/models` ✅（实测 401 有路由）—— **现状 preset 也能拉到，只要 key 对**
- **若 PRD 推测 `/v1` 被采纳**：`https://api.longcat.chat/v1/models` → **404 openresty**（已证伪）

**结论**: longcat 两个协议的 models 端点**都存在且都需鉴权**，意味着：
- openaiEp 优先 + anthropic 回退 = 两次 401 都可能命中（鉴权对的话）
- 现状 anthropic-only preset 实际能拉到模型（前提 key 正确）→ 用户报「拉不到」可能是 **api_key 为空 / 错误**（401 → 后端吞 status → 前端显示 parse 错误），**而非 endpoint 不存在**

### apply_models_auth (passthrough.rs:343-357) 鉴权头注入

- Anthropic → `x-api-key` + `anthropic-version: 2023-06-01`
- openai/兼容 → `Authorization: Bearer <key>` + 叠加 `api-key` 头（小米 token-plan 要求，其他上游忽略）

---

## §3 401/403/404 错误处理现状 + 改造接入点

### 后端 model_fetch.rs 错误处理（已读全文 124 行）

**当前行为**（model_fetch.rs:94-107）:
```rust
let status = resp.status();
let body = resp.text().await.map_err(|e| format!("read body: {e}"))?;
tracing::info!(url = %url, %status, "fetch models response status");
let upstream_status = status.as_u16() as i32;
// 记 proxy_log（status 已落库 upstream_status_code 字段）
db::upsert_proxy_log(&db, make_log(upstream_status, upstream_status, &body, &url)).await;
let resp: Value = serde_json::from_str::<Value>(&body)   // ← 401/404 错误体也走这里
    .map_err(|e| format!("parse response: {e}"))?;        // ← 401 体是合法 JSON → 解析「成功」
// 然后取 resp["data"]，401 错误体无 data 字段 → 返回 Vec::new()（空）
```

**问题**:
1. **status code 不参与控制流** —— 401/403/404 全部继续走到 JSON 解析
2. **401 错误体 `{"error":{...}}` 是合法 JSON** → `serde_json` 不报错 → `resp["data"]` 取不到 → 返回空 `Vec<String>`（model_fetch.rs:109-121 `.unwrap_or_default()`）
3. **前端收到空数组** → `setFetchError(t("platform.fetchEmpty"))`（Platforms.tsx:2398-2400）—— **显示「未获取到可用模型」而非「鉴权失败」**，与 404 表现完全相同，用户无法区分

**改造接入点**（PRD D2）:

| 文件 | 行 | 改造 |
|---|---|---|
| `src-tauri/src/commands/model_fetch.rs` | **103-107** | 在 `serde_json::from_str` 前加 `if !status.is_success() { return Err(... 带状态码 ...); }` |
| `src-tauri/src/commands/model_fetch.rs` | **18-23** | 返回类型可能需从 `Result<Vec<String>, String>` 改为带 status_code 的结构（或 enum / 元组）|

**结构化错误方案对比**（供 design 决策 D2）:

| 方案 | 后端改动 | 前端改动 | 优劣 |
|---|---|---|---|
| (a) 后端 Err 带结构化 status: `Err(format!("HTTP {code}: {body}"))` 约定前缀 | model_fetch.rs:103 加一行 | 前端字符串 match | 最小改动，但字符串解析脆弱 |
| (b) 后端返 `Result<Vec<String>, FetchError>` 自定义 enum | 加 enum + serde | 前端 catch 按 type 分流 | 强类型，但 Tauri command Err 须为 serde-serializable |
| (c) 后端永远 Ok + 在返回值带 status: `Ok(FetchResult { models, status, error })` | 改返回类型 | 前端按 status 分支 | 破坏现有 api.ts 签名，影响 17 个 openaiEp 平台 |
| (d) **前端循环多协议调 fetchModels，各自 try/catch** | 不改后端，仍吞 status | 前端按错误字符串区分 + 多协议循环 | 后端零改动，但 401/404 区分仍需后端透传 status（绕回 a/b）|

### 前端 fetchError 渲染（Platforms.tsx:3074-3078）

```tsx
{fetchError && (
  <div style={{ fontSize: 12, color: "var(--danger, #e55)", padding: "2px 0" }}>
    {fetchError}
  </div>
)}
```

- 无鉴权专用文案，直接渲染后端错误字符串
- i18n 仅 `platform.fetchEmpty`（zh: "未获取到可用模型" / en: "No models found"，zh-CN.json:765 / en-US.json:766）+ `platform.fetchModels`（"一键获取" / "Fetch Models"，:766）
- **改造接入点**: 若要区分 401/403 专用文案，需新增 i18n key（如 `platform.fetchAuthError`）+ Platforms.tsx:3074 分支渲染

### proxy 主路径错误处理（与本任务无关但作对照）

`non_success.rs:68` 已有 `code == 401 || code == 403 || code == 402` 触发 `set_platform_auto_disabled` 的成熟语义（spec `platform-error-handling.md` C1）。**fetch-models 路径完全独立**（model_fetch.rs 不经 non_success.rs），不共享此逻辑。若要 fetchModels 也按相同码区分，须在 model_fetch.rs 内复制此判定（spec C1 仅约束 proxy 主路径，不约束 fetch-models）。

---

## §4 PRD 填充建议

### D3 — Longcat openai base_url

**`https://api.longcat.chat/openai/v1`**（实证，非 PRD 推测的 `/v1`）。

Platforms.tsx:269-271 preset 改为:
```ts
longcat: [
  { protocol: "anthropic", base_url: "https://api.longcat.chat/anthropic", client_type: "claude_code" },
  { protocol: "openai", base_url: "https://api.longcat.chat/openai/v1", client_type: "codex_tui" },  // 新增
],
```

参考同类多协议 preset（`xiaomi_mimo:259-265` / `sensenova:277-280` / `qianfan:253-258`，均为 anthropic + openai 双 endpoint）。

### D4 — 范围

**通用回退链**（非 longcat 专属）。改造影响 9 个 anthropic-only 平台 + 17 个 openaiEp 平台。Longcat 仅是触发例（PRD 推测正确）。

### D1 — 回退链实现层

**建议前端循环多协议调 fetchModels（方案 d）+ 后端透传 status（方案 a/b）混合**:
- 前端 `handleFetchModels`（Platforms.tsx:2389）改为按 `endpoints` 顺序循环（openaiEp 在前，主协议次之，其余 endpoint 兜底），每个协议 try 一次
- 后端 `model_fetch.rs:103` 加 `if !status.is_success()` 透传 status_code（方案 a 最小改动：`Err(format!("HTTP {code}: {body}"))`），前端按 `HTTP 401` / `HTTP 403` / `HTTP 404` 字符串分流
- **401/403 → 立即 break + setFetchError（鉴权专用文案）**
- **404 / 网络错 → continue 试下一协议**
- 全部协议 404 → setFetchError「拉不到模型」

### D2 — 401/403 透传

后端 `model_fetch.rs:103-107` 插入 status 判定（load-bearing file:line）:
```rust
if !status.is_success() {
    return Err(format!("HTTP {}: {}", status.as_u16(), body));
}
```
（最小改动方案 a；强类型方案 b 见 §3 表格）

### Longcat 静态默认模型列表 — 不补

reason: 模型 id 未拿到静态来源（SPA + 鉴权墙），硬编码会编造。维持 `getDefaultModels` longcat 槽位为空（`Platforms.tsx:544` 注释 `// bailing / longcat: 官方模型文档无静态来源，留空靠 fetchModels` 保持正确），回退链修好后 fetchModels 实拉即可。

---

## Caveats / Not Found

- **Longcat 模型 id 清单未拿到**（SPA + 鉴权墙）→ 已给用户实测 curl 命令，**禁硬编码**社区推测的 `LongCat-Flash-Chat` 等进静态默认列表
- **openaiEp 鉴权风格仅推测为 `Authorization: Bearer`**（未用真 key 实测）—— 但 apply_models_auth 对 openai 协议默认注入 `Bearer` + `api-key` 双头（passthrough.rs:353-356），与 doubao/qianfan/xiaomi_mimo 同款 openaiEp 一致，**无需为 Longcat 特化**
- **401 错误体 `{"error":{"code":"invalid_api_key",...}}` 是合法 JSON**（实测）→ 当前 model_fetch.rs 不会因 JSON 解析失败报错，而是返回空数组 → **这是用户报「拉不到模型」的可能根因之一**（401 被吞成空）
- **openai base_url `/openai/v1` 是 spring-boot 路由**（404 page JSON 格式佐证），与 openresty 兜底的 `/v1/*` 404 不同 → 路径稳定性依赖后端 spring 路由表，月级稳定性未长期观测
- **未实测 `openai_responses` 端点**（`/openai/v1/responses`）—— Codex TUI client_type 用 responses 协议，若 Longcat 不支持 responses 仅支持 chat completions，preset 配 `client_type: "codex_tui"` 可能仍走 chat completions（需后续验证，本任务 scope 外）
