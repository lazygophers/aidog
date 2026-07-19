---
title: 拆库表跨库迁移 idiom (DDL迁回+ATTACH搬迁+幂等三守卫)
layer: core
category: arch
keywords: [db,sqlite,拆库,迁移,attach,搬迁,幂等,guard,yagni]
source: stats-agg-to-main-db
created: 1752940800
---

# 拆库表跨库迁移 idiom

何时被读: 表从 A 库迁回 B 库（如 stats_agg_hourly 从 log.db 迁主库），需 DDL 重建 + 数据搬迁时
谁读: 拆库/合库类 sub-agent / main
不遵守的代价: 搬迁中途崩溃丢数据 / fresh install 启崩 / 重复搬迁主键冲突

---

## MUST 三件套

1. **DDL 迁目标库 `CREATE IF NOT EXISTS`**：目标库 late migration 建表（编号紧随），源库 migration 建表段删
2. **数据搬迁 migration（ATTACH + INSERT OR IGNORE + 事务）**：
   ```sql
   ATTACH '{src_path}' AS src;
   BEGIN;
   INSERT OR IGNORE INTO t SELECT * FROM src.t;
   COMMIT;
   DETACH DATABASE 'src';
   ```
3. **幂等三守卫（全满足才搬）**：
   - 目标表非空跳（`SELECT COUNT(*) FROM t` > 0）
   - 内存库跳（`PRAGMA database_list` main file 空 / `:memory:` / `mode=memory` → 三 handle 共享连接无独立源库文件）
   - 源库无此表跳（ATTACH 后查 `src.sqlite_master`）

## 源库路径推导（禁跨文件改签名）

migration 函数内 `PRAGMA database_list` 取 main file 列 → `Path::new(main_file).parent().join("log.db")`（与 `Db::new` 派生同源）。避免改 Db 签名透传 path。

## 关键取舍

| 取舍 | 选 | 理由 |
|---|---|---|
| 搬迁后删源库旧表 | **不删** | YAGNI；源库下次 VACUUM 自然回收；跨库 DROP 增复杂度 |
| 失败阻断启动 | **不阻断** | `tracing::warn!` + DETACH 兜底 + 继续；数据可后续 rebuild |
| 搬迁用 ATTACH vs 读出再批量插 | ATTACH | 单 SQL 原子；批量插多轮 IPC 慢 |
| fresh install 回填读不存在的源表 | 加存在性 guard | Mig 051 回填 legacy proxy_log，fresh install 主库无 proxy_log → 跳过避免读崩 |

## 反例

- ❌ 搬迁用裸 `INSERT INTO t SELECT FROM src.t`（非 IGNORE）→ 重跑主键冲突
- ❌ 无内存库守卫 → 三 handle 共享内存连接 ATTACH 同库报错
- ❌ DDL 迁了但数据搬迁 migration 编号在 DDL 之前 → 搬到不存在的表
