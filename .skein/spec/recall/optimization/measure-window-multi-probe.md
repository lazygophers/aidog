---
title: measure-window-multi-probe
layer: recall
category: optimization
keywords: [量测,采样,cpu,前台,探针,regime,steady-state,foreground]
status: active
---

## 量测 regime 自证必须窗口内多点探针

## 判据
CPU/内存稳态采样，只在采样前打一次前台确证（如 `lsappinfo front`）不够——采样窗口内应用可能中途失焦（切到终端/其他窗口），单点起始确证无法发现窗口末端已漂移，导致读数混入前台/背景混合值。

## 陷阱
实测：run3 采样前确证前台，但 60s 窗口末端已漂回终端，读数被稀释成 8.2%（前台+背景混合值）。同实例钉死前台复采（每 15s 一次探针，t=0/15/30/45/60 全确证）测得 3.0%，差 2.7 倍。

## 正解
稳态采样窗口内必须**多点探针**（如每 15s 一次），全程确证前台/目标态未漂移，而非仅窗口前一次性确证。另需注意 activate 后的 settle 时间——30s 不足以排干 activate 尖峰，应取协议上限（如 90s）再开始采样。

## 案例
`.scratch/perf-200mb/assets/results/cpu-s7-after-run3.txt`（8.2% 误读）vs `cpu-s7-after-run3b.txt`（3.0% 修正后，多点探针）。

## 适用
CPU/内存稳态性能采样，尤其涉及应用前台/背景态切换、GUI 应用量测场景。

## 关联
[[measure-window-exclusive-env]]（跨项目 memory：同族约束——采样期间禁并发 build 争抢资源；本条补充窗口内探针密度维度）
