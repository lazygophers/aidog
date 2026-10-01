#!/usr/bin/env bash
# 票 06 内存矩阵：日志全开 / 全关 × c=10/50/100/200（280 KB）+ c=50 1 MB。串行，各 run 冷启动。
H=/Users/luoxin/persons/lyxamour/aidog/.scratch/perf-backend/harness; B=/Users/luoxin/persons/lyxamour/aidog/.scratch/perf-backend/harness/../results/bodies
on()  { PERF_ID=mem    PROXY_PORT=19894 MGMT_PORT=19895 MOCK_PORT=18904 $H/mem06.sh run "$@"; }
off() { PERF_ID=memoff PROXY_PORT=19896 MGMT_PORT=19897 MOCK_PORT=18906 $H/mem06.sh run "$@"; }
for c in 10 50 100 200; do on on-c$c $c $B/body-280k.json; done
on on-1m-c50 50 $B/body-1m.json
for c in 10 50 100 200; do off off-c$c $c $B/body-280k.json; done
off off-1m-c50 50 $B/body-1m.json
echo MATRIX DONE
