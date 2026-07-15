# CPA OAuth platform model-test 502 builder error — 根因调研

## 核心结论

OAuth provider 导入后 base_url 恒为空（`""`），cpa-* 协议在 platform-presets.json 无 preset 兜底，
model-test / 正常代理两条路径都用空 base_url 拼 URL → reqwest 收到无 scheme/host 的 path（如 `/v1/responses`）
→ reqwest 构建阶段失败 → 502 "builder error"。**model-test 与正常代理两条路径同等受影响，无差异。**

---

## 逐条回答

### Q1. CPA OAuth provider 导入时 base_url 为何空？

- `aidog_core/src/gateway/cpa_import/parser.rs:546-556` `parse_oauth_json`：**所有 OAuth 凭据硬编码 `base_url: String::new()`**（注释 "OAuth 平台 base_url 由后续映射确定"，但映射阶段并未确定）。
- OAuth 凭据 JSON 本身无 base_url 字段：`OAuthCredential` 仅 type/email/access_token/refresh_token/model_aliases（`parser.rs:507-522`）。上游 URL 是 per-OAuth-type 的隐式知识，不在凭据里。
- 映射器 `aidog_core/src/gateway/cpa_import/mapper.rs:61` `base_url: p.base_url` —— **直接透传空值，无回填**。
- apply `commands_platform/src/cpa_import.rs:100` `base_url: p.base_url` —— 同样透传，`endpoints: None`（line 105）。
- 最终入库的平台：`base_url=""`, `endpoints=[]`, `extra=""`，与现象一致。
- CpaGrok 的真实上游（CLIProxyAPI 原版 grok OAuth）：`https://api.x.ai` + `/v1/responses`
  （来源 `.skein/task/cpa-import/research/cpa-format-round2.md:12,20,35`）。

### Q2. model-test 取 base_url 的链路 + 空 base_url 处理

- `commands_ai_tools/src/model_test.rs:79-86` `prepare_http_request`：
  - endpoints 非空 → 取 `endpoints[0].base_url`（或 coding_plan 端点）。
  - endpoints 空 → 回退 `ctx.platform.base_url`（line 85）。
- line 94-95：`let base_url = target_base_url.trim_end_matches('/'); let url = format!("{}{}", base_url, api_path);`
- platform 303：endpoints=[] 且 base_url="" → `url = "" + "/v1/responses"` = `/v1/responses`（无 host）。
- **无任何 empty-base_url guard**（grep 确认 model_test.rs 内无 is_empty 检查）→ reqwest `client.post("/v1/responses")` 在 RequestBuilder 阶段 fail → 返 `Err(builder error)`。
- model_test.rs:301-321 捕获该 Err → 落 `proxy_log(upstream_status=0, user_status=502, body="upstream error: {e}")`，返 result.error="request failed: {e}"。
- model-test 直接读 DB platform（line 35 `get_platform`），**不查 platform-presets.json 默认 endpoint** —— 即便补了 preset 也救不了 model-test。

### Q3. CpaGrok 协议的 provider_api_path / OAuth 专用上游 URL

- `aidog_core/src/gateway/models/protocol.rs:162` `CpaGrok`（serde "cpa-grok"）。Protocol 枚举**无 provider_api_path 方法**。
- path 由 `adapter/converter/request.rs` 的 `convert_request` 按 wire 协议返回：
  - `convert_request` CpaGrok arm（`request.rs:38-43`）→ body=openai_responses 转换 + path=`/v1/responses`。
  - `passthrough_api_path`（`request.rs:83`）→ `/v1/responses`。
- **无 OAuth 专用上游 URL 解析**：host 必须来自 base_url。协议层只产 path，不产 host。

### Q4. 正常代理请求对 OAuth platform 会怎样？

- **同等 builder error**。`aidog_core/src/gateway/proxy/forward.rs:75-77` 与 model_test 同构：
  `matched_ep.map(|ep| (...ep.base_url...)).unwrap_or((...route.platform.base_url...))`。
- forward.rs:231-232 `let url = format!("{}{}", base_url, api_path)` —— 同样空 base_url → 无 host URL → reqwest builder error → 502。
- router `candidates.rs` / `selection.rs` **不按 base_url 过滤候选**（grep `base_url` 在 router/ 仅测试文件命中，无过滤逻辑）。
- 即单平台组（含一个 cpa-grok OAuth 平台）走代理路由：候选被选中 → forward_attempt → builder error → 502 → 触发熔断/退避。
- **结论**：proxy 路径与 model-test 路径**同样坏**，proxy 路径**没有** OAuth base_url 解析。

---

## 修复方向（选项，不拍板）

### A. mapper 按 OAuth type 回填静态 base_url（根因修复，最小 diff，推荐主线）
- 改 `mapper.rs::map_provider`：OAuth 分支按 `oauth_type` 回填：
  - Xai (CpaGrok) → `https://api.x.ai`
  - Aistudio (CpaAistudio) → `https://generativelanguage.googleapis.com/v1beta`
  - Vertex (CpaVertex) → region-specific（用户预览补全，留空也合理）
  - Antigravity (CpaAntigravity) → `https://cloudcode-pa.googleapis.com`
  - Claude/Codex/Kimi → 原生协议已有各自 base_url 语义，但 OAuth 段也空 → 同样需回填各自官方 OAuth 端点
- 注意 base_url 与 path 的 `/v1` 归属：convert_request CpaGrok 返 `/v1/responses`，故 base_url 应为 `https://api.x.ai`（不带 /v1）。
- 仅影响新导入平台；**已入库的 platform 303 需手工改 base_url 或删后重导**。

### B. model-test + forward 双路径 empty-base_url guard（防御纵深）
- 在 `model_test.rs::prepare_http_request` 和 `forward.rs` 拼 url 前加：
  `if target_base_url.trim().is_empty() { return 友好错误 "base_url 缺失，无法请求上游" }`
- 不让 platform 可用，但把 reqwest builder error 换成可读错误。
- 两条路径共享此 gap，改一处不够，须改两处（违反则只修一半）。

### C. cpa-* 协议补 platform-presets.json 默认 endpoint
- 给 cpa-grok/cpa-aistudio/cpa-vertex/cpa-antigravity 加 preset entries（含 default endpoints）。
- **局限**：model-test 不查 preset（直读 DB platform），preset 默认 endpoint 救不了 model-test；
  仅前端 PlatformCard 展示会回填。要真正生效还需 model-test/forward 读 preset 兜底，surface 更大。

### D. 仅改错误文案
- 捕获 reqwest builder error 特征串，返 "上游 URL 无效（base_url 为空）"。
- 治标，platform 仍不可用。适合作为 A 的配套。

**建议主线**：A（根因）+ B（防御，两处）。已入库脏数据靠用户手工或重导。
**SPEC 候选**：CPA OAuth 导入必须按 oauth_type 回填静态上游 base_url，禁透传空值到 DB（落 core/recall，防 cpa-aistudio/vertex 等同族再踩）。
