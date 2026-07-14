---
title: dedup 禁用设计为空的字段作 key
layer: core
category: arch
keywords: [dedup,空字段,base_url,key,静默丢失,合并,数据丢失,去重]
source: cpa-parse-no-provider
authored-by: skein-memory
---

# dedup 禁用设计为空的字段作 key

何时被读: 写任何 dedup / 去重 / 合并逻辑(HashSet key / HashMap key / groupBy key)前
谁读: 写后端聚合 / 数据合并 / 导入解析逻辑的开发者

## 规则

字段设计为空(待后续回填 / 占位 / 语义上由下游确定)但被用作 dedup key → N 个对象共享同一空值(如 `""` / `None`)→ HashSet 全撞 → **静默合并 N-1 个, 数据丢失, 无报错**。

## Why

dedup 是「相等则视为重复」操作。空字段全部相等 = 全部视为重复 = 只留首个。这类 bug:
- 无运行时错误(逻辑合法, HashSet 正常工作)
- 无日志(合并是预期行为)
- 仅在「实际丢了数据」后才发现(用户投诉 / 数据对不上)

code 记了「改 key 为 name」, 但「为什么空字段是坑」非显然, 未来 dedup 易再踩。

## 示例

❌ **错误**(`parser.rs:641-649` 原 OAuth dedup, cpa-parse-no-provider bug):
```rust
// OAuth provider base_url 全空("OAuth 平台 base_url 由后续映射确定")
let key = (provider.source_segment, provider.base_url);
// → 10 个 OAuth 凭据 dedup key 全 (OAuth, "") → 只留首个, 丢 9 个 access_token
```

✅ **正确**(改用非空唯一字段):
```rust
// OAuth 按 (segment, name/email) 去重; 各凭据 email 不同 → 各自独立 key 不撞
let key_name = provider.name.clone().unwrap_or_default();
let key = (provider.source_segment, key_name);
```

## How to apply

dedup key 选择优先级:
1. **业务唯一键**(user_id / email / name) — 最稳
2. **非空技术键**(url / path / hash) — 次稳
3. **组合键** — 多字段拼, 至少一维非空

**禁**用设计为空(待回填 / 占位 / 语义延后)的字段作 dedup key。

测试: 构造 N 个对象(该字段全空但其余不同), dedup 后必须保留 N 个(非合并为 1)。

## Cross-ref

- `src-tauri/crates/aidog_core/src/gateway/cpa_import/parser.rs:666-683` 修复示例(cpa-parse-no-provider s1)
- 关联 [[parser-multi-path-format-symmetry]](本 task 同源: OAuth 多凭据场景暴露)
