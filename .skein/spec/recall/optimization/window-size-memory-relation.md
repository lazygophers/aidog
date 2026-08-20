---
title: window-size-memory-relation
layer: recall
created: 1785561018
category: optimization
keywords: [memory,window-size,area,fitting,release,webkit,footprint]
status: active
inclusion: auto
anchors: .skein/task/window-default-size/curve-result.md,.scratch/perf-200mb/window-size-measure-protocol.md
---

## 窗口面积与内存无可信拟合关系（release 口径实测）

## 实测数据（2026-07-29，release 构建，纯背景态口径，4 档）

| 档 | 面积(px²) | TOTAL(MB) | graphics(MB) |
|---|---|---|---|
| 1026×759（默认） | 778,734 | **378.7** | 43.7 |
| 1150×750 | 862,500 | 308.7 | 70.9 |
| 1800×1100 | 1,980,000 | 319.6 | 101.0 |
| 2304×1265 | 2,914,560 | 283.8 | 66.9 |

默认档 1026×759 下 TOTAL **378.7MB** 分解：graphics 43.7 / main 143.0 / GPU 23.0 / Net 7.7 / WebContent×2 167.0+38.0（唯一逐进程留档的档，来源 `.skein/task/window-default-size/curve-result.md`）。

## 结论：无可信拟合式，禁外推

**TOTAL 与面积呈负相关**——面积涨 3.7 倍（0.78M → 2.91M px²），TOTAL 反从 378.7 降到 283.8。档间噪声幅度 ±95MB **远超**任何可能的面积效应，是「信号被噪声完全淹没」，不是「效应小」。

`graphics` 分类前三档单调递增（43.7 → 70.9 → 101.0），第四档却掉回 66.9——即使最该线性的指标也不干净。

因此：**`6.34e-5×面积+67.3`（早期 dev 口径 [03]）与 `7.35e-5×面积+16.7`（另一 dev 口径）均不可外推到 release 口径**，禁在后续任务里拿这两个式子做预测或决策依据。

## 正解：窗口面积不是内存杠杆，别在这个维度找优化空间

内存大头在堆而非窗口合成面：档 1 分解显示 `aidog(main)` MALLOC_SMALL 115MB + MALLOC_LARGE 11MB、WebContent#1 WebKit malloc 102MB，graphics 合计仅占 TOTAL 11.5%。**200MB 目标必须靠堆侧任务去啃**（tokenizer-residency-trim / sqlite-page-cache-residency / proxy-hotpath-buffers），不是靠改窗口默认尺寸。

## 用户拉大窗口可能超 200MB：物理成本，非缺陷

WKWebView 合成面随窗口尺寸增大而分配更大绘制缓冲/纹理是 WebKit 的正常行为，用户主动拉大窗口后内存上涨（即使不呈线性关系）是**合成层的物理成本**，不应被当作 bug 排查或优化目标。

## 反例（错误模式）

| 错误做法 | 正解 |
|---|---|
| 拿 dev 口径拟合式预测 release 内存 | 两口径不互通，release 需独立实测且当前无可信拟合式 |
| 看到窗口变大内存变大就当 bug 报 | 先判断是否是 WKWebView 合成面物理成本，非缺陷 |
| 试图靠调窗口默认尺寸去够 200MB 目标 | 窗口尺寸救不了 200MB，改攻堆侧任务 |

## 关联

[[memory-measure-background]] 采样口径 / 附：完整量测协议见 `.scratch/perf-200mb/window-size-measure-protocol.md`（编制核验硬闸、纯背景态口径、`--bundles app` 三条教训对后续量测任务仍适用）

[[perf-200mb-final-verification]]：`perf-final-verification` s2 最大化对照组（release 口径、`2304×1265`、独立重启+背景态+≥600s 稳态，WebContent graphics 单项 147MB）把本文档「合成面是窗口面积函数」这条定性结论从拟合外推坐实为实测证据（不改变「无可信拟合式」这条否定性结论，仅证据强度升级）。
