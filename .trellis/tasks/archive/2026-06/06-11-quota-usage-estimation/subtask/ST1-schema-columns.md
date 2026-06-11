# ST1: platform 加预估列

- **目标**: platform 表加 4 预估列 + 全链路同步
- **产出**:
  - migrations/001_init.sql platform 表加 est_balance_remaining REAL DEFAULT 0 / est_coding_plan TEXT DEFAULT '' / last_real_query_at INTEGER DEFAULT 0 / estimate_count INTEGER DEFAULT 0
  - db.rs migration 004：init_tables 加最小 ALTER 块（`let _ = execute("ALTER TABLE platform ADD COLUMN ...")` 忽略 duplicate，4 列）
  - db.rs: PLATFORM_COLUMNS + PLATFORM_COLUMNS_PREFIXED + row_to_platform(新 index) + **get_group_platforms(:411)第二处 parser**(偏移+2) + create_platform(默认0/'')/update_platform(不覆盖预估列)
  - models.rs Platform + api.ts Platform 加 4 字段（est 前端只读）
- **验证**: cargo build + tsc 0；现有 platform CRUD/测试不破
- **资源**: research/02-platform-columns-migration.md、design.md、db-conventions
- **依赖**: 无
- **失败处理**: row index 错位 → 对照 PLATFORM_COLUMNS 列序；两处 parser 都改
