---
title: optional-config-backward-compat
layer: recall
category: ts-rust-boundary
keywords: [ts-rust-boundary,option,backward-compat,unwrap_or,config-migration]
status: active
---

## Option<T> 可选字段的向后兼容方案

## 问题

新旋钮常需跨 Rust↔TS 边界，并与旧配置字段共存以确保向后兼容。

例：`mock` 配置新增 `ttft_ms` 和 `inter_chunk_ms` 两个独立延迟旋钮，但旧配置仍用单一 `delay_ms`。需确保：
- 旧配置 `delay_ms=500` 单独设置时行为不变
- 新配置可分别指定两个值
- Rust/TS 两端字段定义、序列化、编辑器一致

## 方案

**Rust 端** (`config.rs:11-25`)：
```rust
pub struct MockConfig {
    pub delay_ms: u64,  // 兼容入口（必留）
    pub ttft_ms: Option<u64>,       // None=回落 delay_ms
    pub inter_chunk_ms: Option<u64>, // None=回落 delay_ms
}
```

**取值** (`proxy/mock.rs:36-39`)：
```rust
let ttft_ms = cfg.ttft_ms.unwrap_or(cfg.delay_ms);
let inter_chunk_ms = cfg.inter_chunk_ms.unwrap_or(cfg.delay_ms);
```

**TS 端** (`manual.ts:467-485`)：
```typescript
export interface MockConfig {
  delay_ms: number;  // 兼容入口
  ttft_ms?: number;   // undefined=回落 delay_ms
  inter_chunk_ms?: number; // undefined=回落 delay_ms
}
```

**序列化** (`platforms.ts:139-153`)：
```typescript
// 新字段为 undefined 时，JSON.stringify 自动丢弃（空字段）→ Rust 端 deserialize 作 None
obj.mock = mock; // 包含 ttft_ms/inter_chunk_ms
return JSON.stringify(obj); // undefined 字段被丢弃
```

## 关键点

- **旧字段保留**：必须保留兼容入口，不删不改
- **Option/undefined 对应**：Rust `Option<T>` ↔ TS `field?: T`；JSON 序列化时 undefined 自动丢弃
- **无条件回落**：新字段缺失时用 `unwrap_or(旧字段)` 兜底，零行为变化
- **四处同步**：Rust struct + TS interface + 编辑器 default 值 + 序列化逻辑，任一漏改即失配

## 用途

配置迭代的通用方案，适用于：
- 新增可选旋钮
- 旧版本平台配置升级
- 分阶段特性开关（旧特性先 disable，新特性先 enable）
