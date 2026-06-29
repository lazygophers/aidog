# fix GET /proxy/models 返回 404

## Goal

GET `/models` / `/v1/models` 探测当前在 `resolve_group`（handler.rs:186）阶段就 404 —— token 不匹配任何 group_key（无 token 或 token 错）时，分流到 models 端点（handler.rs:217）之前被拦下，模型发现 UI 用不了。改为：**GET /models 总是返回一份静态默认模型列表**（Claude + Codex 官方默认），不依赖 group、不 relay 上游，按请求路径协议格式化。

## Root Cause（已定位）

- `handler.rs:186` `resolve_group(token)` 仅按 `token == group_key` 精确匹配（`endpoint.rs:148`），无/错 token → `None` → `:194/:200` 直接 404。
- models 端点分流在 `handler.rs:217`，**晚于** group 解析，故 tokenless `/models` 探测永远到不了。
- 请求 `12a891b9645e4094a567a0843fbd2373`：client URL `/proxy/models`，group/model 全空（未路由），client=404 upstream=0，7ms。

## Requirements

* GET `/models` 与 `/v1/models`（`is_models_endpoint` 命中）总是返回 200 + 静态模型列表，**不需要 group / token**。
* 分流必须**移到 `resolve_group` 之前**，使 tokenless 探测也走静态列表，彻底消除 404 根因。
* 静态列表 = Claude + Codex 官方默认模型：`claude-opus-4-8`、`claude-sonnet-4-6`、`claude-haiku-4-5`、`gpt-5.5-codex`、`gpt-5.5`。
* 按 `detect_source_protocol(path)` 格式化：
  * `openai`（`/v1/models` 等含 `/v1/`）→ OpenAI 列表格式 `{"object":"list","data":[{"id":..,"object":"model","created":..,"owned_by":..}]}`
  * 其余（含 `/proxy/models` 裸路径 → 回退 `anthropic`）→ Anthropic 列表格式 `{"data":[{"type":"model","id":..,"display_name":..,"created_at":..}],"has_more":false,"first_id":..,"last_id":..}`
* 旧 `handle_models_passthrough` 的「选分组首个平台 relay 上游 /models」逻辑被静态列表取代（用户明确选「总是返回静态」）。
* 仍写 proxy_log（status=200，记 source_protocol / url），保持现有日志行为。

## Acceptance Criteria

* [ ] `cargo build` / `cargo clippy` 零 warning
* [ ] GET `/proxy/models`（无 Authorization）返回 200 + anthropic 格式静态列表（不再 404）
* [ ] GET `/v1/models`（无 Authorization）返回 200 + OpenAI 格式静态列表
* [ ] 单测覆盖：静态列表构造（两种格式）+ 模型集内容；`is_models_endpoint` 命中路径仍正确
* [ ] 现有 proxy 集成测试不回归（`cargo test`）

## Definition of Done

* 测试新增/更新（passthrough / handler 层单测）
* lint / typecheck / cargo test 全绿
* 行为变化（不再 relay 上游、tokenless 可用）在代码注释里说明
* CLAUDE.md「Local API」段如涉及 GET /models 行为，按需补一句

## Out of Scope

* `/v1beta/models`（gemini）—— `is_models_endpoint` 已显式排除，不在本期。
* 全局（含 chat）的 group 回退 —— 用户否决，仅 models 端点不依赖 group。
* 动态拉取上游真实模型列表 —— 本期纯静态。
* 前端 `getDefaultModels` 改动 —— 后端独立静态常量，不跨层复用 TS 预设。

## Technical Approach

1. **handler.rs**：把 `if GET && is_models_endpoint(&path)` 分流块从 `:217` 上移到 `resolve_group`（:186）**之前**，调用新静态处理函数（不传 `group`）。删除/简化原 :217 处分流。
2. **passthrough.rs**：`handle_models_passthrough` 改为 `handle_models_static`（不接 `group`、不打上游 HTTP），内部：
   - `let proto = detect_source_protocol(path);`
   - 按 proto 选格式构造 JSON（静态模型 id 列表常量）
   - 写 log（status=200, source_protocol=proto, response_body），返回 200 JSON。
   - 静态模型 id 列表作 `const`（backend 独立，不引前端）。
3. **格式构造**：拆纯函数 `build_static_models_json(proto: &str) -> serde_json::Value`（便于单测，免起 HTTP）。

## Decision (ADR-lite)

**Context**: tokenless / 错 token 的 GET /models 探测 404，模型发现失效。
**Decision**: GET /models 总是返回静态 Claude+Codex 默认列表，分流前置于 group 解析之前，按路径协议格式化；放弃上游 relay。
**Consequences**: 模型发现开箱即用、无需配置 token；代价是 /models 不再反映上游真实可用模型集（用户接受，本期纯静态）。模型 id 月级腐化需手工维护常量（注释标注核对日期）。

## Technical Notes

* `detect_source_protocol`（endpoint.rs:9）：`/proxy/models` 无 `/v1/` → 回退 `anthropic`；`/v1/models` → `openai`。
* `is_models_endpoint`（passthrough.rs:343）：`ends_with("/v1/models")||ends_with("/models")`，排除 `/v1beta/`。
* 现有 `handle_models_passthrough`（passthrough.rs:229）+ `build_models_url` / `apply_models_auth`（上游 relay 用）——本期不再需要 relay，注意清理引用避免 dead_code warning。
* 默认模型集参照前端 `getDefaultModels`（Platforms.tsx:438/440）：anthropic opus-4-8/sonnet-4-6/haiku-4-5；codex gpt-5.5-codex；openai gpt-5.5。
* proxy 路由表：mod.rs:189 `.fallback(handle_proxy)`，/models 走 fallback。
