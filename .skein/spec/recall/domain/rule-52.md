---
title: reasoning_content → anthropic 方案 B
layer: recall
category: domain
keywords: [reasoning thinking anthropic signature converter]
source: -
authored-by: skein-spec
created: 1784985323
status: active
related: []
updated: 1784985323
---

# reasoning_content → anthropic 方案 B 决策

## 触发场景

- 第三方（deepseek/sensenova/glm）reasoning_content 纯文本无 signature
- 转 anthropic 时出 text 块（方案 B）禁 thinking 块（thinking 需 cryptographic signature，空串被 CC 多轮拒）

## 陷阱-正解

- ❌ 方案 A（标准协议）：出 thinking 块 → signature 风险
- ✅ 方案 B（务实方案）：reasoning_content 忽略，出 text 块

## 决策背景

- TrueFoundry/LiteLLM #8927 调研佐证：第三方 reasoning 无 signature
- 用户原选方案 A，摆出 signature 风险后改选方案 B

## 实现

- openai/response.rs:13：reasoning_content 被忽略，不影响 content/tool_use 产出
- anthropic.rs:58-64：只保留已知类型（Text/ToolUse/ToolResult），Unknown(thinking/redacted_thinking/image) 跳过

## 反例

- 强行出 thinking 块 → CC 多轮交互时 400/empty or malformed
- 空 reasoning_content 不应出 thinking 块（signature 不可伪造）

## 适用

- 所有第三方 → anthropic 跨协议转换
- reasoning 扩展字段处理（未来第三方新增非标准字段）

## 关联

- [[5-协议定义锚点]] — 5 协议真值源
- [[N×N-互转-路A-vs-路B]] — converter 中间归一设计
