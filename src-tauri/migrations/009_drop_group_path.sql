-- Migration 009: 移除 group.path，分组路由纯按 apikey(group.name)。
-- 原 path 用作 URL 前缀路由 fallback + UNIQUE 标识；现统一按
-- Authorization Bearer(apikey = group.name) 精确匹配，不再支持路径前缀路由。
-- SQLite 不能直接 DROP COLUMN + 加表级约束，重建表。
-- group 无独立 index，重建不丢索引；列名显式匹配保证幂等（path 已删的库 SELECT 同样命中）。
-- name 加 UNIQUE（apikey 语义唯一，防重名 group 创建）。
CREATE TABLE IF NOT EXISTS "group_new" (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    name                 TEXT NOT NULL DEFAULT '',
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
    UNIQUE(name)
);
INSERT INTO "group_new"
    (id, name, routing_mode, auto_from_platform, source_protocol, model_mappings,
     request_timeout_secs, connect_timeout_secs, created_at, updated_at, deleted_at,
     sort_order, max_retries)
SELECT
    id, name, routing_mode, auto_from_platform, source_protocol, model_mappings,
    request_timeout_secs, connect_timeout_secs, created_at, updated_at, deleted_at,
    sort_order, max_retries
FROM "group";
DROP TABLE "group";
ALTER TABLE "group_new" RENAME TO "group";
