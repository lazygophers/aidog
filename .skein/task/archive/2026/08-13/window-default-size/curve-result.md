# 窗口尺寸-内存曲线（run3，release 构建，纯背景态口径）

采样 2026-07-29 16:20–17:01，4 档全部通过自证四项（编制核验 PASS、launch 各 1 次成功、
app mtime 早于 launch、pid 全存活）。协议见 `.scratch/perf-200mb/window-size-measure-protocol.md`。

## 曲线表

| 档 | 面积(px²) | TOTAL(MB) | graphics(MB) | main | GPU | Net | WebContent×2 |
|---|---|---|---|---|---|---|---|
| 1026×759 | 778,734 | **378.7** | 43.7 | 143.0 | 23.0 | 7.7 | 167.0 + 38.0 |
| 1150×750 | 862,500 | **308.7** | 70.9 | — | — | — | — |
| 1800×1100 | 1,980,000 | **319.6** | 101.0 | — | — | — | — |
| 2304×1265 | 2,914,560 | **283.8** | 66.9 | — | — | — | — |

原始盘：`.scratch/perf-200mb/assets/results/size-curve-raw.txt`。

## 结论 1：窗口面积不是内存杠杆（核心假设被证伪）

**TOTAL 与面积呈负相关** —— 面积涨 3.7 倍（0.78M → 2.91M px²），TOTAL 反而从 378.7 降到 283.8。
档间噪声幅度 ±95MB **远超**任何可能的面积效应。这不是「效应小」，是「信号被噪声完全淹没」。

`graphics` 分类倒是前三档单调递增（43.7 → 70.9 → 101.0），第四档掉回 66.9 —— 即使这条最该
线性的指标也不干净。**无法给出可信的 release 口径拟合式**，[03] 的 `6.34e-5×面积+67.3` 与
dev 的 `7.35e-5×面积+16.7` 都不该被当作可外推的模型使用。

## 结论 2：1026×759 下 **378.7MB**，离 200MB 目标差 179MB

窗口默认尺寸这条路**救不了 200MB**。

## 结论 3：内存大头不在窗口面积，在堆

档 1 分解（唯一逐进程留档的档）：

- `aidog(main)` MALLOC_SMALL **115MB** + MALLOC_LARGE 11MB
- WebContent#1 WebKit malloc **102MB** + graphics 40MB
- WebContent#2（预建 popover）WebKit malloc **26MB**

graphics 合计只有 43.7MB（占 TOTAL 11.5%）。**200MB 目标必须靠堆侧去啃** ——
即 `tokenizer-residency-trim` / `sqlite-page-cache-residency` / `proxy-hotpath-buffers`
这三个 task，以及尚未立项的「main 进程 115MB MALLOC_SMALL 归因」。

## 结论 4：预建 popover 窗口净成本 ≥26MB

`app_setup.rs:494-517` 的 `prebuild_popover` 常驻一个 WebContent，档 1 实测 38.0MB
（WebKit malloc 26MB）。与 `rust-main-idle-cpu` 的 S3 subtask（预建 popover 常驻开销）
是同一个点位，内存维度的证据在此。

## 对本 task 的影响

「删 `maximized: true` 回默认 1026×759」这个改动**不再有性能依据**，只剩 UX 依据
（首次启动占满全屏不合理）。s3–s6 是否继续，见 task 决策记录。

## 决策记录（2026-07-29 用户拍板）

**task 归档，s3–s6 全部砍掉。** 性能依据已被 run3 实测证伪，`maximized: true` 保持现状不动。
省下的机时转投堆侧三个 task，下一个开工 `tokenizer-residency-trim`（main 进程
MALLOC_SMALL 115MB 的头号嫌疑）。

本 task 的保留价值 = 上方四条结论 + `.scratch/perf-200mb/window-size-measure-protocol.md`
的量测协议（编制核验硬闸、纯背景态口径、`--bundles app` 三条教训对后续 task 仍适用）。
