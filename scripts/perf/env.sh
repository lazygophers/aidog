# 公共变量；其它脚本 source 本文件。全部可用环境变量覆盖。
HARNESS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HARNESS_DIR/../../.." && pwd)"
PERF_ID="${PERF_ID:-default}"
ISO="/tmp/aidog-perf-$PERF_ID"          # 隔离 HOME；数据在 $ISO/.aidog
PROXY_PORT="${PROXY_PORT:-19890}"       # 代理端口（禁 9890）
MGMT_PORT="${MGMT_PORT:-19891}"         # 管理面 /rpc 端口（--ui）
MOCK_PORT="${MOCK_PORT:-18901}"         # 假上游端口
KERNEL_BIN="${KERNEL_BIN:-$REPO/src-tauri/target/release/aidog-kernel}"
BODIES="${BODIES:-$HARNESS_DIR/results/bodies}"
if [ "$PROXY_PORT" = 9890 ] || [ "$MGMT_PORT" = 9890 ] || [ "$MOCK_PORT" = 9890 ]; then
  echo "refuse: port 9890 belongs to the user's live AiDog" >&2; exit 1
fi
wait_port() { # wait_port <port> <timeout_s>
  local i=0; while ! nc -z 127.0.0.1 "$1" 2>/dev/null; do
    i=$((i+1)); [ $i -gt $(( $2 * 10 )) ] && return 1; sleep 0.1; done
}
