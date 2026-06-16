# 缓存率可超 100%

## 根因
`cache_tokens` = `cache_read_input_tokens`（Anthropic 命中缓存读取），`input_tokens` = 非缓存新输入。两者独立维度。
当前公式 `cache_rate = cache_tokens / input_tokens * 100` —— 大量缓存命中时 cache_read >> input → 用户实测今日缓存率 2025%。

## 正确定义
缓存率 = 命中缓存占**总输入**比 = `cache_read / (input + cache_read) * 100`，恒 ≤100%。

## 改动（4 处，全 `src-tauri/src/gateway/db.rs`）
1. line 1003 today stats: `cache_tokens / input_tokens` → `cache_tokens / (input_tokens + cache_tokens)`
2. line 2361 group stats: `cache / inp` → `cache / (inp + cache)`
3. line 2441 group stats: 同
4. line 2633 stats overview: `row(4) / inp` → `row(4) / (inp + row(4))`

分母 0 时仍 0.0。

## 验证
- cargo test：加 cache_rate 单元测试（cache>input 场景验证 ≤100%）。
- clippy 干净。

## 非目标
- 不改 cache_tokens 语义（仍 cache_read）。
- 不引入 cache_creation（当前不记）。
