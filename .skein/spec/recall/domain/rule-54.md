---
title: bug1 真相
layer: recall
category: domain
keywords: [target_protocol platform_type matched_ep preset]
source: -
authored-by: skein-spec
created: 1784985323
status: active
related: []
updated: 1784985323
---

# bug1 真相：target_protocol 落平台名

## 触发场景

- proxy_log.target_protocol 落平台名（如 "glm"）而非 endpoint 协议（如 "openai"）
- forward.rs:75 fallback platform_type 致 target_protocol 落 platform_type

## 根因分析

1. matched_ep=None 时 `unwrap_or((&route.platform.platform_type, ...))` fallback
2. 非 endpoint 配置缺 protocol，而是 DB 平台 endpoints 字段空（preset 未加载）
3. is_valid_wire_protocol 缺白名单 gate

## 修复方案

- is_valid_wire_protocol 白名单：5 协议（anthropic/openai/openai_responses/openai_completions/gemini）
- 不可变 endpoint 层只支持 5 协议；其余是平台别名（platform_type）

## 反例

- ❌ 误判：endpoint 配置缺 protocol → 实际是 DB endpoints 字段空
- ❌ 误修：加 endpoint 配置而未修白名单 → 新协议仍 route fail

## 关键不变量

matched_ep=None 的合法情况：preset 未加载（DB endpoints 空），非用户配置错误

## 适用

- target_protocol 异常落平台名
- 新增 wire protocol 后 route fail
- preset 未加载场景

## 案例

- converter-reasoning-content bug1：preset 未加载致 matched_ep=None
- 修复后 is_valid_wire_protocol 白名单 gate

## 关联

- [[5-协议定义锚点]] — 5 协议真值源
- [[新增-wire-protocol-必须同步白名单]] — 白名单硬约束
