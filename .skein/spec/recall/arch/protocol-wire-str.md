---
name: protocol-wire-str
title: protocol-wire-str
layer: recall
category: arch
keywords: [arch,rule,spec]
created: 1725080438
inclusion: auto
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
[[rule-05]]

---

