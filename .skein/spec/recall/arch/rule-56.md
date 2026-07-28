---
title: Gemini SSE 需 ?alt=sse 参数
layer: recall
category: arch
keywords: [gemini,sse,streaming,adapter,parameter]
source: -
authored-by: skein-spec
created: 1785226166
status: active
related: []
updated: 1785226166
---

## 触发场景
改 gemini adapter 或调试 Gemini streaming 响应时。

## 陷阱
不带 `?alt=sse` 参数时，Gemini API 响应体不是 SSE 格式（返回普通 JSON 数组），`strip_prefix("data: ")` 对其永不匹配，导致解析失败。

## 正解
向 Gemini 端点拼入 `?alt=sse` 参数，确保响应格式为 Server-Sent Events。

## 案例
- arch-deepen-2 commit `39a6614c`：gateway/proxy/forward.rs:203-211 补参数修复
- adapter/gemini.rs:430 注释标记该限制

## 适用
- Gemini 协议 SSE 响应处理
- 其他 SSE 适配器的对称性检查（防止他协议有类似参数需求遗漏）

## 关联
[[rule-57]] [[rule-58]]
