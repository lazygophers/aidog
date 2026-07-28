#!/usr/bin/env bash
# EXPLAIN QUERY PLAN 基线复跑脚本 —— logs-query-ipc-slimming/s1-explain-baseline
# 全程只读连接（mode=ro），不写用户库。一条命令重跑全部三条重点查询 + 索引清单。
set -euo pipefail

LOG_DB="file:$HOME/.aidog/log.db?mode=ro"
AIDOG_DB="file:$HOME/.aidog/aidog.db?mode=ro"
PLATFORM_DB="file:$HOME/.aidog/platform.db?mode=ro"

hr() { echo "----------------------------------------------------------------"; }

echo "=== 索引清单 ==="
for pair in "log:$LOG_DB" "aidog:$AIDOG_DB" "platform:$PLATFORM_DB"; do
  name="${pair%%:*}"; uri="${pair#*:}"
  echo "--- $name.db ---"
  sqlite3 "$uri" "SELECT name, sql FROM sqlite_master WHERE type='index';"
done
hr

echo "### [1] Logs 分页查询 (log.db, proxy_log ${1:-18174} 行) ###"
echo "-- 1a. filtered_list_proxy_logs（默认 filter exclude_sources=[test,quota]） --"
sqlite3 "$LOG_DB" "EXPLAIN QUERY PLAN
SELECT id, group_key, model, actual_model, source_protocol, target_protocol, platform_id, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, is_stream, retry_count, created_at
FROM proxy_log WHERE deleted_at = 0 AND (source_protocol NOT IN ('test','quota') OR source_protocol IS NULL) ORDER BY created_at DESC LIMIT 20 OFFSET 0;"

echo "-- 1b. filtered_count_proxy_logs（同 filter，转发期每 500ms 轮询一次） --"
sqlite3 "$LOG_DB" "EXPLAIN QUERY PLAN
SELECT COUNT(*) FROM proxy_log WHERE deleted_at = 0 AND (source_protocol NOT IN ('test','quota') OR source_protocol IS NULL);"

echo "-- 1c. list_proxy_logs（无 filter 变体） --"
sqlite3 "$LOG_DB" "EXPLAIN QUERY PLAN
SELECT id, group_key, model, actual_model, source_protocol, target_protocol, platform_id, status_code, duration_ms, input_tokens, output_tokens, cache_tokens, is_stream, retry_count, created_at
FROM proxy_log WHERE deleted_at = 0 ORDER BY created_at DESC LIMIT 20 OFFSET 0;"
hr

echo "### [2] Stats 聚合查询 (aidog.db, stats_agg_hourly 703 行；默认 7d daily 粒度) ###"
START="$(date -u -v-7d +%Y-%m-%d-00 2>/dev/null || date -u -d '-7 days' +%Y-%m-%d-00)"
END="$(date -u +%Y-%m-%d-23)"

echo "-- 2a. overview (SUM 聚合) --"
sqlite3 "$AIDOG_DB" "EXPLAIN QUERY PLAN
SELECT COALESCE(SUM(request_count),0), COALESCE(SUM(success_count),0), COALESCE(SUM(sum_input_tokens),0), COALESCE(SUM(sum_output_tokens),0), COALESCE(SUM(sum_cache_tokens),0), COALESCE(SUM(sum_duration_ms),0), COALESCE(SUM(sum_est_cost),0.0)
FROM stats_agg_hourly WHERE deleted_at = 0 AND time_hour >= '$START' AND time_hour <= '$END';"

echo "-- 2b. time buckets (daily) --"
sqlite3 "$AIDOG_DB" "EXPLAIN QUERY PLAN
SELECT substr(time_hour,1,10) AS b, COALESCE(SUM(request_count),0), COALESCE(SUM(success_count),0), COALESCE(SUM(error_count),0), COALESCE(SUM(sum_input_tokens),0), COALESCE(SUM(sum_output_tokens),0), COALESCE(SUM(sum_cache_tokens),0), COALESCE(SUM(sum_duration_ms),0), COALESCE(SUM(sum_est_cost),0.0)
FROM stats_agg_hourly WHERE deleted_at = 0 AND time_hour >= '$START' AND time_hour <= '$END' GROUP BY b ORDER BY b;"

echo "-- 2c. dimension by-platform (GROUP BY platform_id LIMIT 50) --"
sqlite3 "$AIDOG_DB" "EXPLAIN QUERY PLAN
SELECT platform_id AS pid, COALESCE(SUM(request_count),0) FROM stats_agg_hourly WHERE deleted_at = 0 AND time_hour >= '$START' AND time_hour <= '$END' GROUP BY platform_id ORDER BY 2 DESC LIMIT 50;"

echo "-- 2d. dimension by-group (GROUP BY group_key LIMIT 50) --"
sqlite3 "$AIDOG_DB" "EXPLAIN QUERY PLAN
SELECT group_key AS dim, COALESCE(SUM(request_count),0) FROM stats_agg_hourly WHERE deleted_at = 0 AND time_hour >= '$START' AND time_hour <= '$END' GROUP BY group_key ORDER BY 2 DESC LIMIT 50;"

echo "-- 2e. [附带/关联] platform_usage_stats_all 近5条健康度裸查 proxy_log（无 LIMIT，log.db） --"
sqlite3 "$LOG_DB" "EXPLAIN QUERY PLAN
SELECT platform_id, group_key, status_code FROM proxy_log WHERE deleted_at = 0 ORDER BY created_at DESC;"
hr

echo "### [3] 平台余额聚合 (platform.db, platform 8 行 / group 4 行 / group_platform 20 行) ###"
echo "-- 3a. list_platforms --"
sqlite3 "$PLATFORM_DB" "EXPLAIN QUERY PLAN
SELECT * FROM platform WHERE deleted_at = 0 ORDER BY sort_order, created_at;"

echo "-- 3b. list_groups --"
sqlite3 "$PLATFORM_DB" "EXPLAIN QUERY PLAN
SELECT * FROM \"group\" WHERE deleted_at = 0 ORDER BY sort_order, created_at;"

echo "-- 3c. list_group_platforms_for_groups（批量 IN） --"
sqlite3 "$PLATFORM_DB" "EXPLAIN QUERY PLAN
SELECT group_id, platform_id, priority, weight, level_priority FROM group_platform WHERE deleted_at = 0 AND group_id IN (1,2,3,4) ORDER BY group_id, priority;"
