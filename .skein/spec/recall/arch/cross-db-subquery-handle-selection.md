---
title: 跨库补查闭包 handle 按补查表归属
layer: recall
category: arch
keywords: [db,sqlite,跨库,补查,handle,闭包,cpp,平台名,N+1]
source: config-db-split
authored-by: skein-memory
created: 1784181958
---

# 跨库补查闭包 handle 按补查表归属

何时被读: 跨库查询（主表查 A 库 + 补查 B 库表，如 proxy_log(log.db) 补查 platform/group 名(platform.db)）时
谁读: trellis-implement sub-agent / main
不遵守的代价: 补查闭包复用主表 handle → conn 指向错库 → `no such table` 或查空

---

## MUST 规则

跨库补查闭包的 handle **必须按补查表的库归属选**，禁顺手复用主表 handle。

## 错误样本（❌）

```rust
// proxy_log 在 log.db，补查 cpp.name 在 platform.db
proxy_log_handle.call_read_traced(|conn| {
    let logs = conn.query("SELECT ... FROM proxy_log", ...)?;
    for log in logs {
        // conn 仍是 log.db → 访问 platform.db 的 cpp 表失败
        let name = conn.query_row("SELECT name FROM cli_proxy_provider WHERE id=?", [log.cpp_id])?;
    }
})
```

## 正确写法（✅）

```rust
// 主查走 log.db handle
let logs = proxy_log_handle.call_read_traced(|c| c.query("SELECT ... FROM proxy_log", ...))?;

// 收集补查 id，切到补查表所属库的 handle
let cpp_ids: Vec<i64> = logs.iter().map(|l| l.cpp_id).collect();
let names = platform_handle.call_read_traced(|c| {
    c.query("SELECT id, name FROM cli_proxy_provider WHERE id IN rarray(?)", [&cpp_ids])
})?;

// Rust 内存 map 合并（跨库禁 JOIN，见 sqlite-cross-db-no-join）
```

关键：**主查闭包与补查闭包分离，各自用所属库 handle**；合并走 Rust 内存 map。

## 验收

```bash
# 找跨库补查点（同函数 / 同闭包内出现多库表名）
grep -rn 'FROM "proxy_log"' src-tauri/crates/aidog_core/src | grep -E "cli_proxy_provider|platform|\"group\""
```

## Cross-ref

- sqlite-cross-db-no-join（跨库禁 JOIN，强制拆闭包 + Rust 合并）
- [[auto-fix-downgrade-34]]（访问点审计总则）
