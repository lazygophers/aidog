-- Migration 011 (file) / 032 (inline): 小时级聚合统计表 stats_agg_hourly。
--
-- 目的：统计读取（today_stats / today_platform_stats / group usage / Stats hourly+daily）
-- 从逐请求扫 proxy_log 改为查预聚合表，且【不受 ProxyLogSettings.enabled 日志开关影响】
-- （关日志也写聚合）。聚合粒度 = 本地时区小时桶 × model × group_key × eff_pid(回溯后平台)。
--
-- 列语义：
--  - time_hour: 本地时区小时桶 "YYYY-MM-DD HH:00:00"（与 bucket_time_expr 'localtime' 对齐）。
--  - model: actual_model 非空优先，否则 model（与 Stats actual_model 优先一致）。
--  - group_key: proxy_log.group_key（gk_<hex>，非显示名）。
--  - platform_id: 存 eff_pid（platform_id=0 经 group.auto_from_platform 回溯后的源平台 id）。
--  - sum_duration_ms 用 SUM 非 AVG，便于跨桶再聚合；avg 在查询时 = sum/request_count。
--  - success_count = 2xx；error_count = 终态非 2xx（status_code 不在 200..300）。
-- UNIQUE(time_hour,model,group_key,platform_id) 是 upsert 的 ON CONFLICT 目标键。
CREATE TABLE IF NOT EXISTS stats_agg_hourly (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    time_hour         TEXT NOT NULL,
    model             TEXT NOT NULL DEFAULT '',
    group_key         TEXT NOT NULL DEFAULT '',
    platform_id       INTEGER NOT NULL DEFAULT 0,
    request_count     INTEGER NOT NULL DEFAULT 0,
    success_count     INTEGER NOT NULL DEFAULT 0,
    error_count       INTEGER NOT NULL DEFAULT 0,
    sum_input_tokens  INTEGER NOT NULL DEFAULT 0,
    sum_output_tokens INTEGER NOT NULL DEFAULT 0,
    sum_cache_tokens  INTEGER NOT NULL DEFAULT 0,
    sum_est_cost      REAL NOT NULL DEFAULT 0,
    sum_duration_ms   INTEGER NOT NULL DEFAULT 0,
    created_at        INTEGER NOT NULL DEFAULT 0,
    updated_at        INTEGER NOT NULL DEFAULT 0,
    deleted_at        INTEGER NOT NULL DEFAULT 0,
    UNIQUE(time_hour, model, group_key, platform_id)
);

CREATE INDEX IF NOT EXISTS idx_stats_agg_time     ON stats_agg_hourly(time_hour);
CREATE INDEX IF NOT EXISTS idx_stats_agg_model    ON stats_agg_hourly(model);
CREATE INDEX IF NOT EXISTS idx_stats_agg_group    ON stats_agg_hourly(group_key);
CREATE INDEX IF NOT EXISTS idx_stats_agg_platform ON stats_agg_hourly(platform_id);

-- 一次性回填：把存量 proxy_log 按 (本地小时桶, actual_model优先, group_key, eff_pid) 聚合写入。
-- 幂等：仅当 stats_agg_hourly 为空时回填（NOT EXISTS 守卫），避免重复执行翻倍。
-- eff_pid 回溯：platform_id=0 时经 group.auto_from_platform（十进制字符串）回溯到源平台。
-- 仅聚合 deleted_at=0 的有效日志。2xx → success，终态非 2xx → error。
INSERT INTO stats_agg_hourly
    (time_hour, model, group_key, platform_id,
     request_count, success_count, error_count,
     sum_input_tokens, sum_output_tokens, sum_cache_tokens,
     sum_est_cost, sum_duration_ms, created_at, updated_at, deleted_at)
SELECT
    strftime('%Y-%m-%d %H:00:00', created_at/1000, 'unixepoch', 'localtime') AS time_hour,
    CASE WHEN actual_model != '' THEN actual_model ELSE model END AS model,
    group_key,
    CASE WHEN platform_id = 0 THEN COALESCE(
        (SELECT CAST(g.auto_from_platform AS INTEGER)
         FROM "group" g
         WHERE g.group_key = proxy_log.group_key
           AND g.auto_from_platform != ''
           AND g.deleted_at = 0
         LIMIT 1), 0)
    ELSE platform_id END AS eff_pid,
    COUNT(*),
    SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END),
    SUM(CASE WHEN status_code < 200 OR status_code >= 300 THEN 1 ELSE 0 END),
    COALESCE(SUM(input_tokens), 0),
    COALESCE(SUM(output_tokens), 0),
    COALESCE(SUM(cache_tokens), 0),
    COALESCE(SUM(est_cost), 0.0),
    COALESCE(SUM(duration_ms), 0),
    CAST(strftime('%s','now') AS INTEGER) * 1000,
    CAST(strftime('%s','now') AS INTEGER) * 1000,
    0
FROM proxy_log
WHERE deleted_at = 0
  AND NOT EXISTS (SELECT 1 FROM stats_agg_hourly LIMIT 1)
-- 位置引用 1..4 绑定到 SELECT 输出表达式（time_hour / model别名 / group_key / eff_pid）。
-- 不可写 `GROUP BY ..., model, ...`：SQLite 会把裸 `model` 优先绑定到 proxy_log 真实列，
-- 而 SELECT/UNIQUE 用的是 `CASE actual_model 非空优先` 别名；两个 raw model 映射到同一
-- actual_model 时聚合后输出同一复合键 → 撞 UNIQUE(time_hour,model,group_key,platform_id)。
GROUP BY 1, 2, 3, 4;
