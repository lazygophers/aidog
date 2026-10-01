#!/usr/bin/env bash
# 票 02：-c 50 perf-conv 280k 流式压测期间采样内核 30 s。需用带符号内核（KERNEL_BIN=/tmp/aidog-perf-prof-target/release/aidog-kernel）起实例。
# 用法: profile02.sh samply|sample
set -euo pipefail; source "$(dirname "$0")/env.sh"
unset HTTP_PROXY HTTPS_PROXY ALL_PROXY http_proxy https_proxy all_proxy
O=${OUT:-$HARNESS_DIR/results}; K=$(cat "$ISO/kernel.pid")
oha -z 45s -w -c 50 --no-tui --output-format json -m POST -T application/json -H 'x-api-key: perf-conv' \
  -H 'anthropic-version: 2023-06-01' -D "$BODIES/body-280k.json" "http://127.0.0.1:$PROXY_PORT/v1/messages" > "$O/profile-c50-$1.oha.json" &
sleep 8
case $1 in
samply) samply record -p "$K" -d 30 -s -o "$O/profile-c50.json.gz" > "$O/profile-c50-samply.log" 2>&1 || echo "samply exit $?";;
sample) /usr/bin/sample "$K" 30 -mayDie -file "$O/profile-c50-sample.txt" > /dev/null 2>&1 || echo "sample exit $?";;
esac
wait
jq -c '{rps: .summary.requestsPerSec, status: .statusCodeDistribution}' "$O/profile-c50-$1.oha.json"
