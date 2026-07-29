#!/bin/bash
# s2-curve-measure：窗口尺寸-内存曲线，4 档。run3 版。
# 每档独立重启 → 设尺寸 → 推到背景 → 600s 稳态 → 直接采样。约 42min。
#
# run3 相对 run2 的两处修订（2026-07-29 用户拍板）：
# ① 编制核验闸门（在 measure.sh launch 内）——差集口径会把飞书/微信/Safari 在窗口期内
#    新起的 WebKit helper 误纳（run2 档3 多出 106MB WebContent，档2/4 GPU 虚高到 109/95MB）。
#    改为核验 WebContent=2 / GPU=1 / Networking=1，超编该档重取（本脚本重试 ≤3 次）。
# ② 删掉 activate + regime 前台判据，改纯背景态采样——内存量测本就走背景态口径
#    （用户第 8 条拍板：只认背景态可比读数）。run1/run2 各 4 档全废于「AiDog 保不住前台」，
#    activate 是从 CPU 量测继承来的，对内存量测无必要。launch 后主动把前台让给 Finder，
#    全程背景，读数不再受用户正常使用电脑影响。
set -uo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"
OUT="$DIR/results"
mkdir -p "$OUT"
SUMMARY="$OUT/size-curve-raw.txt"
: > "$SUMMARY"

run_one() {
  local W=$1 H=$2 LABEL=$3
  echo "=== [$LABEL] start $(date +%FT%T) ===" | tee -a "$SUMMARY"

  local LAUNCH_TS ATTEMPT=0
  while :; do
    ATTEMPT=$((ATTEMPT+1))
    pkill -x aidog 2>/dev/null; sleep 5
    LAUNCH_TS=$(date +%s)
    if "$DIR/measure.sh" launch >> "$SUMMARY" 2>&1; then break; fi
    if [ "$ATTEMPT" -ge 3 ]; then
      echo "[$LABEL] launch 编制核验连续 3 次超编 → 本档作废" >> "$SUMMARY"
      return
    fi
    echo "[$LABEL] launch 超编，第 $ATTEMPT 次重试" >> "$SUMMARY"
  done

  osascript -e "tell application \"System Events\" to tell process \"AiDog\" to set position of window 1 to {100, 100}" 2>>"$SUMMARY"
  osascript -e "tell application \"System Events\" to tell process \"AiDog\" to set size of window 1 to {$W, $H}" 2>>"$SUMMARY"

  # 推到背景：内存量测口径 = 背景态。让 Finder 抢走前台，此后用户怎么用电脑都不影响。
  osascript -e 'tell application "Finder" to activate' 2>>"$SUMMARY"

  sleep 600
  "$DIR/measure.sh" mem "$LABEL" >> "$SUMMARY" 2>&1

  {
    echo "--- 自证 [$LABEL] ---"
    echo "1) 采样时间戳: $(date +%FT%T)  (launch_ts=$LAUNCH_TS, launch 尝试 $ATTEMPT 次)"
    echo "2) pids: $(tr '\n' ' ' < "$DIR/.pids")  存活: $(tr '\n' ' ' < "$DIR/.pids" | xargs -n1 ps -p 2>/dev/null | grep -c aidog)"
    echo "3) app mtime: $(stat -f %m /Applications/AiDog.app) (须 < launch_ts)"
    echo "4) 采样时前台: $(lsappinfo info -only name "$(lsappinfo front)" 2>/dev/null | sed 's/.*=//; s/"//g')  (背景态口径，非 AiDog 即符合预期)"
  } >> "$SUMMARY"
  echo "=== [$LABEL] done $(date +%FT%T) ===" >> "$SUMMARY"
}

run_one 1026 759  w1026x759
run_one 1150 750  w1150x750
run_one 1800 1100 w1800x1100
run_one 2304 1265 w2304x1265

echo "ALL DONE $(date +%FT%T)" >> "$SUMMARY"
