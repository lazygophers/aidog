# Rust 主进程空闲 CPU 归零 — 详细设计

## 总体思路

先归因后动手。静态检索能穷举的 Rust 侧空闲定时器只有 60s×1 + 300s×1 + 24h×3，
均摊占比在 0.01% 量级（`research/idle-wakeup-survey.md:111-112`），解释不了实测 1.8%。
S1 用 Instruments Time Profiler 出栈归因，把缺口分解成三类：本项目代码 / 框架底噪 /
第三方 crate 内部线程。S2-S4 按归因结论逐点消除可控项，S5 复测判决。

## S1 — 栈归因（前置，其余全部依赖）

CLI 无 GUI 路径，`xctrace` 随 Xcode CLT 附带：

```bash
xctrace record --template 'Time Profiler' --attach <pid> --time-limit 30s --output idle.trace
xctrace export --input idle.trace --xpath '/trace-toc/run/data/table[@schema="time-sample"]'
```

三进程各录一轮：`aidog(main)` / 主窗口 `WebContent` / `GPU`。pid 取 `.scratch/perf-200mb/assets/.pids`。
产物落 `research/idle-attribution.md`，须对 S2/S3/S4 各给「值得改 / 无效」判定 —— 归因说某点
无效时，对应 subtask 允许以「按归因判定不改 + 记录理由」结项，不强行改码。

## S2 — backup scheduler 事件驱动化

现状 `gateway/backup/scheduler.rs:84-91`：`tick_secs = (interval_hours*3600).clamp(1,60)`，
默认 24h 配置下恒为 60s；且 sleep 在 `maybe_backup` 之前，`enabled=false` 也照唤醒。
60s clamp 的存在理由是「设置即时生效」（`scheduler.rs:79` 注释）。

改法：把「设置即时生效」从轮询换成通知。设置写入路径 emit 一个唤醒信号（Tauri event 或
`tokio::sync::Notify`），循环用 `tokio::select!` 同时等「到点」与「设置变更」，两者任一到达
才醒。`enabled=false` 时不排下次 sleep，纯等 Notify。语义等价，空闲唤醒归零。

## S3 — 托盘 coarse tick 放宽

`src-tauri/src/app_setup.rs:428-450`，macOS 专属，`min(300s, 距本地 00:00)`。唯一职责是跨日
重算（`:417-425` 注释），实时刷新由 `tray-refresh` 事件 + 200ms 防抖负责（`:404-415`）。
300s → 30min，`min(距本地 00:00)` 那一支保留不动，跨日精度不受影响。

## S4 — 隐藏 popover 动画 gate

`app_setup.rs:494-517` `prebuild_popover` 启动即建隐藏窗口，React 已 mount，
`popover.css:77` `statusPulse 2s infinite` 常驻，现有 gate 只有 `prefers-reduced-motion`
（`popover.css:80`）。隐藏窗口仍在跑动画 = 持续合成 / CA 提交。

改法优先级按最小实现阶梯：能用 CSS 解决就不写 JS。窗口隐藏时给根节点挂一个类
（或直接用 `document.visibilityState` 驱动），`animation-play-state: paused`，显示时恢复。
`globals.css:204` spin / `:290` statusPulse 是否一并处理，取决于 S1 归因是否把主窗口合成
列为实际占比来源。

## S5 — 复测

采样 ≥60s 稳态均值，三进程各自 + 总和。若剩余占比经 S1 归因证明全为框架底噪
（WKWebView / tao 事件循环），按「可控项全清零 + 记录实测值与底噪证据」判定通过，
不为不可达阈值反复返工。

## 执行约束

profiling 与 cargo/yarn build 互斥占机（memory `measure-window-exclusive-env`），
S1 与 S5 采样期间禁并发构建。`window-default-size` 量测窗口已结束，本 task 可开跑。
