#!/bin/bash
# 票 01 的可复现量测工具。macOS only。
#
# 归属难点：WebKit 的 WebContent / GPU / Networking 都是 XPC service，ppid 恒为 1，
# 无法从进程树反查属主。解法：launch 前后各拍一次 WebKit pid 集合，差集即本 app 的。
#
# 用法：
#   ./measure.sh launch              # 关掉旧实例 → 记录 WebKit 基线 → 启动 → 差集落 .pids
#   ./measure.sh mem <label>         # 按 .pids 逐进程取 phys_footprint，出分解表
#   ./measure.sh cpu <label> [secs]  # 采 CPU（默认 20s），出各进程 %cpu
#   ./measure.sh stacks <label> [s]  # 对主进程 + WebContent 抓调用栈（默认 10s）
set -uo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"
APP="${APP:-/Applications/AiDog.app}"
# per-run 隔离（2026-08-01 加固）：多个并发量测方（多 agent/多会话）共享这套设施时，
# 全局单份 .pids/stdout 日志会被互相覆盖，产生"认错进程"的幽灵（本轮实测两个 agent
# 互相踩过 3 次）。用 ISO_HOME 的 basename 做 run id，缺省仍退化为全局单份（向后兼容
# 无 ISO_HOME 的老用法），但设了 ISO_HOME 时天然按 run 隔离文件名。
RUN_ID="${ISO_HOME:+.$(basename "$ISO_HOME")}"
PIDFILE="$DIR/.pids${RUN_ID}"
OUT="$DIR/results"
mkdir -p "$OUT"
STDOUT_LOG="$OUT/iso-app-stdout${RUN_ID}.log"

webkit_pids() { pgrep -f "WebKit.framework.*XPCServices" | sort -n; }

# 身份断言：pid 的 HOME 是否等于期望值。跨 run 串台的根治判据——不猜"当前唯一的
# aidog 就是我的", 直接读它的真实 env 核对。
pid_home() { ps eww -p "$1" 2>/dev/null | tr ' ' '\n' | grep '^HOME=' | cut -d= -f2-; }
assert_owned() {
  local p="$1" want="$2"
  [ -z "$want" ] && return 0   # 未隔离场景不校验
  kill -0 "$p" 2>/dev/null || { echo "Error: pid $p 已不存在（可能被并发操作顶掉）" >&2; return 1; }
  local got; got=$(pid_home "$p")
  [ "$got" = "$want" ] && return 0
  echo "Error: pid $p 的 HOME='$got' ≠ 期望 '$want' —— 这不是本 run 的进程，疑似并发串台" >&2
  return 1
}

# phys_footprint（真实物理占用，非 ps 的 rss —— rss 会漏算压缩内存与 swap）
fp_mb() {
  local mb
  mb=$(footprint -p "$1" 2>/dev/null | awk '/phys_footprint:/ {print $2, $3}')
  [ -z "$mb" ] && { echo "0"; return; }
  # footprint 输出单位可能是 KB / MB / GB
  awk -v v="${mb% *}" -v u="${mb#* }" 'BEGIN{
    if (u ~ /^GB/) printf "%.1f", v*1024;
    else if (u ~ /^KB/) printf "%.1f", v/1024;
    else printf "%.1f", v;
  }'
}

proc_label() {
  case "$(ps -p "$1" -o comm= 2>/dev/null)" in
    *WebContent) echo "WebContent" ;;
    *GPU)        echo "GPU" ;;
    *Networking) echo "Networking" ;;
    *aidog)      echo "aidog(main)" ;;
    *)           echo "?" ;;
  esac
}

case "${1:-}" in
launch)
  pkill -x aidog 2>/dev/null; sleep 3
  # 清场验空（加固4）：pkill 后必须确认真清空了，禁止"顶着别人的残留继续跑"。
  if pgrep -x aidog >/dev/null; then
    echo "Error: pkill 后 aidog 仍存活（pid: $(pgrep -x aidog | tr '\n' ' ')）—— 可能有其他量测方正在用，拒绝继续" >&2
    exit 1
  fi
  before=$(webkit_pids)
  # ISO_HOME 隔离量测（perf-final-verification 起）：`open -a` 走 LaunchServices 拉起，
  # 不可靠传递调用方 shell 导出的 env（实测：设 HOME 后 open -a 起的进程仍读真实 HOME）。
  # 直接执行 .app 内二进制才是普通 fork/exec，正常继承调用方 env，`dirs::home_dir()`
  # 才能读到覆盖值（已用 codex 配置落盘路径实测验证）。
  if [ -n "${ISO_HOME:-}" ]; then
    case "$ISO_HOME" in
      "$HOME"|/tmp) echo "Error: ISO_HOME 未隔离 ($ISO_HOME) — 拒绝执行" >&2; exit 1 ;;
      /tmp/*) ;;
      *) echo "Error: ISO_HOME 不在 /tmp 下 ($ISO_HOME) — 拒绝执行" >&2; exit 1 ;;
    esac
    mkdir -p "$ISO_HOME/.aidog"
    echo "✓ ISO_HOME isolated: ${ISO_HOME} (真实 HOME=${HOME} 不受影响)"
    ( HOME="$ISO_HOME" "$APP/Contents/MacOS/aidog" >"$STDOUT_LOG" 2>&1 & )
  else
    open -a "$APP"
  fi
  for i in $(seq 1 30); do pgrep -x aidog >/dev/null && break; sleep 1; done
  main=$(pgrep -x aidog | head -1)
  [ -z "$main" ] && { echo "aidog 未起来"; exit 1; }
  # 身份断言（加固2）：launch 后立刻核对拿到的 pid 真是本 run 起的，不是并发方的——
  # 当场识破串台，不留到 mem/cpu 采样时才发现"数据是别人的"。
  assert_owned "$main" "${ISO_HOME:-}" || exit 3
  sleep 10   # 等 WebView 全部就位
  after=$(webkit_pids)
  ours=$(comm -13 <(echo "$before") <(echo "$after"))
  { echo "$main"; echo "$ours"; } | grep -v '^$' > "$PIDFILE"
  echo "main=$main  webkit=$(echo "$ours" | grep -cv '^$') 个"
  cat "$PIDFILE" | while read -r p; do echo "  $p $(proc_label "$p")"; done

  # 编制核验：AiDog 恒为 GPU×1 + Networking×1 + WebContent×2（主窗口 + 预建 popover）。
  # 差集口径的固有缺陷：窗口期内其他 WKWebView 宿主（飞书/微信/Safari）新起的 helper
  # 会被误纳（run2 档3 多出 pid 54276 WebContent 106MB，档2/档4 GPU 虚高到 109/95MB）。
  # ppid 恒为 1、launchctl procinfo 需 root，无法归属反查 → 改用编制上限做硬闸。
  nweb=0; ngpu=0; nnet=0
  for p in $ours; do
    case "$(proc_label "$p")" in
      WebContent) nweb=$((nweb+1)) ;;
      GPU)        ngpu=$((ngpu+1)) ;;
      Networking) nnet=$((nnet+1)) ;;
    esac
  done
  if [ "$nweb" -ne 2 ] || [ "$ngpu" -ne 1 ] || [ "$nnet" -ne 1 ]; then
    echo "LAUNCH-FAIL 超编: WebContent=$nweb(期望2) GPU=$ngpu(期望1) Networking=$nnet(期望1) — 有外部 app 的 WebKit helper 混入，本档须重取"
    exit 2
  fi
  echo "编制核验 PASS: WebContent=2 GPU=1 Networking=1"
  ;;

mem)
  label="${2:-unnamed}"
  f="$OUT/mem-$label.txt"
  { echo "# 内存分解 @ $label"
    printf "%-8s %-14s %10s\n" PID PROC FOOTPRINT_MB
    total=0
    while read -r p; do
      kill -0 "$p" 2>/dev/null || { echo "  (pid $p 已退出)"; continue; }
      # 采样前复验（加固3）：HOME 不匹配 = 采到别人的数，当场剔除而非静默计入总数。
      # WebKit XPC helper（GPU/Networking/WebContent）经 xpcproxy 拉起，`ps eww` 读不到
      # 其 HOME（空值，非"不匹配"）——这类进程的归属由 launch 阶段的 before/after 差集
      # + 编制核验兜底，本处只在**读得到 HOME 且确实不同**时才判串台，空值不拦截。
      ph="$(pid_home "$p")"
      if [ -n "${ISO_HOME:-}" ] && [ -n "$ph" ] && [ "$ph" != "$ISO_HOME" ]; then
        echo "  (pid $p HOME 不匹配，疑似串台，跳过)"; continue
      fi
      m=$(fp_mb "$p")
      printf "%-8s %-14s %10s\n" "$p" "$(proc_label "$p")" "$m"
      total=$(awk -v a="$total" -v b="$m" 'BEGIN{printf "%.1f", a+b}')
      # 全量分类明细留档，供事后归因
      footprint -p "$p" > "$OUT/footprint-$label-$p-$(proc_label "$p").txt" 2>&1
    done < "$PIDFILE"
    echo "----"
    printf "%-23s %10s\n" TOTAL "$total"
    echo
    echo "## 各进程 top-5 分类（≥1MB）"
    while read -r p; do
      ff="$OUT/footprint-$label-$p-$(proc_label "$p").txt"
      [ -f "$ff" ] || continue
      echo "### $p $(proc_label "$p")"
      awk '/^ *---/{n++; next} n==1 && NF>=5 {print}' "$ff" \
        | grep -E '^ *[0-9.]+ (MB|GB)' | head -5
    done < "$PIDFILE"
  } | tee "$f"
  ;;

# 定时追踪：看内存是否随时间增长（用户报 1G+，需证实是增长而非静态基线）
track)
  label="${2:-track}"; n="${3:-10}"; gap="${4:-60}"
  f="$OUT/track-$label.txt"
  : > "$f"
  for i in $(seq 1 "$n"); do
    line="t=$((  (i-1)*gap ))s"
    total=0
    while read -r p; do
      kill -0 "$p" 2>/dev/null || continue
      m=$(fp_mb "$p")
      line="$line  $(proc_label "$p")=$m"
      total=$(awk -v a="$total" -v b="$m" 'BEGIN{printf "%.1f", a+b}')
    done < "$PIDFILE"
    echo "$line  TOTAL=$total" | tee -a "$f"
    [ "$i" -lt "$n" ] && sleep "$gap"
  done
  ;;

cpu)
  label="${2:-unnamed}"; secs="${3:-20}"
  f="$OUT/cpu-$label.txt"
  # ps 的 %cpu 是进程生命周期均值，测不出当下负载。
  # 改取累计 CPU 时间（ps -o time）在区间前后的差值 / 墙钟时间 = 区间真实占用。
  cputime_s() {
    ps -p "$1" -o time= 2>/dev/null | tr -d ' ' \
      | awk -F: '{n=NF; s=$n; if(n>1) s+=$(n-1)*60; if(n>2) s+=$(n-2)*3600; print s+0}'
  }
  { echo "# CPU @ ${label} — ${secs}s 区间内 CPU 时间差值 / 墙钟"
    declare -a t0=()
    idx=0
    while read -r p; do
      # 采样前复验（加固3），同 mem 分支：空 HOME（WebKit XPC）不拦截，只拦截确实不同的。
      ph="$(pid_home "$p")"
      if [ -n "${ISO_HOME:-}" ] && [ -n "$ph" ] && [ "$ph" != "$ISO_HOME" ]; then
        echo "Error: pid $p HOME 不匹配，疑似串台，本轮 cpu 采样中止" >&2; exit 1
      fi
      t0[$idx]=$(cputime_s "$p"); idx=$((idx+1))
    done < "$PIDFILE"
    sleep "$secs"
    total=0; idx=0
    printf "%-8s %-14s %8s\n" PID PROC PCT_CPU
    while read -r p; do
      t1=$(cputime_s "$p")
      c=$(awk -v a="${t0[$idx]:-0}" -v b="$t1" -v s="$secs" 'BEGIN{printf "%.1f", (b-a)*100/s}')
      printf "%-8s %-14s %8s\n" "$p" "$(proc_label "$p")" "$c"
      total=$(awk -v a="$total" -v b="$c" 'BEGIN{printf "%.1f", a+b}')
      idx=$((idx+1))
    done < "$PIDFILE"
    echo "----"
    printf "%-23s %8s\n" TOTAL "$total"
  } | tee "$f"
  ;;

stacks)
  label="${2:-unnamed}"; secs="${3:-10}"
  while read -r p; do
    case "$(proc_label "$p")" in
      "aidog(main)"|WebContent|GPU)
        sample "$p" "$secs" -f "$OUT/stacks-$label-$p-$(proc_label "$p").txt" >/dev/null 2>&1 &
        ;;
    esac
  done < "$PIDFILE"
  wait
  echo "栈已落 $OUT/stacks-$label-*.txt"
  ;;

# 压测用 mock 平台/分组种子（幂等）。只对 ISO_HOME/.aidog/platform.db 操作，从不碰真实库。
# 分组名 = token = "mock"（与 loadgen.sh 的 Authorization Bearer mock 对齐）。
seed-mock)
  [ -n "${ISO_HOME:-}" ] || { echo "Error: 需要先设 ISO_HOME（未隔离禁播种）" >&2; exit 1; }
  db="$ISO_HOME/.aidog/platform.db"
  [ -f "$db" ] || { echo "Error: $db 不存在，先 launch 一次生成 schema" >&2; exit 1; }
  n=$(sqlite3 "$db" "SELECT COUNT(*) FROM platform WHERE platform_type='\"mock\"' AND deleted_at=0" 2>/dev/null || echo 0)
  if [ "$n" != "0" ]; then
    echo "✓ mock 平台/分组已存在 (${db})，跳过播种"
  else
    sqlite3 "$db" <<'SQL'
INSERT INTO platform (name, platform_type, extra, enabled, status)
VALUES ('Mock', '"mock"', '{"mock":{"status_code":200,"delay_ms":0,"response_text":"Hello from mock","finish_reason":"end_turn","input_tokens":100,"output_tokens":50,"cache_tokens":0,"error_mode":"none","chunk_count":5}}', 1, 'enabled');
INSERT INTO "group" (name, group_key, routing_mode, auto_from_platform, source_protocol)
VALUES ('mock', 'mock', '"health_aware"', (SELECT id FROM platform WHERE name='Mock'), 'anthropic');
INSERT INTO group_platform (group_id, platform_id, priority, weight, level_priority)
VALUES ((SELECT id FROM "group" WHERE name='mock'), (SELECT id FROM platform WHERE name='Mock'), 1, 1, 5);
SQL
    echo "✓ mock 平台/分组已播种到 $db"
  fi
  ;;

*)
  echo "用法: $0 {launch|seed-mock|mem <label>|cpu <label> [secs]|stacks <label> [secs]}"
  echo "  ISO_HOME=/tmp/aidog-perf-home-XXX \$0 launch     # HOME 隔离启动（禁用 open -a，直接二进制执行）"
  echo "  ISO_HOME=/tmp/aidog-perf-home-XXX \$0 seed-mock   # 幂等播种 mock 平台/分组（仅隔离库）"
  exit 1 ;;
esac
