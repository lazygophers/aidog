#!/usr/bin/env python3
"""一次性 DB 迁移脚本：旧 schema → v2 规范 schema。

转换内容（对应 prd 规范 R1-R10）：
- 表名复数→单数：platforms→platform, groups→"group", group_platforms→group_platform,
  proxy_logs→proxy_log, settings→setting；model_mappings 表删除（聚合进 group）
- 主键 TEXT(uuid) → INTEGER 自增（proxy_log 主键保留 TEXT，uuid 去连字符）
- 时间字段 TEXT(ISO) → INTEGER 毫秒时间戳
- platforms.protocol → platform_type
- 每表补 created_at/updated_at/deleted_at（INTEGER，默认 0）
- NULL → '' / 0（禁 NULL）
- model_mappings 聚合为 group.model_mappings JSON 数组

幂等：检测到已迁移（存在 platform 表且无 platforms 表）则跳过。
完成后请删除本脚本（不属 app 运行时）。
"""
import json
import os
import shutil
import sqlite3
import sys
from datetime import datetime, timezone

DB_PATH = os.path.expanduser(sys.argv[1] if len(sys.argv) > 1 else "~/.aidog/aidog.db")

NEW_SCHEMA = """
CREATE TABLE platform (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL DEFAULT '', platform_type TEXT NOT NULL DEFAULT '',
    base_url TEXT NOT NULL DEFAULT '', api_key TEXT NOT NULL DEFAULT '',
    extra TEXT NOT NULL DEFAULT '', models TEXT NOT NULL DEFAULT '{}',
    available_models TEXT NOT NULL DEFAULT '[]', endpoints TEXT NOT NULL DEFAULT '[]',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0,
    deleted_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE "group" (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL DEFAULT '', path TEXT NOT NULL DEFAULT '',
    routing_mode TEXT NOT NULL DEFAULT '', auto_from_platform TEXT NOT NULL DEFAULT '',
    source_protocol TEXT NOT NULL DEFAULT 'anthropic', model_mappings TEXT NOT NULL DEFAULT '[]',
    request_timeout_secs INTEGER NOT NULL DEFAULT 0, connect_timeout_secs INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0,
    deleted_at INTEGER NOT NULL DEFAULT 0, UNIQUE(path)
);
CREATE TABLE group_platform (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id INTEGER NOT NULL DEFAULT 0, platform_id INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 0, weight INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0,
    deleted_at INTEGER NOT NULL DEFAULT 0, UNIQUE(group_id, platform_id)
);
CREATE TABLE setting (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope TEXT NOT NULL DEFAULT '', key TEXT NOT NULL DEFAULT '',
    value TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0,
    deleted_at INTEGER NOT NULL DEFAULT 0, UNIQUE(scope, key)
);
CREATE TABLE proxy_log (
    id TEXT PRIMARY KEY,
    group_name TEXT NOT NULL DEFAULT '', model TEXT NOT NULL DEFAULT '',
    actual_model TEXT NOT NULL DEFAULT '', source_protocol TEXT NOT NULL DEFAULT '',
    target_protocol TEXT NOT NULL DEFAULT '', platform_id INTEGER NOT NULL DEFAULT 0,
    request_headers TEXT NOT NULL DEFAULT '{}', request_body TEXT NOT NULL DEFAULT '',
    upstream_request_headers TEXT NOT NULL DEFAULT '', upstream_request_body TEXT NOT NULL DEFAULT '',
    response_body TEXT NOT NULL DEFAULT '', request_url TEXT NOT NULL DEFAULT '',
    upstream_request_url TEXT NOT NULL DEFAULT '', upstream_response_headers TEXT NOT NULL DEFAULT '',
    upstream_status_code INTEGER NOT NULL DEFAULT 0, user_response_headers TEXT NOT NULL DEFAULT '',
    user_response_body TEXT NOT NULL DEFAULT '', status_code INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0, input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0, cache_tokens INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT 0,
    deleted_at INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_proxy_log_group ON proxy_log(group_name);
CREATE INDEX idx_proxy_log_created ON proxy_log(created_at);
"""


def to_ms(val):
    """ISO/datetime 字符串 → 毫秒时间戳；空/解析失败 → 0。"""
    if val is None:
        return 0
    s = str(val).strip()
    if not s:
        return 0
    for fmt in ("%Y-%m-%dT%H:%M:%S%z", "%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"):
        try:
            dt = datetime.strptime(s.replace("Z", "+0000"), fmt)
            if dt.tzinfo is None:
                dt = dt.replace(tzinfo=timezone.utc)
            return int(dt.timestamp() * 1000)
        except ValueError:
            continue
    # rfc3339 带微秒/冒号时区兜底
    try:
        dt = datetime.fromisoformat(s.replace("Z", "+00:00"))
        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=timezone.utc)
        return int(dt.timestamp() * 1000)
    except ValueError:
        return 0


def table_exists(cur, name):
    cur.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?", (name,))
    return cur.fetchone() is not None


def col_or(row, key, default):
    try:
        v = row[key]
    except (IndexError, KeyError):
        return default
    return default if v is None else v


def main():
    if not os.path.exists(DB_PATH):
        print(f"DB 不存在: {DB_PATH}")
        return 1

    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    if table_exists(cur, "platform") and not table_exists(cur, "platforms"):
        print("已迁移，跳过。")
        return 0
    if not table_exists(cur, "platforms"):
        print("无旧表 platforms，可能空库；建新 schema。")

    # 备份
    bak = DB_PATH + ".bak"
    shutil.copy2(DB_PATH, bak)
    print(f"备份: {bak}")

    counts_old = {}
    for t in ("platforms", "groups", "group_platforms", "model_mappings", "proxy_logs", "settings"):
        if table_exists(cur, t):
            counts_old[t] = cur.execute(f"SELECT COUNT(*) FROM {t}").fetchone()[0]

    # 建新表
    cur.executescript(NEW_SCHEMA)

    platform_id_map = {}  # old uuid -> new int
    group_id_map = {}

    # platform
    if table_exists(cur, "platforms"):
        for r in cur.execute("SELECT * FROM platforms").fetchall():
            cur.execute(
                """INSERT INTO platform
                   (name, platform_type, base_url, api_key, extra, models, available_models,
                    endpoints, enabled, created_at, updated_at, deleted_at)
                   VALUES (?,?,?,?,?,?,?,?,?,?,?,0)""",
                (
                    col_or(r, "name", ""), col_or(r, "protocol", ""), col_or(r, "base_url", ""),
                    col_or(r, "api_key", ""), col_or(r, "extra", ""),
                    col_or(r, "models", "{}"), col_or(r, "available_models", "[]"),
                    col_or(r, "endpoints", "[]"), col_or(r, "enabled", 1),
                    to_ms(col_or(r, "created_at", "")), to_ms(col_or(r, "updated_at", "")),
                ),
            )
            platform_id_map[r["id"]] = cur.lastrowid

    # model_mappings 聚合 (旧 group uuid -> mapping json 列表)
    mappings_by_group = {}
    if table_exists(cur, "model_mappings"):
        for r in cur.execute("SELECT * FROM model_mappings").fetchall():
            tgt_old = col_or(r, "target_platform_id", "")
            mappings_by_group.setdefault(r["group_id"], []).append({
                "source_model": col_or(r, "source_model", ""),
                "target_platform_id": platform_id_map.get(tgt_old, 0),
                "target_model": col_or(r, "target_model", ""),
                "request_timeout_secs": col_or(r, "request_timeout_secs", 0),
                "connect_timeout_secs": col_or(r, "connect_timeout_secs", 0),
            })

    # group
    if table_exists(cur, "groups"):
        for r in cur.execute("SELECT * FROM groups").fetchall():
            mj = json.dumps(mappings_by_group.get(r["id"], []), ensure_ascii=False)
            cur.execute(
                """INSERT INTO "group"
                   (name, path, routing_mode, auto_from_platform, source_protocol, model_mappings,
                    request_timeout_secs, connect_timeout_secs, created_at, updated_at, deleted_at)
                   VALUES (?,?,?,?,?,?,?,?,?,?,0)""",
                (
                    col_or(r, "name", ""), col_or(r, "path", ""), col_or(r, "routing_mode", ""),
                    col_or(r, "auto_from_platform", ""),
                    col_or(r, "source_protocol", "anthropic"), mj,
                    col_or(r, "request_timeout_secs", 0), col_or(r, "connect_timeout_secs", 0),
                    to_ms(col_or(r, "created_at", "")), to_ms(col_or(r, "updated_at", "")),
                ),
            )
            group_id_map[r["id"]] = cur.lastrowid

    # group_platform
    if table_exists(cur, "group_platforms"):
        for r in cur.execute("SELECT * FROM group_platforms").fetchall():
            gid = group_id_map.get(r["group_id"], 0)
            pid = platform_id_map.get(r["platform_id"], 0)
            if gid == 0 or pid == 0:
                continue
            cur.execute(
                """INSERT INTO group_platform
                   (group_id, platform_id, priority, weight, created_at, updated_at, deleted_at)
                   VALUES (?,?,?,?,0,0,0)""",
                (gid, pid, col_or(r, "priority", 0), col_or(r, "weight", 1)),
            )

    # setting
    if table_exists(cur, "settings"):
        for r in cur.execute("SELECT * FROM settings").fetchall():
            cur.execute(
                """INSERT INTO setting (scope, key, value, created_at, updated_at, deleted_at)
                   VALUES (?,?,?,?,?,0)""",
                (
                    col_or(r, "scope", ""), col_or(r, "key", ""), col_or(r, "value", "{}"),
                    to_ms(col_or(r, "updated_at", "")), to_ms(col_or(r, "updated_at", "")),
                ),
            )

    # proxy_log
    if table_exists(cur, "proxy_logs"):
        for r in cur.execute("SELECT * FROM proxy_logs").fetchall():
            new_id = str(col_or(r, "id", "")).replace("-", "")
            pid_old = col_or(r, "platform_id", "")
            pid = platform_id_map.get(pid_old, 0) if pid_old else 0
            ts = to_ms(col_or(r, "created_at", ""))
            cur.execute(
                """INSERT INTO proxy_log
                   (id, group_name, model, actual_model, source_protocol, target_protocol, platform_id,
                    request_headers, request_body, upstream_request_headers, upstream_request_body,
                    response_body, request_url, upstream_request_url, upstream_response_headers,
                    upstream_status_code, user_response_headers, user_response_body, status_code,
                    duration_ms, input_tokens, output_tokens, cache_tokens, created_at, updated_at, deleted_at)
                   VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,0)""",
                (
                    new_id, col_or(r, "group_name", ""), col_or(r, "model", ""),
                    col_or(r, "actual_model", ""), col_or(r, "source_protocol", ""),
                    col_or(r, "target_protocol", ""), pid,
                    col_or(r, "request_headers", "{}"), col_or(r, "request_body", ""),
                    col_or(r, "upstream_request_headers", ""), col_or(r, "upstream_request_body", ""),
                    col_or(r, "response_body", ""), col_or(r, "request_url", ""),
                    col_or(r, "upstream_request_url", ""), col_or(r, "upstream_response_headers", ""),
                    col_or(r, "upstream_status_code", 0), col_or(r, "user_response_headers", ""),
                    col_or(r, "user_response_body", ""), col_or(r, "status_code", 0),
                    col_or(r, "duration_ms", 0), col_or(r, "input_tokens", 0),
                    col_or(r, "output_tokens", 0), col_or(r, "cache_tokens", 0), ts, ts,
                ),
            )

    # drop 旧表
    for t in ("model_mappings", "group_platforms", "proxy_logs", "groups", "platforms", "settings"):
        if table_exists(cur, t):
            cur.execute(f"DROP TABLE {t}")

    conn.commit()

    # 校验
    print("迁移完成，行数校验:")
    checks = {
        "platforms": ("platform", counts_old.get("platforms", 0)),
        "groups": ('"group"', counts_old.get("groups", 0)),
        "group_platforms": ("group_platform", counts_old.get("group_platforms", 0)),
        "proxy_logs": ("proxy_log", counts_old.get("proxy_logs", 0)),
        "settings": ("setting", counts_old.get("settings", 0)),
    }
    ok = True
    for old_t, (new_t, old_n) in checks.items():
        new_n = cur.execute(f"SELECT COUNT(*) FROM {new_t}").fetchone()[0]
        flag = "OK" if new_n == old_n else "MISMATCH"
        if new_n != old_n:
            ok = False
        print(f"  {old_t}({old_n}) -> {new_t}({new_n}) {flag}")
    mm = counts_old.get("model_mappings", 0)
    print(f"  model_mappings({mm}) -> 聚合进 group.model_mappings")
    conn.close()
    print("成功。" if ok else "存在行数不一致，请检查备份。")
    return 0 if ok else 2


if __name__ == "__main__":
    sys.exit(main())
