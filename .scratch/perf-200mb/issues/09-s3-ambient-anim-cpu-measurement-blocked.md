# s3-ambient-anim：CPU 前后对比未产出

Task: `frontend-compositing-purge` / subtask `s3-ambient-anim`

代码改动（删 bgShimmer / pulseGlow 改 opacity / 内联 animation 提类 / reduced-motion
覆盖补全）已完成并提交（`06a91513` + `ee45f11b`），但 s1 协议要求的 CPU 前后对比
数值未能产出。记录如下，供 check 阶段核对，CPU 归因已移交 s7-verify 加总量测。

## ① 两轮采样均落背景 regime，按协议作废

装好含本改动的 release `.app` 后跑了两轮完整 `pkill → launch → activate →
sleep 600 → activate → sleep 30 → 采样`：

| 轮次 | pid | CPU% (30s 区间) | WebContent footprint |
|---|---|---|---|
| 1 | 55942 | 7.2% | 357MB |
| 2 | 13817 | 0.3% | 156MB |

s1 协议门槛（`measure-protocol.md`）：CPU <10% 或 WebContent <350MB → 判定未真正
前台，作废重采。两轮均命中作废条件，均不可用作结论。

## ② 与 fe-s5 共享单实例测量设施导致 pid 互踩

排查发现 `.scratch/perf-200mb/assets/results/` 下同时存在 `s5-before`/`s5-after`
系列文件，pid（55942/13817 等）与本轮采样完全一致——即 fe-s5-glassbuffer 与本
subtask 在同一时段用同一份 `measure.sh` + 同一个 `/Applications/AiDog.app` 单
实例 + 共享 `.pids` 并发采样，双方的 `pkill`/`launch` 互相打断对方稳态窗口，
数据链路被交叉污染。误删共享 `.pids` 后已 `git checkout` 还原为提交态，未产生
遗留污染。

## ③ CPU 归因移交 s7-verify

main（team-lead）裁定：不再为本 subtask 单独补量测（两轮成本 ~11min/轮 且未必
稳定收敛；s7-verify 的「空闲前台 CPU <0.5%」验收项本身是全仓加总量测，bgShimmer
是全仓唯一无条件常驻动画，其 CPU 收益可在 s7 的加总读数里归因，无需 s3 单独
重复验证）。本 subtask 的验收改为结构性证据（内联 animation 计数=0 / pulseGlow
改 opacity / reduced-motion 全覆盖 / bgShimmer 已删），四项均已过。
