#!/usr/bin/env bash
# 起假上游 + 隔离内核。UI=0 → 纯内核（无 /rpc，seed.sh 不可用）。
set -euo pipefail; source "$(dirname "$0")/env.sh"
mkdir -p "$ISO/.aidog"
# 首次启动：DB 无代理设置时内核读此文件迁移入库（shared.rs load_proxy_settings）
[ -f "$ISO/.aidog/aidog.db" ] || echo "{\"port\":$PROXY_PORT,\"autostart\":true,\"bind_lan\":false}" > "$ISO/.aidog/proxy_settings.json"
alive() { [ -f "$1" ] && kill -0 "$(cat "$1")" 2>/dev/null; }
if ! alive "$ISO/mock.pid"; then
  MOCK_PORT=$MOCK_PORT nohup node "$HARNESS_DIR/mock_upstream.mjs" > "$ISO/mock.log" 2>&1 &
  echo $! > "$ISO/mock.pid"
fi
if ! alive "$ISO/kernel.pid"; then
  args=(); [ "${UI:-1}" = 1 ] && args=(--ui --port "$MGMT_PORT")
  # exec：让 $! 就是内核本身的 pid（不 exec 时 $! 是包装子 shell，stop.sh 会认不出）
  (cd "$ISO" && HOME="$ISO" exec nohup "$KERNEL_BIN" ${args[@]+"${args[@]}"} > "$ISO/kernel.log" 2>&1) &
  echo $! > "$ISO/kernel.pid"
fi
wait_port "$MOCK_PORT" 10 || { echo "mock not up, see $ISO/mock.log"; exit 1; }
wait_port "$PROXY_PORT" 30 || { echo "proxy not up on $PROXY_PORT, see $ISO/kernel.log"; exit 1; }
echo "up: ISO=$ISO kernel pid $(cat "$ISO/kernel.pid") proxy :$PROXY_PORT mgmt :$MGMT_PORT mock pid $(cat "$ISO/mock.pid") :$MOCK_PORT"
