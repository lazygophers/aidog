---
title: Protocol 协议名统一 wire_str() 编码
layer: recall
category: arch
keywords: [protocol,serde,wire,codegen,enum]
source: -
authored-by: skein-spec
created: 1785226172
status: active
related: []
updated: 1785226172
---

## 触发场景
在 proxy/forward 层需要获取协议名或序列化 Protocol enum 时。

## 陷阱
禁手写 `serde_json::to_string(&x).trim_matches('"')` 或其他字符串转换，容易遗漏边界。

## 正解
统一用 `Protocol::wire_str()` 方法序列化协议名。

## 案例
- gateway/models/protocol.rs:173 定义 wire_str()
- arch-deepen-2 commit 97a890d5 统一迁移调用点

## 适用
- Protocol enum 序列化时
- adapter 分发时协议名判定

## 关联
[[Protocol-枚举变体扩展范式]] [[新增-wire-protocol-必须同步白名单]]
