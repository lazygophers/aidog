# Devin 平台支持 — 详细设计

## 架构定位
Devin = **特殊 platform_type**（归 `protocol.rs:19` 「平台类型」组，与 Mock/ClaudeCode/CliProxy 同组），非 wire 协议（不入 `protocol.rs:8` 「AI 请求协议」组）。

接入范式同 ClaudeCode（`handler.rs:432`）：重试循环外按 `platform_type` 拦截，调专门 handler。**不经标准 `forward_attempt` / adapter/converter wire 层**（Devin 不兼容 openai/anthropic/gemini 任一 wire）。

## 接入点

### 1. `handler.rs` Devin 分支（核心）
仿 `handler.rs:432` ClaudeCode 分支，在重试循环外（line 408-450 段）加：
```rust
if matches!(first.platform.platform_type, Protocol::Devin) {
    log.platform_id = first.platform.platform_id;
    return handle_devin(state, log, log_settings, &first.platform,
                        &chat_req, &req_value, &source_protocol,
                        &requested_model, is_stream, start, lang).await;
}
```

### 2. `handle_devin` 函数（新，独立文件 `proxy/devin.rs` 或 handler 内）
session 编排 + chat↔session 转换 + 伪流式 + 超时 504 全在此。不经 adapter/converter。

### 3. 候选解析（`candidates.rs`）
Devin 平台用标准 platform 字段（`base_url`=api.devin.ai / `api_key`=cog_ key / `extra.org_id`），候选解析大概率不需特殊分支（不像 CliProxy 要从另一表拉 wire）。s2 确认；若候选阶段需过滤/填充再补。

## 数据流（handle_devin 内）

```
客户端 POST /chat/completions 或 /v1/messages
  ↓ resolve group → 选 Devin platform candidate（handler 拦截）
handle_devin:
  ── convert chat → devin ──
  messages(system+user+assistant) → prompt（[role] content 拼接）
  model → mode（5 档映射表）
  tools → 丢弃 + warn
  X-Devin-Session-Id header → 新建 vs 复用
  ── session 编排 ──
  新建: POST /v3/organizations/{org_id}/sessions {prompt, mode, max_acu_limit?}
        → 存 X-Devin-Session-Id → devin_id 映射
  复用: POST /v3/organizations/{org_id}/sessions/{devin_id}/messages {message}
  轮询: GET /sessions/{id} 到终态(exit/error/suspended)，间隔 10s，上限 devin_timeout
  取输出: GET /messages → 最后 source==devin message
  ── 包 chat response ──
  非流式: {choices:[{message:{content}}], usage:{acus_consumed}}
  伪流式: 轮询 diff 新 devin message → SSE chunk → 终态 [DONE]
  超时:   504 + {session_id, url, message}
  ── 落库 ──
  est_cost = acus_consumed; proxy_log 落库
```

## 字段映射

| chat request | Devin |
|---|---|
| messages | prompt（`[role] content` 标注拼接） |
| model | mode 字段（5 档，字段名实测裁定） |
| stream | 伪流式 / 超时 504 |
| tools | 丢弃 + warn |
| max_tokens / temperature | 忽略 |

| chat response | Devin |
|---|---|
| choices[0].message.content | 最后 `source==devin` message |
| usage.total_tokens | `acus_consumed`（UI 标 ACU） |
| est_cost | `acus_consumed`（不折算 $） |

## session 映射存储
`X-Devin-Session-Id`（客户端传）→ `devin_id`。v1 内存 LRU + TTL 30min（Devin session 闲置 sleep，映射过期=新建，可接受）。重启丢=下次新建，不致命。

## 超时异步
`devin_timeout` 默认 300s（`platform.extra.devin_timeout` 可配）。超时 → 504：
```json
{"error":{"type":"devin_timeout","session_id":"devin-...","url":"https://app.devin.ai/sessions/devin-...","message":"Devin task still running, check url"}}
```
禁 200 假回复（混淆 chat 语义）。

## quota
`GET /v3/organizations/{org_id}/consumption/daily` → `balance.used = total_acus`。无 remaining（Devin 无余额端点）。UI 标"ACU 用量"。

## mode 字段冲突裁定
- researcher `.md` 源: `advanced_mode[analyze/create/improve/batch/manage]`
- WebFetch 渲染页: `devin_mode[normal/fast/lite/ultra/fusion]`
- **策略**：s3 转换用 `devin_mode`（WebFetch 较新）+ 5 档命名。s9 验收用真 cog_ key curl 实测；不符则改。preset models 命名待实测定型。

## 取舍
- **handler 平台分支而非 adapter wire 层**：Devin 非 wire 协议，转换在 `handle_devin` 内，不碰 adapter/converter。
- **伪流式**：用户选。代价=轮询 diff + chunk 粒度粗。
- **stateful**：用户选。代价=映射状态管理 + header 约束客户端。收益=省 ACU + 保上下文。
- **504 异步**：用户选。语义正确，客户端按错误处理。
- **est_cost 不折算 $**：ACU 单价未公开，记数不臆造。
