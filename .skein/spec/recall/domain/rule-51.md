---
title: 5 协议定义锚点
layer: recall
category: domain
keywords: [protocol endpoint converter platform_type]
source: -
authored-by: skein-spec
created: 1784985323
status: active
related: []
updated: 1784985323
---

# 5 协议定义锚点

## 触发场景

- endpoint 协议层只 5 种（anthropic/openai/openai_responses/openai_completions/gemini）
- 其余 Protocol enum 变体（sensenova/glm/kimi 几十个）是平台别名（platform_type）非协议
- protocol.rs:8-18 注释已标「AI 请求协议」vs「平台类型」

## 陷阱-正解

- ❌ 混淆：以为所有 Protocol 枚举值都是「协议」
- ✅ 区分：仅 5 个可作为 endpoint 协议参与转换；其余是平台类型（路由/聚合/CLI 等用途）

## 反例

- 把 glm/kimi/sensenova 当作 endpoint 协议 → 转换时 panic/未实现
- 误以为有 40+ 种协议格式 → N×N 互转矩阵爆炸

## 关键不变量

endpoint 协议 = converter 模块支持的格式（convert_request + parse_sse）

## 适用

- converter 模块扩展（新增 wire protocol）
- N×N 协议互转设计（真值源）
- 平台接入时 endpoint 配置

## 案例

- converter-reasoning-content task：5 协议是 N×N 互转矩阵的锚点
- glm/kimi 等虽独立协议 enum，但 endpoint 层仍走 5 协议之一

## 关联

- [[新增-wire-protocol-必须同步白名单]] — endpoint 协议白名单
- [[N×N-互转-路A-vs-路B]] — converter 设计决策
