#!/usr/bin/env bash
# 票 06 内存曲线。每次 run 都从模板库冷启动一个新内核（避免上一轮分配器保留污染下一轮）。
#   PERF_ID=mem  PROXY_PORT=19894 MGMT_PORT=19895 MOCK_PORT=18904 mem06.sh template 1   # 日志全开模板
#   PERF_ID=memoff PROXY_PORT=19896 MGMT_PORT=19897 MOCK_PORT=18906 mem06.sh template 0 # 日志全关模板
#   <同上 env> mem06.sh run <label> <c> <body.json> [dur=25s] [after_s=120] [heap=0]
# 输出 results/memory/<label>.{tsv,oha.json,fp-*.txt,vmmap-*.txt,heap-*.txt}
# 额外 env（原样传给内核，只作用于自己的实例）：MallocNanoZone 等；MOCK env 见 mock_upstream.mjs。
set -euo pipefail; source "$(dirname "$0")/env.sh"
unset HTTP_PROXY HTTPS_PROXY ALL_PROXY http_proxy https_proxy all_proxy
OUT="$HARNESS_DIR/../results/memory"; mkdir -p "$OUT"
H=$HARNESS_DIR; TPL="$ISO.tpl"
rows() { sqlite3 "file:$ISO/.aidog/log.db?mode=ro" "SELECT count(*), coalesce(sum(done),0) FROM proxy_log" 2>/dev/null | tr '|' ' ' || echo "- -"; }
snap() { # snap <tag>：footprint + vmmap 摘要（+ 可选 heap）
  local k; k=$(cat "$ISO/kernel.pid")
  /usr/bin/footprint -p "$k" > "$OUT/$label.fp-$1.txt" 2>&1 || true
  /usr/bin/vmmap --summary "$k" > "$OUT/$label.vmmap-$1.txt" 2>&1 || true
  [ "$HEAP" = 1 ] && { /usr/bin/heap -s "$k" > "$OUT/$label.heap-$1.txt" 2>&1 || true; }
  return 0
}
case $1 in
template)
  lb=$2; [ -d "$TPL" ] && { echo "template exists: $TPL"; exit 0; }
  rm -rf "$ISO"; "$H/start.sh"; LOG_BODIES=$lb "$H/seed.sh"; "$H/stop.sh"
  cp -R "$ISO/.aidog" "$TPL"; echo "template → $TPL";;
run)
  label=$2 c=$3 body=$4 dur=${5:-25s} after=${6:-120} HEAP=${7:-0}
  "$H/stop.sh" >/dev/null 2>&1 || true
  rm -rf "$ISO/.aidog"; mkdir -p "$ISO"; cp -R "$TPL" "$ISO/.aidog"
  "$H/start.sh" >/dev/null; K=$(cat "$ISO/kernel.pid")
  sleep 20
  # 采样器：每秒一行 t_s  phys_footprint_MB  rows  rows_done  kernel_cpu_s
  ( t0=$(date +%s); while kill -0 "$K" 2>/dev/null && [ ! -f "$OUT/.stop-$label" ]; do
      read -r cpu _ _ fp < <(python3 "$H/rusage.py" "$K"); printf '%s\t%.1f\t%s\t%s\n' $(( $(date +%s)-t0 )) "$(echo "$fp/1048576"|bc -l)" "$(rows | tr ' ' '\t')" "$cpu"
      sleep 1; done ) > "$OUT/$label.tsv" &
  SP=$!
  snap idle
  d=${dur%s}
  ( sleep $((d - 5)); snap peak ) & SNP=$!
  oha --no-tui --output-format json -m POST -T application/json -H 'x-api-key: perf-conv' \
    -H 'anthropic-version: 2023-06-01' -D "$body" -z "$dur" -w -c "$c" "http://127.0.0.1:$PROXY_PORT/v1/messages" > "$OUT/$label.oha.json"
  wait $SNP 2>/dev/null || true
  snap end
  # PLATEAU=<s>：负载结束后第 s 秒再拍一组（回落平台期），并 /usr/bin/sample 10 s 看谁在忙
  if [ -n "${PLATEAU:-}" ]; then sleep "$PLATEAU"; snap plateau; /usr/bin/sample "$(cat "$ISO/kernel.pid")" 10 -mayDie -file "$OUT/$label.sample-plateau.txt" >/dev/null 2>&1 || true; after=$((after - PLATEAU - 10)); fi
  sleep "$after"
  snap after
  touch "$OUT/.stop-$label"; wait $SP 2>/dev/null || true; rm -f "$OUT/.stop-$label"
  pk=$(awk -F'\t' 'BEGIN{m=0}{if($2>m)m=$2}END{print m}' "$OUT/$label.tsv")
  last=$(tail -1 "$OUT/$label.tsv" | cut -f2)
  lifepk=$(grep -m1 -i 'phys_footprint_peak' "$OUT/$label.fp-after.txt" | awk '{print $2,$3}')
  printf '%s\tc=%s\tbody=%s\t%s\tsampled_peak_MB=%s\tafter%ss_MB=%s\tlifetime_peak=%s\n' "$label" "$c" "$(basename "$body")" \
    "$(jq -r '"rps=\(.summary.requestsPerSec|floor) p99_ms=\(.latencyPercentiles.p99*1000|floor) status=\(.statusCodeDistribution|tostring)"' "$OUT/$label.oha.json")" \
    "$pk" "$after" "$last" "$lifepk" | tee -a "$OUT/summary.tsv"
  "$H/stop.sh" >/dev/null;;
*) echo "unknown $1"; exit 1;;
esac
