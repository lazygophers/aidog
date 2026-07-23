---
title: 非典型 SQL 形态易漏审计
layer: recall
category: arch
keywords: [db,sqlite,sql,审计,helper,裸sql,grep,易漏,访问点]
source: config-db-split
authored-by: skein-memory
created: 1784181958
---

# 非典型 SQL 形态易漏审计

何时被读: 拆库审计某表访问点时
谁读: trellis-implement sub-agent / main
不遵守的代价: 只 grep helper 函数名 → 遗漏裸 SQL 调用 → handle 未切 → 运行时查错库

---

## MUST 审计两形态

拆库审计时 **禁只 grep helper 函数名**，必须同时查：

1. **Helper 函数形式**：`load_auto_from_map` / `save_auto_to_map` 等封装
2. **裸 SQL 形式**：`SELECT ... FROM "group" WHERE auto_from_platform` 等直接 query

## 漏网样本（task config-db-split s5）

- `SELECT ... FROM "group" WHERE auto_from_platform` 不经任何 helper，是裸 SQL
- 只 grep `load_auto_from_map` 会漏 → handle 未切

## 验收命令

```bash
# 按被拆表名 grep（FROM "table"），覆盖所有访问形态
grep -rn 'FROM "group"' src-tauri/crates/aidog_core/src
grep -rn "FROM platform " src-tauri/crates/aidog_core/src
grep -rn "FROM group_platform" src-tauri/crates/aidog_core/src
grep -rn "FROM cli_proxy_provider" src-tauri/crates/aidog_core/src
```

关键：**按表名 grep（`FROM "table"`）而非按 helper 函数名**，确保覆盖裸 SQL + helper 内部两路。

## Cross-ref

- [[auto-fix-downgrade-34]]（访问点审计总则，本文是其子形式之一）
