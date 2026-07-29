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
APP="/Applications/AiDog.app"
PIDFILE="$DIR/.pids"
OUT="$DIR/results"
mkdir -p "$OUT"

webkit_pids() { pgrep -f "WebKit.framework.*XPCServices" | sort -n; }

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
  pkill -x aidog 2>/dev/null && sleep 3
  before=$(webkit_pids)
  open -a "$APP"
  for i in $(seq 1 30); do pgrep -x aidog >/dev/null && break; sleep 1; done
  main=$(pgrep -x aidog | head -1)
  [ -z "$main" ] && { echo "aidog 未起来"; exit 1; }
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
    while read -r p; do t0[$idx]=$(cputime_s "$p"); idx=$((idx+1)); done < "$PIDFILE"
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

*)
  echo "用法: $0 {launch|mem <label>|cpu <label> [secs]|stacks <label> [secs]}"; exit 1 ;;
esac
