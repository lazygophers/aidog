#!/usr/bin/env bash
# 只停 pid 文件里、且命令行确认是本 harness 起的进程。绝不 pkill/killall。
set -uo pipefail; source "$(dirname "$0")/env.sh"
for name in kernel mock; do
  f="$ISO/$name.pid"; [ -f "$f" ] || continue
  pid=$(cat "$f")
  cmd=$(ps -p "$pid" -o command= 2>/dev/null || true)
  # 内核额外核对环境变量 HOME=$ISO（防 pid 复用误杀用户的 aidog-kernel）
  if [ "$name" = kernel ] && ! ps eww -o command= -p "$pid" 2>/dev/null | tr ' ' '\n' | grep -qx "HOME=$ISO"; then cmd="(HOME mismatch) $cmd"; fi
  case "$name:$cmd" in
    kernel:"$KERNEL_BIN"*|mock:*mock_upstream.mjs*)
      kill "$pid"
      for _ in $(seq 50); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
      kill -0 "$pid" 2>/dev/null && echo "WARN $name pid $pid still alive" || echo "stopped $name pid $pid";;
    *) echo "skip $name: pid $pid not ours or gone (cmd='$cmd')";;
  esac
  rm -f "$f"
done
