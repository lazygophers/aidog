# PRD: 流断优雅透传 + 单平台无视状态必请求 + 成功即恢复

## 背景

实证 request `07a98bbad7c44e958d6a25e78ee18286`（group glm-coding-plan-auto，单平台，anthropic 同协议透传）：
GLM 流式输出到一半（thinking_delta），~60s 后上游截断连接、无 `message_stop`。aidog 读上游 `bytes_stream` 时 reqwest 抛 `error decoding response body`：
- 透传路径 `proxy.rs:1918` → `return Err(io::Error)` → 直接断下游流 → CC 报 `API Error: {"error":"error decoding response body"}`。
- 转换路径 `proxy.rs:1460` → 注入 `event: error\ndata: {"error":"<e>"}` SSE → CC 同样显示 API Error。

## 目标（用户三条诉求）

1. **流式 chunk 读失败 → 优雅收尾，不报错**：仅记日志，把已有内容透传出去并干净结束流（合成客户端协议的 Stop/message_stop 终止事件），不再注入 `event: error`、不再 `return Err` 断流。
2. **单平台分组无视平台状态必请求**：分组只有 1 个平台时，无论该平台 auto_disabled / 熔断 Open 状态如何，都纳入候选发起请求（哪怕会失败也要尝试）；只有多平台分组才按平台状态过滤。（手动 Disabled 是用户显式意图，仍为唯一硬停。）
3. **成功即恢复平台状态**：请求 2xx 成功时及时清 auto_disabled / 关熔断（验证现有 `record_success`/`recover_platform_auto_disabled` 在单平台必请求路径仍生效）。

## 方案

### 触点 1 — router.rs select_candidates_ctx
- `group_platforms.len() == 1` 时：跳过 auto_disabled 退避过滤 + 熔断准入过滤，直接把该唯一平台作为候选（除非手动 Disabled → 仍 Err，分组无效）。
- 多平台：保持现有 auto_disabled ∪ 熔断 并集过滤 + 上轮 06-16-blackhole 的「熔断踢空回退」。

### 触点 2 — proxy.rs 两个 bytes_stream 闭包
- 1460（转换）：chunk Err → `tracing::warn` + emit `to_client_sse(Stop)`（按 client_protocol 干净收尾），不再注入 error 事件。
- 1918（透传）：chunk Err → `tracing::warn` + emit 同协议 message_stop 终止字节（或最小化：结束流不报错），不再 `return Err`。

### 触点 3 — proxy.rs 成功恢复
- 验证 `1319 record_success` + `1328 recover_platform_auto_disabled` 在 2xx（含 stream header 200）路径触发；单平台必请求成功后状态正确回 enabled。

## 验收标准

1. 流式上游中途断流时，CC 不再收到 `API Error: error decoding response body`，已输出内容保留、流干净结束。
2. 单平台分组、平台 auto_disabled 或熔断 Open 时仍发起上游请求（proxy_log 有 upstream 记录，非 400 no available platform）。
3. 多平台分组行为不变：有健康平台优先，坏平台按状态过滤。
4. 2xx 成功后平台 auto_disabled 清除 / 熔断关闭。
5. `cargo test` 全绿（新增单平台必请求单测）；`cargo clippy` 无新 warning。

## 非目标
- 不修上游 GLM 自身 60s 截断（上游侧不可控）。
- 不改 429 是否计熔断。
