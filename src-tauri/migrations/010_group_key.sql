-- Migration 010: group 拆 group_key（密钥/路由/日志归属键）+ name（显示名）。
-- group_key UNIQUE: Bearer token + 路由匹配键 + proxy_log 归属键（前端按 group_key 反查 name 显示）。
-- name UNIQUE: 防重名显示。
-- 老 group.group_key 初值 = 旧 name（statusline 脚本 / 已分发 token 不破，用户后续可改）。
-- SQLite 不能给现存表加 UNIQUE 列约束，重建表（仿 009_drop_group_path.sql 幂等范式）。
CREATE TABLE IF NOT EXISTS "group_new" (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    name                 TEXT NOT NULL DEFAULT '',
    group_key            TEXT NOT NULL DEFAULT '',
    routing_mode         TEXT NOT NULL DEFAULT '',
    auto_from_platform   TEXT NOT NULL DEFAULT '',
    source_protocol      TEXT NOT NULL DEFAULT 'anthropic',
    model_mappings       TEXT NOT NULL DEFAULT '[]',
    request_timeout_secs INTEGER NOT NULL DEFAULT 0,
    connect_timeout_secs INTEGER NOT NULL DEFAULT 0,
    created_at           INTEGER NOT NULL DEFAULT 0,
    updated_at           INTEGER NOT NULL DEFAULT 0,
    deleted_at           INTEGER NOT NULL DEFAULT 0,
    sort_order           INTEGER NOT NULL DEFAULT 0,
    max_retries          INTEGER NOT NULL DEFAULT 2,
    UNIQUE(name),
    UNIQUE(group_key)
);
-- 仅当源表存在 group_key 列时取已存值，否则用 name 兜底（首次迁移 + 兼容已迁移库）。
-- SQLite 无 IF_COL_EXISTS；靠列名显式匹配：旧库无 group_key → 用 name 作 group_key。
INSERT INTO "group_new"
    (id, name, group_key, routing_mode, auto_from_platform, source_protocol, model_mappings,
     request_timeout_secs, connect_timeout_secs, created_at, updated_at, deleted_at,
     sort_order, max_retries)
SELECT
    id, name, name, routing_mode, auto_from_platform, source_protocol, model_mappings,
    request_timeout_secs, connect_timeout_secs, created_at, updated_at, deleted_at,
    sort_order, max_retries
FROM "group";
DROP TABLE "group";
ALTER TABLE "group_new" RENAME TO "group";
