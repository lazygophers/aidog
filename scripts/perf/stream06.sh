#!/usr/bin/env bash
# 票 06 长流复测：CHUNKS=20 vs 200（DELAY_MS=5），日志全开，perf-conv / perf-pass 各 n=1000 c=20 测 CPU/请求；
# 再以 CHUNKS=200 + 带符号内核跑 profile02.sh sample（c=50，采 30 s）。结果 → results/memory/stream.tsv 等。
set -euo pipefail
H=$(cd "$(dirname "$0")" && pwd); B=$H/../results/bodies
export PERF_ID=mem PROXY_PORT=19894 MGMT_PORT=19895 MOCK_PORT=18904 OUT=$H/../results/memory DELAY_MS=5
fresh() { "$H/stop.sh" >/dev/null 2>&1 || true; rm -rf /tmp/aidog-perf-mem/.aidog; mkdir -p /tmp/aidog-perf-mem
  cp -R /tmp/aidog-perf-mem.tpl /tmp/aidog-perf-mem/.aidog; "$H/start.sh"; sleep 20; }
for ch in 20 200; do
  CHUNKS=$ch fresh
  for g in perf-conv perf-pass; do "$H/bench02.sh" cpu stream "$g-ch$ch" $g "$B/body-280k.json" 1000 20; done
done
"$H/stop.sh"   # 先用默认 KERNEL_BIN 停 release 内核（stop.sh 按 KERNEL_BIN 核对命令行）
CHUNKS=200 KERNEL_BIN=/tmp/aidog-perf-prof-target/release/aidog-kernel fresh
KERNEL_BIN=/tmp/aidog-perf-prof-target/release/aidog-kernel "$H/profile02.sh" sample
mv "$OUT/profile-c50-sample.txt" "$OUT/profile-ch200-c50-sample.txt"; mv "$OUT/profile-c50-sample.oha.json" "$OUT/profile-ch200-c50.oha.json"
KERNEL_BIN=/tmp/aidog-perf-prof-target/release/aidog-kernel "$H/stop.sh"
python3 "$H/sample_analyze.py" "$OUT/profile-ch200-c50-sample.txt" 25 > "$OUT/profile-ch200-c50-analysis.txt"
echo STREAM DONE
