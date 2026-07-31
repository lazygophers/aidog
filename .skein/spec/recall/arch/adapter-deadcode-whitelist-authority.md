---
name: adapter-deadcode-whitelist-authority
title: adapter-deadcode-whitelist-authority
layer: recall
category: arch
keywords: [arch,rule,spec]
created: 1725080438
inclusion: auto
---


## 触发场景
删除 vendor adapter 文件或判定某 adapter 是否属于死代码时。

## 陷阱
用文件名判定（如「vendor 名 = 协议名」），误删活代码；或遗漏实际有白名单的 adapter。

## 正解
**唯一权威 = `gateway/proxy/forward.rs:85-86` 的 `is_valid_wire_protocol` 5 协议白名单**（Anthropic/OpenAI/OpenAIResponses/OpenAICompletions/Gemini）+ preset `endpoints[].protocol` 值域。

死代码判定三层：
1. 不在 forward.rs 白名单内 → 检查 preset endpoints 有无该协议
2. preset 无该协议 → 检查源码有无 `#[allow(dead_code)]` 标记
3. 有标记 → 确认死代码，可删

## 案例
- arch-deepen-2 commit `78e32df4`：删的 5 个 vendor adapter（glm_coding/bailian/qianfan/xai/llama_cloud）全带 `#[allow(dead_code)]`
- converter/request.rs match 分支缺失 = 该协议确实不在活代码里

## 适用
- adapter 文件管理时
- protocol 数量变更
- 编码规范卡关：为什么要删这个文件

## 关联
[[rule-07]]

---

