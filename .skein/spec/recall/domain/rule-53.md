---
title: N×N 互转路 A vs 路 B
layer: recall
category: domain
keywords: [converter NonStreamResponse parse render protocol]
source: -
authored-by: skein-spec
created: 1784985323
status: active
related: []
updated: 1784985323
---
---
# N×N 互转路 A vs 路 B

## 触发场景

- N 协议互转设计选择：内部归一（路 A）vs 点对点（路 B）
- O(N) parse + render vs O(N²) 函数

## 陷阱-正解

- ❌ 路 B：点对点 N×N 函数 → 新增协议需加 N 个函数
- ✅ 路A：NonStreamResponse 作中间归一 → 新增协议只加 1 parse + 1 render

## 设计决策

路 A（内部归一）：
1. 上游响应 → parse → NonStreamResponse（归一）
2. NonStreamResponse → render → 客户端协议

## 覆盖范围

- 当前：openai → anthropic 真转换（convert_response）
- 其余组合：回退透传（return None）

## 反例

- 点对点设计：新增协议时改 N 处 → O(N²) 维护成本
- 无中间归一：无法跨协议组合（如 openai→gemini）

## 适用

- converter 模块扩展（新增协议/转换组合）
- N×N 互转矩阵设计（converter-reasoning-content task）

## 案例

- converter-reasoning-content：5×5 互转矩阵用 NonStreamResponse
- parse_openai_response → render_anthropic_response 链路

## 关联

[[rule-52]] [[rule-54]]
