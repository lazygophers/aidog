---
title: agent-as-LLM 平台 handler 分支接入范式
layer: core
category: arch
keywords: [agent,platform,handler,branch,bypass,wire,forward_attempt,pseudo-stream,sse,acu,session,504,mock,claudecode,devin,factory]
source: add-devin-support
---

# agent-as-LLM 平台 handler 分支接入范式

何时被读: 新增「agent-as-LLM」类平台（无标准 chat completions wire，API 形态是 session / task / 异步轮询 / 订阅透传 / mock 测试）/ 改 handler.rs 平台分支拦截区 / 评估新平台应走 wire 还是 branch。
谁读: planning sub-agent（方案选型）/ 执行 sub-agent（接入实现）。
不遵守的代价: 新平台硬塞 wire 层 → adapter/converter 反复打补丁、协议转换丢字段 / 候选切换语义错位（agent session 单 session 终态，无 retry 候选切换）/ 流式语义错（无原生 SSE 的 agent 被迫走非流式）。

## 判定：何时走分支 vs 何时走 wire

| 特征 | wire 层（默认） | handler 分支（本规则） |
|---|---|---|
| 上游 API 形态 | OpenAI/Anthropic/Gemini chat completions 兼容 | session/task/异步轮询 / 订阅 OAuth 透传 / 测试 mock |
| 候选切换语义 | 多候选 retry，逐个 forward_attempt | 单 session 即终态，无候选切换 |
| 流式 | 原生 SSE relay | 伪流式（轮询/纯透传 + 切块 SSE） |
| 计费单位 | token → $ | ACU / work-unit / 无（订阅自带） |
| **例** | openai / anthropic / qwen / glm / kimi ... | Mock / ClaudeCode / Devin / Factory(未来) |

命中分支特征任一 → 走分支，禁塞 wire。

## MUST 分支接入三要素

### 1. 拦截点：handler.rs 重试循环外

位置：`select_platform` 之后、`convert_request` / 重试循环之前（handler.rs L420-470 区）。
判定：`matches!(first.platform.platform_type, Protocol::<Xxx>)` → 调本平台 handler → `return`（禁进入 `forward_attempt` 循环）。

现状实例（2026-07）：
- `Protocol::Mock` → mock 测试 handler（最早实例）
- `Protocol::ClaudeCode` → `handle_passthrough`（订阅透传）
- `Protocol::Devin` → `super::devin::handle_devin`（session 编排）

新增分支平台时 grep 命中点：
```bash
grep -n "Protocol::Mock\|Protocol::ClaudeCode\|Protocol::Devin" src-tauri/crates/aidog_core/src/gateway/proxy/handler.rs
```

### 2. 协议转换在本平台 handler 内自包

chat completions ↔ 平台原生协议（session / task / OAuth header）的转换**全部在本平台 handler 模块内**，禁泄漏到 adapter/converter（wire 层）。
- Devin: chat_req → create session / poll / fetch messages → chat response（`proxy/devin.rs`）
- ClaudeCode: 0 转换，orig bytes 1:1 relay（`handle_passthrough`）

### 3. 伪流式 SSE（无原生 SSE 的 agent）

复用 `adapter::converter::to_client_sse(event, source_protocol, model)` 按客户端协议（openai/anthropic/gemini）格式化切块，再用 `futures::stream::iter(chunks).map(Ok::<_, io::Error>) → Body::from_stream` 包 axum Response（参考 `mock.rs:112` / `finish.rs:332` / `devin.rs:906`）。

chunk 序列骨架：`Start → N×Delta → Stop | Error`。禁 progressive poll-during-stream（需 session 映射或 spawn，复杂度爆）。

## 长任务 / 超时（异步 agent 平台）

session/task 非即终态 → handler 内轮询 + 可配超时（`extra.<platform>.<platform>_timeout` 秒）。超时返 **504 Gateway Timeout + 结构化 body（含 session_id / url / message）**，禁 200 假回复。流式分支：Start + error SSE chunk + http 504。

## 非 token 计费（ACU / work-unit）

`proxy_log.est_cost` 对本类平台记平台原生单位（Devin = `session.acus_consumed` f64），**禁 token→$ 折算**（单价未公开无可靠源）。BalanceInfo 三字段语义：`used` = 累计用量 / `remaining` = 0（无余额端点）/ `currency` = 原生单位字符串。前端 BalanceBar 需 UI 标注「用量」非「$ 余额」。

## 反例（禁）

- ❌ 新 agent 平台塞 wire 层 → adapter 改到吐血、字段丢失
- ❌ 分支内做多候选 retry → agent 单 session 终态，无候选切换语义
- ❌ 伪流式重写 SSE 格式化逻辑 → 必复用 `to_client_sse`，否则客户端协议格式漂移
- ❌ 流式超时返 200 + error body → 客户端按正常流处理吞错（必须 504）

## Cross-ref

- Mock platform type（最早分支实例，handler.rs:408）
- Claude Code passthrough（透传分支实例，handler.rs:432）
- Protocol enum 变体扩展（新平台加 enum + TS union 同步，本规则的 enum 侧机械）
- [[dashmap-sharding]]（session 映射用模块级 OnceLock<DashMap>，见 Devin 实例）
