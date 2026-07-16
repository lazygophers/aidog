---
title: 拆库 crash-safe 四阶段迁移模式
layer: recall
category: db
keywords: [db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等]
source: config-db-split
authored-by: skein-memory
created: 1784181958
---

# 拆库 crash-safe 四阶段迁移模式

何时被读: 表从一个 SQLite 库迁移到另一个库（主库→log.db / platform.db），需迁存量数据 + DROP 源表时
谁读: trellis-implement sub-agent / main
不遵守的代价: 迁移中途 crash（断电/进程 kill）→ 源表已 DROP 但目标库未写完 → **数据永久丢失，不可逆**

---

## 禁用模式（❌）

`read → DROP 源表 → INSERT 目标库`（notification migration 049 原模式）：
- DROP 与 INSERT 分属不同连接闭包 / 不同物理库
- 中间 crash：源表已删，目标库未写 → 数据丢
- transient 数据（通知）可容忍；**用户配置数据（platform/group）禁用**

## MUST 四阶段模式（✅）

```
Phase 1: read-without-drop（源库读全行入 Vec，不 DROP）
Phase 2: 目标库先建表（run_migrations_*）
Phase 3: INSERT OR IGNORE 目标库（保原 id，幂等）
Phase 4: 验证目标库写入成功后 → DROP 源表（仅 Phase 3 成功才达此）
```

关键点：
- **read-without-drop**：Phase 1 只读不删，源表保留兜底
- **INSERT OR IGNORE**：保原 id 幂等插入（id PK 冲突跳过），迁移可任意重放
- **DROP 延后到 Phase 4**：确认目标库数据落地后才清源
- **内存库短路**：`is_memory()=true` 时 **MUST 跳过 DROP**（shared connection 下 DROP 等于拆整个库，见 [[dual-db-aggregate-is-memory-shortcut]]）

## crash 恢复矩阵

| crash 点 | 重启行为 |
|---|---|
| Phase 1 前/中 | 源表在，重读 |
| Phase 3 前 | 源表在，重读 → INSERT OR IGNORE |
| Phase 3 INSERT 中（部分） | 源表在，重读 → OR IGNORE 跳已存补缺 |
| Phase 4 前 | 目标已全量，源表仍在 → 重读 OR IGNORE 全跳 → DROP |
| Phase 4 后 | 源表已无，重读 Vec 空 → 全 no-op |

## 保 id（MUST）

`INSERT INTO platform SELECT *` / 显式列含 id 保原 id。log.db.proxy_log.platform_id 等数值游离引用（无 FK）继续指向正确行。自增 id → platform_id 全错位 → 历史日志平台名解析崩。

## 实例

task config-db-split（s2）：platform / group / group_platform / cli_proxy_provider 4 表从 aidog.db 拆到 platform.db，采用四阶段模式。

## Cross-ref

- [[db-handle-ownership-audit-three-forms]]（访问点审计）
- [[dual-db-aggregate-is-memory-shortcut]]（内存库短路）
