---
title: wire-protocol-whitelist-sync
layer: core
category: proxy
keywords: []
source: -
authored-by: skein-spec
created: 1784985303
status: active
related: []
updated: 1784985303
---
---
# 新增 wire protocol 必须同步白名单

## MUST 硬约束

新增 wire protocol 时必须同步更新以下白名单，否则新协议会导致 route fail：
- forward.rs 中 is_valid_wire_protocol 白名单（5 协议：anthropic/openai/openai_responses/openai_completions/gemini）
- converter/request.rs convert_request 的 match 分支
- converter/response.rs parse_sse 的 match 分支

## 反例

- 新增 protocol X 但未加入白名单 → matched_ep=None 时 fallback 到 platform_type，target_protocol 落平台名（如 "glm" 而非 "openai"）
- 只更新白名单而未加 converter 分支 → 转换时 panic/未实现

## 触发场景

- converter-reasoning-content task：bug1 根因分析发现 matched_ep=None 时 forward.rs:75 fallback platform_type
- DB 平台 endpoints 字段空（preset 未加载）导致 matched_ep=None，非 endpoint 配置缺 protocol

## 适用

- 所有新增 wire protocol（endpoint 协议层）的变更
- 非 platform_type（平台别名，Protocol enum 其他几十个变体）

## 关联

[[rule-52]] [[rule-53]]
