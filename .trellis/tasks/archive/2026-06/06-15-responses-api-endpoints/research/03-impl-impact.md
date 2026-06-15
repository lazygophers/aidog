# Research: 复用 vs 新增 + 实现影响清单 + 建议方案

- **Query**: 能否泛化 handle_models_passthrough？要改哪些文件/分流点？风险/Codex 回归？是否拆 subtask？
- **Scope**: internal + 实现建议
- **Date**: 2026-06-15

## 1. 复用 vs 新增 `handle_models_passthrough`

`handle_models_passthrough`（proxy.rs:1916-2008）已具备的可复用骨架：
- 取分组首个启用平台（proxy.rs:1936）
- endpoint 优先取协议/URL，否则平台主配置（proxy.rs:1949-1953）
- `build_*_url` + `apply_*_auth`（平台凭证）+ relay + 记 proxy_log + 透传响应 content-type（proxy.rs:1954-2007）

差异点（responses 子端点 vs models）：
| 维度 | models | responses 子端点 |
|---|---|---|
| method | 固定 GET | GET(retrieve) / POST(cancel/compact，可能带 body) |
| body | 无 | cancel 可能无，compact 可能有 |
| URL 后缀 | `/models` 或 `/v1/models`（build_models_url） | `/responses/{id}[/cancel]`、`/responses/compact`（需保留客户端子路径） |
| 平台选择 | 首个启用平台（任意协议） | 首个 **responses-capable** 平台（协议须匹配，否则上游不认 /responses） |
| auth | apply_models_auth | 同款 Bearer（OpenAI 兼容） |

**建议：泛化为通用「非 chat 端点 passthrough」helper**，让 models + responses 子端点共用。签名草案（仅设计，禁实现）：
```
async fn handle_endpoint_passthrough(
    state, log, log_settings, group, start, lang,
    orig_method, orig_headers, bytes,        // 透传 method/headers/body
    upstream_path: &str,                      // 已 strip 前缀的上游子路径，如 /responses/{id}/cancel
    platform_filter: PlatformFilter,          // AnyEnabled | ResponsesCapable
) -> Response
```
- models 端点：method=GET, upstream_path=build_models_url 的后缀, filter=AnyEnabled。
- responses 子端点：method=原样, upstream_path=客户端子路径, filter=ResponsesCapable, body 透传。
- auth 统一走平台凭证（按平台协议选 Bearer / x-api-key）。

**或各写**：若担心泛化引入 models 回归，可新写 `handle_responses_subendpoint`，与 models 并存，共用底层 `build_http_client` / `apply_*_auth` / upsert_log。**权衡**：泛化减重复但触碰已上线的 models 路径（回归面大）；各写更隔离。**倾向：先各写 responses 子端点 helper，稳定后再考虑抽公共底座**（models 刚修完不宜立刻动）。

## 2. 实现影响清单（要改的文件 + 分流点）

### A. 分流识别（proxy.rs handler 层）
- 在 **models 分流之后、parse_incoming_request 之前**（proxy.rs:816～818 之间）加 **responses 子端点分流**：
  - 新增 `is_responses_subendpoint(path) -> bool`：strip 前缀后匹配 `/v1/responses/...`（尾段非空，即 `/responses/` 后还有 `{id}` 或 `compact`）。**必须排除恰好 `/v1/responses`（create 主请求）**——只命中子路径。
  - 命中 → `return handle_responses_subendpoint(...)`（透传），**不进 JSON parse / from_responses**（避免 EOF 400）。
- create（`POST /v1/responses` 精确）**不动**，继续走现有 parse + same_protocol_passthrough（已 work）。

### B. detect_source_protocol（可选）
- 若分流在 handler 层用 `is_responses_subendpoint` 提前 return，则 detect 不必改（子端点不会走到用 source_protocol 的逻辑）。
- 若想更干净，可在 detect 里把子端点也归 openai_responses（保持），靠 handler 分流区分主/子——**推荐 handler 层分流，detect 不动，改动最小**。

### C. 平台选择 + URL + auth
- 平台选择：分组首个 responses-capable 平台（endpoint 协议含 openai_responses，或 platform_type 走 responses）。复用 `get_group_platforms`（proxy.rs:1925）。
- URL：新 `build_responses_subendpoint_url(base_url, sub_path)`——strip 客户端 `/proxy`+group+`/v1` 前缀，取 `/responses/...` 子路径，`base_url.trim_end('/') + 子路径`。**禁直接用 build_passthrough_url**（会拼客户端 /proxy /v1 前缀，base_url 已含 /v1 会重复，见 02 难点②）。
- auth：平台凭证 Bearer（复用 `apply_models_auth` 或新 `apply_responses_auth`，OpenAI 兼容统一 `Authorization: Bearer {api_key}`）。
- 透传 body（POST cancel/compact）：原样 `bytes`；GET retrieve 不带 body。

### D. 不破坏 create 转换
- create 走的是 parse + 候选重试 + same_protocol_passthrough 分支（proxy.rs:944-1085），与子端点分流互斥（子端点提前 return）。确保 `is_responses_subendpoint` **不误判 create**（精确 `/v1/responses` 尾不带子段 → false）。

### E. 日志 / proxy_log
- 子端点复用 upsert_log，记 source_protocol=openai_responses, target_protocol=openai_responses（或 platform 协议），upstream_request_url=构造后 URL。

## 3. Codex 回归点

- **create 流式不能受影响**：本任务只加子端点分流（提前 return），create 路径不碰，回归风险低，但需测 `POST /v1/responses` 仍走原 parse/透传。
- **`is_responses_subendpoint` 边界**：必须精确放行 create（`/v1/responses` 无子段）、只拦子段（`/responses/{id}`、`/responses/{id}/cancel`、`/responses/compact`）。单测覆盖：`/proxy/v1/responses`(false)、`/proxy/v1/responses/resp_x`(true)、`/proxy/v1/responses/resp_x/cancel`(true)、`/proxy/v1/responses/compact`(true)、`/proxy/v1/responses/`(尾空斜杠，决定 true/false 需明确)。
- **多 responses 平台分组**：子端点取首个平台，若 create 落到非首个 → 子端点上游 404。需在 prd 标记为已知限制或加 response_id→platform 映射（重）。
- **Codex 是否真发子端点未知**：若 Codex 当前根本不发 retrieve/cancel/compact，则本任务为「防御性兼容」而非「修复线上故障」——优先级取决于 04 docs 核实结果。

## 4. 是否需拆 subtask

建议拆（资源/风险隔离）：
1. **subtask-A 调研收口（docs + Codex 实发 path）**：WebFetch 核 4 端点真实存在性/方法/body（04 清单）+ 抓 proxy_log 看 Codex 实发子端点。**先于编码**，决定范围。
2. **subtask-B 实现 responses 子端点 passthrough**：handler 分流 + `is_responses_subendpoint` + `handle_responses_subendpoint`（平台选择/URL/auth/body 透传）+ 单测。依赖 A 的端点清单。
3. （可选）**subtask-C 泛化 models+responses 公共 passthrough 底座**：A/B 稳定后做，非必需。

串行：A → B（B 依赖 A 确定的端点集合 + URL 形态）；C 最后。共享文件均为 proxy.rs，串行执行避免冲突。

## 实现影响汇总（文件）

| 文件 | 改动 |
|---|---|
| `src-tauri/src/gateway/proxy.rs` | handler 加 responses 子端点分流（proxy.rs:816 附近）；新 `is_responses_subendpoint` / `handle_responses_subendpoint` / `build_responses_subendpoint_url`；单测 |
| `converter.rs` | **可能不动**（passthrough_api_path 仅 create 用；子端点走新 helper 自己构造 path） |
| `codex.rs` | 不动（base_url/wire_api 已对） |
| detect_source_protocol | **不动**（handler 层分流） |

## Caveats / Not Found

- 端点真实集合 / body 形态待 04 docs 核 → 决定 B 的 URL 构造与 method 处理细节。
- 多 responses 平台下平台定位是设计缺口（首个平台方案有 404 风险），是否上 response_id 映射由 main/prd 定。
