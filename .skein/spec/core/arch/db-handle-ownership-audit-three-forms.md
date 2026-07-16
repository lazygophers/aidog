---
title: DB 拆库访问点归属审计三形式
layer: core
category: arch
keywords: [db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn,漏网,归属]
source: config-db-split
authored-by: skein-memory
created: 1784181958
---

# DB 拆库访问点归属审计三形式

何时被读: 表从一个 SQLite 库拆到另一个库（主库→log.db / platform.db），需把该表所有访问点切到新 handle 时
谁读: trellis-implement sub-agent / main
不遵守的代价: 访问点遗漏 → 运行时查主库已 DROP 的表 → `no such table` 崩

---

## MUST 审计三形式（缺一不可）

拆库后访问点切换 **禁只查 `call_*_traced` chokepoint**，必须同时查三种形式：

1. **`call_*_traced` / `call_read_*_traced` wrapper 形式**：`call_platform_traced` / `call_group_traced` 等命名阻塞点
2. **`.write_conn()` / `.read_conn()` 直访形式**：绕过 wrapper 直接拿连接写（estimate.rs / manual_budget.rs 热路径常见）
3. **`conn` 直接持有形式**：某些路径直接持有 `Connection` 引用做查询

## 漏网样本（task config-db-split s3→s4）

- `estimate.rs` 6 处 `.write_conn()` 写 `group` 表，s3 只 grep `call_traced` 未覆盖 → 漏切 handle
- 补救：s4 加 `grep -rn "\.write_conn\|\.read_conn"` 才发现

## 验收命令（三形式全查）

```bash
# 1. wrapper 形式
grep -rn "call_platform_traced\|call_group_traced" src-tauri

# 2. 直访形式（必须！）
grep -rn "\.write_conn\|\.read_conn" src-tauri/crates/aidog_core/src/gateway

# 3. 裸 SQL（按被拆表名 grep，覆盖所有 conn 引用路径）
grep -rn 'FROM "group"\|FROM platform\|FROM group_platform\|FROM cli_proxy_provider' src-tauri/crates/aidog_core/src
```

三方命中并集 = 访问点总数。漏任一形式 → 必有遗漏。

## 反例（禁）

- ❌ 只 grep `call_traced` → 6 处 `write_conn` 漏网（s3 错误模式）
- ❌ 只 grep helper 函数名（`load_auto_from_map`）→ 裸 SQL 漏查（见 [[non-typical-sql-audit-pattern]]）
- ❌ 人工 audit → 人肉易疏漏，必须 grep 机器穷举

## Cross-ref

- [[crash-safe-db-split-migration]]（同 task 迁移模式）
- [[non-typical-sql-audit-pattern]]（裸 SQL 形态审计）
- [[cross-db-subquery-handle-selection]]（跨库补查 handle 选择）
