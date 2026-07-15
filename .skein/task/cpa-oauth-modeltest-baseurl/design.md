# CPA OAuth base_url 回填 — 设计

## 根因链
```
parse_oauth_json (parser.rs:546-556) 硬编码 base_url: String::new()
  → mapper.rs:61 透传 base_url: p.base_url（空）
  → cpa_import.rs:100 apply 透传（空）
  → DB platform.base_url = "" + endpoints = []
  → model_test.rs:94 url = format!("{}{}", "", "/v1/responses") = "/v1/responses"（无 host）
  → reqwest builder error（无 host）→ client 502
  → forward.rs:75-77,231-232 同构（代理路径同等 bug）
```

## 修复

### A. OAuth base_url 回填（parser + mapper）
CpaOAuthType 加方法：
```rust
impl CpaOAuthType {
    /// OAuth provider 静态上游 base_url（不含 path，path 在 converter）。
    /// None = 需用户填（Vertex region-specific / 未覆盖 type）。
    pub fn default_base_url(&self) -> Option<&'static str> {
        match self {
            Self::Xai => Some("https://api.x.ai"),
            Self::Aistudio => Some("https://generativelanguage.googleapis.com/v1beta"),
            Self::Antigravity => Some("https://cloudcode-pa.googleapis.com"),
            Self::Vertex | Self::Codex | Self::Claude | Self::Kimi => None,
        }
    }
}
```
接入点（二选一，s1 定）：
- **parser.rs::parse_oauth_json**（line 546-556）：构造 CpaProvider 时 `base_url: oauth_type.default_base_url().unwrap_or_default()`——最早，CpaProvider 完整，mapper 透传即可（推荐，单一真值）
- 或 mapper.rs::map_provider：OAuth 分支回填——晚一层，CpaProvider 仍空

base_url 格式约束：
- 不含 path（path 在 converter/request.rs:38-43 CpaGrok `/v1/responses`，重复则 `/v1/v1/responses`）
- Aistudio base_url 含 `/v1beta`：因 gemini adapter path 是 `:generateContent`（无 version 前缀），version 编码在 base_url——s1 读 gemini adapter 确认现状（gemini-api-key 协议 base_url 现状）

### B. base_url 空 guard（两处对称）
**model_test.rs::prepare_http_request**（line 79-95）：
```rust
let base_url = endpoints.first().map(|e| e.base_url.as_str())
    .filter(|u| !u.is_empty())
    .or(|| (!platform.base_url.is_empty()).then(|| platform.base_url.as_str()));
let base_url = match base_url { Some(u) => u, None => return Err("base_url 缺失，请在 endpoints 配置上游地址".into()) };
```
**forward.rs**（line 75-77 + 231-232）：同构 guard，空时返 proxy_log blocked_reason='no_base_url' 或 upstream error 友好文案。

## 改动文件
1. `src-tauri/crates/aidog_core/src/gateway/cpa_import/parser.rs` — CpaOAuthType::default_base_url + parse_oauth_json 接入
2. `src-tauri/crates/aidog_core/src/gateway/cpa_import/mapper.rs` — 透传（若 parser 接入则无改动；若 mapper 接入则加 OAuth 分支）
3. `src-tauri/crates/commands_ai_tools/src/model_test.rs` — prepare_http_request guard
4. `src-tauri/crates/aidog_core/src/gateway/proxy/forward.rs` — 两处 guard

## 已入库数据修复 SQL
```sql
UPDATE platform SET base_url='https://api.x.ai' WHERE id=303;
```
（仅 303 cpa-grok；其他已入库 OAuth platform 用户自查）

## 不改
- 前端（Vertex 表单填 base_url 走现有 endpoints 区）
- cpa-import-multi-edit（另 task）
- cpa-* preset（platform-presets.json 补 cpa-* preset 归另 task；本 task 回填在 mapper/parser 层，不依赖 preset）

## 风险
- Aistudio base_url `/v1beta` 归属：需 s1 读 gemini adapter 确认 version 在 base_url 还是 path
- forward.rs 两处 guard 对称（流式/非流式），禁漏一处（参考 [[symmetric-body-cap]] 流式非流式对称教训）
