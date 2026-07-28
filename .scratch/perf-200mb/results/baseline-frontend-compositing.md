# 前端合成层性能基线（改前）

Task: `frontend-compositing-purge` / subtask `s1-measure-protocol`
构建类型: **release**（`/Applications/AiDog.app` 安装版，非 dev）
量测方法: 见 `../measure-protocol.md`

## 有效样本判定

两次独立重启+等满10分钟稳态后立即采样，第二次读数（CPU 7.6%、WebContent
footprint 295~446MB）落入"背景态数量级"，判定窗口未真正前台激活，作废。
经 `activate` + 30~90s settle 重采后得到与 run1 同数量级的 run2-focused-settled，
两者均落在"前台态数量级"（CPU ≥40%、WebContent footprint ≥400MB），采纳为
有效同规则（regime）样本对。run3（`activate` 后仍判定为背景态，CPU 0.0%，
总 phys_footprint 436.7MB）排除，不计入基线。

## 基线四项数值

采集时间：2026-07-28

| # | 指标 | run1 | run2-focused-settled | 偏差 | 判定 |
|---|---|---|---|---|---|
| ① | WebContent WebKit malloc | 86 MB | 91 MB | 5.6% | ✓ <10% |
| ② | 空闲前台 CPU（全进程合计） | 58.3% | 54.2% | 7.3% | ✓ <10% |
| ③ | 全进程 phys_footprint 总和 | 721.6 MB | 723.8 MB | 0.3% | ✓ <10% |
| ④ | GPU helper graphics | 71 MB | 58 MB | 20.2% | ✗ 超阈值，见下 |

**基线取值（供 s2-s6/s7 对照）**：
- WebKit malloc: **~86-91MB**（取中点 ~88MB）
- 空闲前台 CPU: **~54-58%**（s7 验收目标 <0.5%，当前基线严重超标，是本任务的核心待清除项）
- 全进程 phys_footprint: **~722MB**
- GPU graphics: **~58-71MB**（区间值，见下方说明，不作为单点精确基线）

## ④ GPU graphics 20.2% 偏差说明（未完全消除的噪声）

已用「同规则样本对」控制了窗口焦点这个最大混淆变量后，GPU graphics 仍有
20.2% 偏差，超过验收 10% 阈值。已知此前票03 也记录过 graphics 类指标的
天然波动（区别于 WebKit malloc/CPU 的强稳定性）。本次未能进一步降噪的原因：
GPU 合成缓冲区大小与"上次合成以来的脏区域历史"相关，非纯函数于当前窗口
状态，短时间窗口内多次采样间会有残留差异。

**处理方式**：不作为假精确的单点数值使用，后续 subtask（尤其 s2 flow-border
修复、s5 `.glass::after` 缓冲收敛）验证「变好/变坏」时，用同一份协议、同规则
样本对，看区间整体是否下移，而非用单点数值做减法对比。

## 交叉核验（heap，诊断用，非验收指标）

`heap <pid>` 独立信源与 footprint 的 "WebKit malloc" 分类一致：

| 进程 | heap `All zones` | footprint `WebKit malloc`（同轮，run1） |
|---|---|---|
| WebContent（主窗口） | 87.5 MB | 86 MB |
| WebContent2（popover 窗口） | 27.7 MB | — |

两者差 <2%，验证 footprint 的 WebKit malloc 分类行本身可信，heap 交叉验证通过。

## 进程拓扑（本次实测）

1 主进程（aidog） + 1 GPU XPC + 1 Networking XPC + 2 WebContent XPC
（主窗口 WebContent + popover 窗口 WebContent2）。
