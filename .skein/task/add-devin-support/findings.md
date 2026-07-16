# Devin 平台 — 调研收敛

> 完整调研笔记: [research/devin-api-lifecycle.md](research/devin-api-lifecycle.md) (238 行)
> 数据源: docs.devin.ai v3 OpenAPI + Mintlify `.md` 源 + common-flows

## 架构定性（关键纠正）

Devin 是**平台**（platform_type），非 wire 协议。依据 `protocol.rs:8,19` 注释两组分类：
- 「AI 请求协议」（endpoint wire）= anthropic/openai/openai_responses/openai_completions/gemini（5 个）
- 「平台类型」（仅作 platform_type）= mock/claude_code/cli-proxy/glm/deepseek/... — Devin 归此组

aidog 所有标准平台 endpoint 用 5 wire 协议之一（preset: glm/deepseek endpoints 均列 `[{protocol:openai},{protocol:anthropic}]`）。Devin 不兼容任一 → 不能走标准 endpoint。

接入范式 = 特殊平台分支：ClaudeCode `handler.rs:432` / Mock `handler.rs:412` / CliProxy `candidates.rs:164`。Devin 同模式 —— `handler.rs` 按 `platform_type==Devin` 拦截调 `handle_devin`，不经 `forward_attempt`/adapter/converter。

## Devin API 事实（高置信，docs.devin.ai v3 原文）

| 维度 | 结论 |
|---|---|
| base URL | `https://api.devin.ai/v3/organizations/{org_id}/*` + `/v3/enterprise/*` |
| auth | Bearer `cog_` key + `org_id`（`org-` 前缀，path 必填） |
| session 生命周期 | POST /sessions → poll GET /sessions/{id} → GET /messages |
| status 状态机 | new/claimed/running/resuming/exit/error/suspended；**终态 = exit ∪ error ∪ suspended** |
| 多轮 | POST /sessions/{id}/messages（须 running 态，body=`{message}`） |
| 流式 | **无原生 SSE/WS**，官方 sleep(10) 轮询 |
| 计费 | ACU（session.acus_consumed，max_acu_limit 硬上限），非 token 非时长 |
| quota 端点 | GET /consumption/daily（total_acus + acus_by_product），无实时余额 |
| RBAC | 创建=ManageOrgSessions，读=ViewOrgSessions，consumption=ViewOrgConsumption |
| 输出通道 | GET /messages（最后 devin message）/ structured_output / attachments |

## 冲突点（需实测裁定）

### 1. mode 字段（critical，阻塞 model 映射命名）
- researcher `.md` 源: `advanced_mode` enum `[analyze/create/improve/batch/manage]`
- WebFetch 渲染页: `devin_mode` enum `[normal/fast/lite/ultra/fusion]`
- 同 create 端点不同果，疑 Mintlify CDN 缓存新旧版差异
- **裁定**：s3 用 devin_mode（WebFetch 较新），s9 实测确认

### 2. POST /sessions/{id}/messages v3 schema 被 gate
- v3 `.md` 源返 null，v1 佐证 body=`{message}`；实测确认有无 attachment 字段

### 3. DELETE terminate 端点
- nav 列出，v3 schema 缺；客户端断连 terminate 止血省 ACU（可选优化）

### 4. 429 响应头
- 未文档化 `Retry-After`/`X-RateLimit-*`；实测抓

## 需要（实测补齐，需 cog_ key + org_id）

`需要:` 真 cog_ key + org_id，用于：
1. curl create-session 确认 mode 字段名 + 枚举（冲突点 1）
2. curl POST messages 确认 v3 schema（冲突点 2）
3. curl 429 抓响应头（冲突点 4）
4. 确认有无实时余额端点

> planning 无 key，实测落 s9 验收。转换层先用 devin_mode 实现，实测修正。

## SPEC 标记（researcher 回传，候选 sediment）
agent-as-LLM 平台接入硬约束（Devin 及同类）：
1. agent 平台非 wire 协议，接入走 handler 平台分支（非 adapter wire 层）
2. 无原生流式，`stream:true` 需伪流式/降级（架构级折衷）
3. 计费非 token（ACU/work units），chat usage 语义不匹配，UI/quota 需专门标注
4. 长任务异步（session 分钟~小时级）vs 同步 chat 根本张力，需超时+异步响应策略
