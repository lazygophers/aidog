-- Migration 004: platform 预估列（请求驱动 quota 预估增量更新）
-- 旧库补列。ALTER 无 IF NOT EXISTS，由 init_tables 用 `let _ = execute(...)` 忽略 duplicate column 错。
-- 新库由 001_init.sql 直接含这 4 列，本脚本仅记录列定义（不在 execute_batch 中执行，逐条 execute 以忽略重复列）。
ALTER TABLE platform ADD COLUMN est_balance_remaining REAL NOT NULL DEFAULT 0;
ALTER TABLE platform ADD COLUMN est_coding_plan TEXT NOT NULL DEFAULT '';
ALTER TABLE platform ADD COLUMN last_real_query_at INTEGER NOT NULL DEFAULT 0;
ALTER TABLE platform ADD COLUMN estimate_count INTEGER NOT NULL DEFAULT 0;
