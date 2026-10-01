#!/usr/bin/env bash
# oha 单请求打两组（280 KB 流式体），核对：HTTP 200、隔离 log.db 有 status=200 done=1 行、假上游计数 +1。
set -euo pipefail; source "$(dirname "$0")/env.sh"
BODY="${BODY:-$BODIES/body-280k.json}"
[ -f "$BODY" ] || node "$HARNESS_DIR/gen_bodies.mjs" "$BODIES"
curl -sf -X POST "http://127.0.0.1:$MOCK_PORT/stats/reset"
for g in perf-conv perf-pass; do
  echo "== $g"
  oha -n 1 --no-tui --output-format json -m POST -T application/json \
    -H "x-api-key: $g" -H "anthropic-version: 2023-06-01" -D "$BODY" \
    "http://127.0.0.1:$PROXY_PORT/v1/messages" | jq -c '{success: .summary.successRate, status: .statusCodeDistribution}'
done
sleep 1 # 流式终态行异步写库
echo "== mock upstream stats"; curl -s "http://127.0.0.1:$MOCK_PORT/stats"; echo
echo "== proxy_log (isolated $ISO/.aidog/log.db)"
sqlite3 -header -column "$ISO/.aidog/log.db" \
  "SELECT id, group_key, is_stream, source_protocol, target_protocol, actual_model, status_code, upstream_status_code, done, input_tokens, output_tokens, duration_ms, length(request_body) req_b, length(upstream_request_body) up_b, upstream_request_url FROM proxy_log ORDER BY created_at DESC LIMIT 4;"
