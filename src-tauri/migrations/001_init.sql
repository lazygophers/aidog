-- AiDog 初始化表结构

PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

-- AI 平台
CREATE TABLE IF NOT EXISTS platforms (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    protocol    TEXT NOT NULL,
    base_url    TEXT NOT NULL,
    api_key     TEXT NOT NULL,
    extra       TEXT,
    enabled     INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- 分组
CREATE TABLE IF NOT EXISTS groups (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    path         TEXT NOT NULL UNIQUE,
    routing_mode TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);

-- 分组-平台关联
CREATE TABLE IF NOT EXISTS group_platforms (
    group_id    TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    platform_id TEXT NOT NULL REFERENCES platforms(id) ON DELETE CASCADE,
    priority    INTEGER NOT NULL DEFAULT 0,
    weight      INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (group_id, platform_id)
);

-- 模型映射
CREATE TABLE IF NOT EXISTS model_mappings (
    id                 TEXT PRIMARY KEY,
    group_id           TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    source_model       TEXT NOT NULL,
    target_platform_id TEXT NOT NULL REFERENCES platforms(id),
    target_model       TEXT NOT NULL,
    UNIQUE(group_id, source_model)
);
