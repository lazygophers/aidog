-- Group multi-platform retry + 401/403 auto-disable + per-attempt logging.
--
-- platform: bool `enabled` → 三态 `status`（enabled/disabled/auto_disabled）+ 退避字段。
--   `enabled` 列保留向后兼容（旧读者）；写入端从 status 同步。
--   迁移：现有 enabled=0 → status='disabled'，否则保持默认 'enabled'（绝不误判为 auto_disabled）。
-- group: 加分组级 `max_retries`（默认 2）。
-- proxy_log: 加 `attempts` JSON 数组列 + `retry_count`（= attempts.len()-1）。
--
-- NOTE: 实际执行在 db.rs::init_tables 内以逐条 `ALTER TABLE ... ADD COLUMN`（忽略 duplicate column）
-- 幂等方式进行，与既有 004-010 迁移风格一致。本文件为 schema 变更的权威记录。

ALTER TABLE platform ADD COLUMN status TEXT NOT NULL DEFAULT 'enabled';
ALTER TABLE platform ADD COLUMN auto_disabled_until INTEGER NOT NULL DEFAULT 0;
ALTER TABLE platform ADD COLUMN auto_disable_strikes INTEGER NOT NULL DEFAULT 0;
UPDATE platform SET status = 'disabled' WHERE enabled = 0 AND status = 'enabled';

ALTER TABLE "group" ADD COLUMN max_retries INTEGER NOT NULL DEFAULT 2;

ALTER TABLE proxy_log ADD COLUMN attempts TEXT NOT NULL DEFAULT '[]';
ALTER TABLE proxy_log ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0;
