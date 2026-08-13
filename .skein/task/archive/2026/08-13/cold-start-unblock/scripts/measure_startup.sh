#!/bin/bash
# 冷启动计时采样脚本（只读量测，不改源码）
# 信号: 直接执行 release 二进制 -> 轮询 AppleScript 查询该进程窗口数首次 >0 的时刻
# 用「窗口创建」作为「首屏可交互」的替代信号（见 baseline.md 协议节说明偏差）
set -u
APP_BIN="/Users/luoxin/persons/lyxamour/aidog/src-tauri/target/release/bundle/macos/AiDog.app/Contents/MacOS/aidog"
PROC_NAME="aidog"
TRIALS="${1:-5}"

kill_app() {
  pkill -x "$PROC_NAME" 2>/dev/null
  sleep 1
}

poll_window() {
  local pid="$1"
  local t0="$2"
  while true; do
    count=$(osascript -e "tell application \"System Events\" to count windows of process \"$PROC_NAME\"" 2>/dev/null)
    if [ -n "$count" ] && [ "$count" -gt 0 ] 2>/dev/null; then
      t1=$(date +%s.%N)
      echo "$t1"
      return 0
    fi
    # 超时保护：10s 未见窗口则放弃本次
    now=$(date +%s.%N)
    elapsed=$(echo "$now - $t0" | bc)
    if (( $(echo "$elapsed > 15" | bc -l) )); then
      echo "TIMEOUT"
      return 1
    fi
    sleep 0.02
  done
}

kill_app

for i in $(seq 1 "$TRIALS"); do
  t0=$(date +%s.%N)
  "$APP_BIN" &
  pid=$!
  t1=$(poll_window "$pid" "$t0")
  if [ "$t1" = "TIMEOUT" ]; then
    echo "trial $i: TIMEOUT"
  else
    delta=$(echo "$t1 - $t0" | bc)
    echo "trial $i: $delta s (t0=$t0 t1=$t1 pid=$pid)"
  fi
  sleep 1
  kill_app
  sleep 1
done
