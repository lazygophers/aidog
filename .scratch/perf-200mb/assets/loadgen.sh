#!/bin/bash
# mock 协议压测负载生成器。**只打 mock 分组，绝不碰真实平台。**
#
# 用法: ./loadgen.sh [并发数] [持续秒数]
#   默认 50 路并发、300 秒（= 票 01 采样点 ③ 的口径）
#
# 每路是一个循环：持续发流式请求，靠 body 顶层 mock 对象控制
# chunk_count / delay_ms 造出真实的长流（见 gateway/adapter/mock/config.rs:83-114）。
set -u

N="${1:-50}"
SECS="${2:-300}"
BASE="http://127.0.0.1:9890/proxy"
TOKEN="mock"   # 分组名即 token（Authorization Bearer <group_name>）

# 单次请求约 200 chunk × 50ms ≈ 10s 一条流
BODY='{"model":"claude-sonnet-4-20250514","max_tokens":1024,"stream":true,
"messages":[{"role":"user","content":"load test"}],
"mock":{"chunk_count":200,"delay_ms":50,"input_tokens":4000,"output_tokens":2000,
"response_text":"lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua"}}'

# 先确认目标确实是 mock 分组，避免误打真实平台
probe=$(curl -s -m 5 -o /dev/null -w '%{http_code}' "$BASE" 2>/dev/null)
[ "$probe" != "200" ] && { echo "代理未响应 ($BASE 返回 $probe)，先启动 app"; exit 1; }

# 注意：全角括号会被 bash 吃进变量名，变量一律用 ${} 包裹
echo "起 ${N} 路并发，持续 ${SECS}s，目标 ${BASE}（分组 ${TOKEN}）"
end=$((SECONDS + SECS))
pids=()
for i in $(seq 1 "$N"); do
  (
    while [ $SECONDS -lt $end ]; do
      curl -s -N -m 60 -o /dev/null \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -H "anthropic-version: 2023-06-01" \
        -d "$BODY" "$BASE/v1/messages"
    done
  ) &
  pids+=($!)
done

trap 'kill "${pids[@]}" 2>/dev/null' INT TERM
wait
echo "压测结束"
