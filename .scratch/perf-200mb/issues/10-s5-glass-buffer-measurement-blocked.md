# s5-glass-buffer：malloc 前后对比未产出

Task: `frontend-compositing-purge` / subtask `s5-glass-buffer`

代码改动（`.glass::after`/`.glass-surface::after` 的 `mask`/`mask-composite` 从
空闲态基础规则挪进 `:hover::after` 规则，避免 WebKit 为非 hover 态元素也建离屏
合成 buffer）已完成并提交（`06a91513`），但 s1 协议要求的 WebKit malloc 前后
对比数值未能产出。记录如下，供 check 阶段核对，malloc 归因已移交 s7-verify 加总
量测。

## ① 六轮采样，两端均未落进前台 regime，按协议全部作废

装好含本改动的 release `.app` 后跑了 1 轮 before + 6 轮 after（after / retry /
retry2 / v2 / v3 / after-clean），每轮均走 `pkill → launch → activate →
sleep 600 → activate → sleep 30 → 采样` 全流程：

| 采样 | 时间戳 | pid(WebContent) | footprint | WebKit malloc | CPU |
|---|---|---|---|---|---|
| before | 00:31:07 | 55986 | 362MB | 72MB | 4.0% |
| after-clean | 01:04:08 | 49106 | 111MB | — | 0.0% |

`/Applications/AiDog.app` mtime = 00:35:31，落在两次采样之间，构建时序无误。

s1 协议门槛（`measure-protocol.md`）：CPU <10% 或 WebContent <350MB → 判定未真正
前台，作废重采。before 卡在门槛边缘（362MB 略高于线但 CPU 仅 4%），after-clean
（111MB / 0%）是深度背景态。两端均不满足「CPU≥40% 或 WebContent≥400MB」的稳健
前台判据，603.8MB→204.6MB 的 TOTAL 降幅主要来自 WebContent graphics 随窗口失焦
释放（before 单进程 graphics 269MB），**不能归因给本 subtask 的 CSS 改动**。

## ② 根因：屏幕锁导致会话内无法保证前台态

`lsappinfo front` 查证 frontmost app 为 `"loginwindow"` —— 屏幕处于锁定状态，
这是环境级 confound，且与 `measure-protocol.md`「已知环境限制」段一致：本机无
Screen Recording 权限，`System Events` 的 frontmost 查询不可靠，会话内无法
100% 程序化验证/保证前台态。`osascript activate` 在锁屏下不生效，6 轮重试
（含 2 次完整重启）均未能突破这堵墙，第 7 轮预期同样无效，故停手。

## ③ malloc 归因移交 s7-verify

main（team-lead）裁定：逐 subtask 单独量测在当前环境（缺 Screen Recording
权限、屏幕锁定不可控）下做不到，「WebContent WebKit malloc ≤32MB」验收项移交
s7-verify，在能保证前台态的条件下做一次全仓加总量测归因。本 subtask 验收改为
结构性证据：代码改动已落地（mask/mask-composite 仅存在于 `:hover::after`
规则）、hover 态视觉无退化（opacity/animation/背景渐变原样保留，仅挪动了
mask 属性所在的选择器）。
