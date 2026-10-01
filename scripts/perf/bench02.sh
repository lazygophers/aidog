#!/usr/bin/env bash
# 票 02 基线测量。前提：start.sh 已起、seed.sh 已灌、启动后已静置 ≥40 s（避开启动期 registry 同步 / 备份）。
# 用法: bench02.sh <子命令> [参数]；结果写 $OUT（缺省 scripts/perf/results/，gitignored），汇总行追加 $OUT/<tag>.tsv
#   idle <tag> <秒>                          空载窗口：内核 CPU 秒 / 磁盘写字节
#   cpu  <tag> <label> <group> <body> <n> <c> 定量压测：内核 + 假上游 CPU / 磁盘写 / log.db 增量
#   lat  <tag> <label> <url> <key|-> <body> <n> oha -c 1 定量（非流式体：总时长≈首字节）
#   ttfb <tag> <label> <url> <key|-> <body> <n> curl 串行：time_starttransfer / time_total
#   conc <tag> <label> <group> <body> <c> <dur> 定时压测（吞吐 / p50 / p99 / 错误）+ CPU
set -euo pipefail; source "$(dirname "$0")/env.sh"
# 本机 HTTP_PROXY=127.0.0.1:7890 且 NO_PROXY 用分号（curl 不认）→ curl 会绕道本机代理；一律直连
unset HTTP_PROXY HTTPS_PROXY ALL_PROXY http_proxy https_proxy all_proxy
OUT="${OUT:-$HARNESS_DIR/results}"; mkdir -p "$OUT"
KPID=$(cat "$ISO/kernel.pid"); MPID=$(cat "$ISO/mock.pid")
ru() { python3 "$HARNESS_DIR/rusage.py" "$1"; }   # cpu_s disk_w disk_r footprint
pst() { ps -o time= -p "$1" | tr -d ' '; }
dbsz() { local s=0 f; for f in "$ISO/.aidog/log.db" "$ISO/.aidog/log.db-wal"; do [ -f "$f" ] && s=$((s + $(stat -f %z "$f"))); done; echo $s; }
rows() { sqlite3 "file:$ISO/.aidog/log.db?mode=ro" "SELECT count(*) FROM proxy_log"; }
PROXY="http://127.0.0.1:$PROXY_PORT/v1/messages"
oha_json() { # label group|key url body  extra-oha-args...
  local label=$1 key=$2 url=$3 body=$4; shift 4
  local hk=(); [ "$key" != - ] && hk=(-H "x-api-key: $key")
  oha --no-tui --output-format json -m POST -T application/json ${hk[@]+"${hk[@]}"} \
    -H 'anthropic-version: 2023-06-01' -D "$body" "$@" "$url" > "$OUT/$label.oha.json"
}
summ() { jq -r '[.summary.requestsPerSec, .latencyPercentiles.p50, .latencyPercentiles.p99, (.statusCodeDistribution|tostring), ((.errorDistribution|to_entries|map(.value)|add) // 0)] | @tsv' "$OUT/$1.oha.json"; }

cmd=$1; shift
case "$cmd" in
idle)
  tag=$1 secs=$2
  read -r c0 w0 _ _ < <(ru "$KPID"); p0=$(pst "$KPID"); d0=$(dbsz); t0=$(date +%s)
  sleep "$secs"
  read -r c1 w1 _ _ < <(ru "$KPID"); p1=$(pst "$KPID"); d1=$(dbsz); t1=$(date +%s)
  printf 'idle\t%s\twall_s=%s\tps_time=%s->%s\tcpu_s=%.4f\tcpu_per_s=%.5f\tdisk_w=%s\tdb_delta=%s\n' \
    "$tag" $((t1-t0)) "$p0" "$p1" "$(echo "$c1-$c0"|bc -l)" "$(echo "($c1-$c0)/($t1-$t0)"|bc -l)" $((w1-w0)) $((d1-d0)) | tee -a "$OUT/$tag.tsv";;
cpu)
  tag=$1 label=$2 group=$3 body=$4 n=$5 c=$6
  curl -sf -X POST "http://127.0.0.1:$MOCK_PORT/stats/reset"
  read -r c0 w0 _ _ < <(ru "$KPID"); read -r m0 _ _ _ < <(ru "$MPID"); p0=$(pst "$KPID"); d0=$(dbsz); r0=$(rows)
  t0=$(python3 -c 'import time;print(time.time())')
  oha_json "$label" "$group" "$PROXY" "$body" -n "$n" -c "$c"
  sleep 3 # 流式终态行异步写库
  t1=$(python3 -c 'import time;print(time.time())')
  read -r c1 w1 _ _ < <(ru "$KPID"); read -r m1 _ _ _ < <(ru "$MPID"); p1=$(pst "$KPID"); d1=$(dbsz); r1=$(rows)
  ok=$(jq '.statusCodeDistribution."200" // 0' "$OUT/$label.oha.json")
  printf 'cpu\t%s\tgroup=%s\tbody=%s\tn=%s\tc=%s\tok=%s\twall_s=%.2f\tps_time=%s->%s\tkernel_cpu_s=%.4f\tmock_cpu_s=%.4f\tdisk_w=%s\tdb_delta=%s\trows=%s\tmock=%s\toha=%s\n' \
    "$label" "$group" "$(basename "$body")" "$n" "$c" "$ok" "$(echo "$t1-$t0"|bc -l)" "$p0" "$p1" \
    "$(echo "$c1-$c0"|bc -l)" "$(echo "$m1-$m0"|bc -l)" $((w1-w0)) $((d1-d0)) $((r1-r0)) \
    "$(curl -s "http://127.0.0.1:$MOCK_PORT/stats")" "$(summ "$label"|tr '\t' '|')" | tee -a "$OUT/$tag.tsv";;
lat)
  tag=$1 label=$2 url=$3 key=$4 body=$5 n=$6
  oha_json "$label" "$key" "$url" "$body" -n "$n" -c 1
  printf 'lat\t%s\turl=%s\tkey=%s\tbody=%s\tn=%s\t%s\n' "$label" "$url" "$key" "$(basename "$body")" "$n" \
    "$(jq -r '"p50_ms=\(.latencyPercentiles.p50*1000) p90_ms=\(.latencyPercentiles.p90*1000) p99_ms=\(.latencyPercentiles.p99*1000) mean_ms=\(.summary.average*1000) status=\(.statusCodeDistribution|tostring)"' "$OUT/$label.oha.json")" | tee -a "$OUT/$tag.tsv";;
ttfb)
  tag=$1 label=$2 url=$3 key=$4 body=$5 n=$6
  hk=(); [ "$key" != - ] && hk=(-H "x-api-key: $key")
  : > "$OUT/$label.curl.txt"
  for _ in $(seq "$n"); do
    curl -s -o /dev/null -X POST ${hk[@]+"${hk[@]}"} -H 'anthropic-version: 2023-06-01' -H 'content-type: application/json' \
      --data-binary "@$body" -w '%{http_code} %{time_starttransfer} %{time_total}\n' "$url" >> "$OUT/$label.curl.txt"
  done
  printf 'ttfb\t%s\turl=%s\tkey=%s\tbody=%s\tn=%s\t%s\n' "$label" "$url" "$key" "$(basename "$body")" "$n" \
    "$(python3 -c 'import sys
rows=[l.split() for l in open(sys.argv[1]) if l.strip()]
pct=lambda v,p: sorted(v)[min(len(v)-1,round(p/100*(len(v)-1)))]*1000
f=[float(r[1]) for r in rows]; t=[float(r[2]) for r in rows]
print("ttfb_p50_ms=%.2f ttfb_p99_ms=%.2f total_p50_ms=%.2f total_p99_ms=%.2f non200=%d" % (pct(f,50),pct(f,99),pct(t,50),pct(t,99),sum(r[0]!="200" for r in rows)))' "$OUT/$label.curl.txt")" | tee -a "$OUT/$tag.tsv";;
conc)
  tag=$1 label=$2 group=$3 body=$4 c=$5 dur=$6
  read -r c0 _ _ _ < <(ru "$KPID"); read -r m0 _ _ _ < <(ru "$MPID")
  oha_json "$label" "$group" "$PROXY" "$body" -z "$dur" -w -c "$c"
  read -r c1 _ _ _ < <(ru "$KPID"); read -r m1 _ _ _ < <(ru "$MPID")
  printf 'conc\t%s\tgroup=%s\tc=%s\tdur=%s\t%s\tkernel_cpu_s=%.3f\tmock_cpu_s=%.3f\n' "$label" "$group" "$c" "$dur" \
    "$(jq -r '"rps=\(.summary.requestsPerSec) p50_ms=\(.latencyPercentiles.p50*1000) p99_ms=\(.latencyPercentiles.p99*1000) status=\(.statusCodeDistribution|tostring) errors=\(.errorDistribution|tostring)"' "$OUT/$label.oha.json")" \
    "$(echo "$c1-$c0"|bc -l)" "$(echo "$m1-$m0"|bc -l)" | tee -a "$OUT/$tag.tsv";;
*) echo "unknown $cmd"; exit 1;;
esac
