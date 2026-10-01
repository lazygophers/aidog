#!/usr/bin/env bash
# 票 02 分阶段编排（调用 bench02.sh）。用法: bench02_phase.sh <tag> latconc|cpu
# 环境变量同 env.sh（PERF_ID / *_PORT）。
set -euo pipefail; source "$(dirname "$0")/env.sh"
tag=$1 phase=$2
H=$HARNESS_DIR; B=$BODIES; O=$H/../results/baseline; K=$(cat "$ISO/kernel.pid")
P=http://127.0.0.1:$PROXY_PORT/v1/messages; M=http://127.0.0.1:$MOCK_PORT/v1/messages
case $phase in
cpu)
  "$H/bench02.sh" idle "$tag" 60
  for g in perf-conv perf-pass; do
    "$H/bench02.sh" cpu "$tag" "$g-280k-$tag" $g "$B/body-280k.json" 2000 20
    "$H/bench02.sh" cpu "$tag" "$g-1m-$tag" $g "$B/body-1m.json" 500 20
  done
  "$H/bench02.sh" idle "$tag" 60;;
latconc)
  /usr/bin/footprint -p "$K" > "$O/footprint-idle-$tag.txt" 2>&1
  "$H/bench02.sh" lat "$tag" "lat-direct-280k-$tag" "$M" - "$B/body-280k-nostream.json" 300
  for g in conv pass; do "$H/bench02.sh" lat "$tag" "lat-$g-280k-$tag" "$P" perf-$g "$B/body-280k-nostream.json" 300; done
  "$H/bench02.sh" lat "$tag" "lat-direct-1m-$tag" "$M" - "$B/body-1m-nostream.json" 100
  for g in conv pass; do "$H/bench02.sh" lat "$tag" "lat-$g-1m-$tag" "$P" perf-$g "$B/body-1m-nostream.json" 100; done
  "$H/bench02.sh" ttfb "$tag" "ttfb-direct-280k-$tag" "$M" - "$B/body-280k.json" 100
  for g in conv pass; do "$H/bench02.sh" ttfb "$tag" "ttfb-$g-280k-$tag" "$P" perf-$g "$B/body-280k.json" 100; done
  for c in 1 10 50 200; do
    if [ $c = 50 ]; then ( sleep 12; /usr/bin/footprint -p "$K" > "$O/footprint-c50-$tag.txt" 2>&1; python3 "$H/rusage.py" "$K" > "$O/rusage-c50-$tag.txt" ) & fi
    "$H/bench02.sh" conc "$tag" "conc-c$c-$tag" perf-conv "$B/body-280k.json" $c 25s
    wait
  done
  /usr/bin/footprint -p "$K" > "$O/footprint-after-$tag.txt" 2>&1;;
esac
