---
title: agent-as-LLM 平台 handler 分支接入范式
layer: recall
category: arch
keywords: [agent,handler,branch,platform,wire,sse]
source: auto-fix-downgrade
authored-by: skein-spec
created: 1784706792
status: active
related: []
updated: 1784706792
---

# agent-as-LLM 平台 handler 分支接入范式

## 触发场景
新增「agent-as-LLM」类平台（无标准 chat completions wire，API 形态是 session / task / 异步轮询 / 订阅透传 / mock 测试）。

## 陷阱-正解
- **陷阱**: 新平台硬塞 wire 层 → adapter/converter 反复打补丁、协议转换丢字段、候选切换语义错位
- **正解**: 走 handler 分支（拦截点在 handler.rs 重试循环外），三要素：
  1. 拦截点：`matches!(first.platform.platform_type, Protocol::<Xxx>)` → return
  2. 协议转换在本平台 handler 内自包
  3. 伪流式 SSE 复用 `to_client_sse`

## 判定：分支 vs wire
| 特征 | wire 层 | handler 分支 |
|------|---------|-------------|
| 上游 API 形态 | OpenAI/Anthropic/Gemini chat completions | session/task/异步轮询/订阅透传/mock |
| 候选切换语义 | 多候选 retry | 单 session 即终态 |
| 流式 | 原生 SSE relay | 伪流式（轮询/透传+SSE） |
| 计费单位 | token → $ | ACU/work-unit/无 |
| 例 | openai/anthropic/qwen | Mock/ClaudeCode/Devin |

## 反例
❌ 新 agent 平台塞 wire 层 → adapter 改到吐血
❌ 分支内做多候选 retry → agent 单 session 终态
❌ 伪流式重写 SSE 格式化 → 必复用 to_client_sse

## 适用
agent-as-LLM 平台接入（Mock/ClaudeCode/Devin/Factory）

## 关联
dashmap-sharding (session 映射)
[[trellis-04]] (enum 变体同步)
