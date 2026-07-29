# findings — rust-main-idle-cpu

调研收敛结论。过程证据见 `research/idle-wakeup-survey.md`（6 类唤醒源全量静态勘察）。
本文件由 main 代写（subagent 被 hook 拦，禁写 report 类文件，只放行 `research/` 下产物）。

## 1. 核心结论：归因缺口是第一阻塞项

静态检索能穷举的 Rust 侧空闲定时器**总共只有 60s×1 + 300s×1 + 24h×3**。

推测: 这个频率解释不了实测 3.0% 的稳态 CPU —— 60s 一次轻量 DB 读的均摊占比在 0.01% 量级
（`research/idle-wakeup-survey.md:111-112`）。缺口只可能在三处，静态检索无法区分：

1. 主窗口 + 预建 popover 窗口的常驻 CSS `infinite` 动画 → 持续合成 / CA 提交
2. Tauri / WKWebView host 侧 IPC + 事件循环基线开销（框架底噪，与本项目代码无关）
3. 某第三方 crate 内部线程（`tao`/`wry`/`tray-icon` 事件循环、`rustls`、`objc2` autorelease pool）

**→ 栈归因（`sample <pid> 10` 或 Instruments Time Profiler）是本 task 的前置 subtask。**
不做归因，后续优化方向全是猜的。

## 2. 静态可见的优化点（按档位排序）

| 档 | 点位 | 现状 | 优化 | 推测省 |
|---|---|---|---|---|
| Top 2 | `gateway/backup/scheduler.rs:84-91` | `tick_secs=(interval_hours*3600).clamp(1,60)` → 默认 24h 配置下恒 60s；sleep 在判定之前，`enabled=false` 也照唤醒；每轮一次 `BackupSettings::load` DB 读 | 事件驱动化（设置变更 emit 唤醒，替代轮询） | ~0.01% |
| Top 3 | `src-tauri/src/app_setup.rs:428-450` | macOS 托盘 `min(300s, 距本地00:00)`，唯一职责是跨日重算（`:417-425` 注释） | 300s → 30min | 可忽略但白捡 |
| — | `app_setup.rs:494-517` `prebuild_popover` | 启动即建隐藏 popover 窗口，React 已 mount，`popover.css:77` `statusPulse 2s infinite` 常驻，gate 仅 `prefers-reduced-motion`（`popover.css:80`） | 隐藏时 `animation-play-state: paused` / 可见性 gate | 视归因结果 |

60s clamp 的存在理由是「设置即时生效」（`scheduler.rs:79` 注释）—— 改事件驱动须保留该语义。

## 3. 已否定的类（查过，本项目没有）

- **文件系统 watcher**：无 `notify` crate 依赖，唯一 `notify::` 匹配是项目内 `gateway/proxy/notify.rs`（Anthropic notify 端点），非 crate。
- **前端定时 invoke 轮询**：`setInterval` 全项目 **0 处**；~60 处 `setTimeout` 全一次性（无递归自调用）；4 处 rAF 均非常驻。
- **网络主动心跳**：axum accept 阻塞 kqueue 零 CPU；reqwest 未配 `tcp_keepalive`/`http2_keep_alive_interval`/`pool_idle_timeout`，空闲池为空。健康端点 `/` `/proxy` `/models` 全被动。
- **SQLite 后台**：WAL checkpoint 走默认 `wal_autocheckpoint=1000 页`（写触发非定时），空闲无写；retention+VACUUM 24h 一轮。
- **托盘逐帧重绘**：无 <60s 刷新定时器，空闲期重绘 = 每 5min 1 次。

## 4. 与 frontend-compositing-purge 的对账（scope 收窄依据）

fcp 已 finish，其 8 个 subtask 覆盖了 rmic Top 1 的大部分点位：

| Top 1 点位 | fcp 覆盖 |
|---|---|
| `globals.css:186` shimmer / `:955` progressStripes | ✅ 「skeleton 与进度条动画按可见性挂载」 |
| `globals.css:215` pulseGlow | ✅ 「常驻动画清除与 reduced-motion 补全」 |
| `globals.css:909` flowBorder | ⚠️ 归 `glass-flow-border-component`；已证「@property 动画仅 `:hover` 挂载，空闲态零开销」，**非空闲 CPU 成因** |
| `globals.css:204` spin / `:290` statusPulse | ❌ 未覆盖 |
| `popover.css:77` + `app_setup.rs:494` 预建 popover 窗口内常驻动画 | ❌ 未覆盖（跨层新点，fcp 只扫主窗口） |

**→ 本 task scope 收窄为三项**：

- **S1（前置）栈归因** — 定位 3.0% 的真实去向，区分上述三种可能
- **S2** Rust 侧 Top2 backup scheduler 事件驱动化 + Top3 托盘 300s→30min
- **S3** 预建 popover 窗口的常驻开销（fcp 盲区，唯一保留的前端点位）

`globals.css:204` spin / `:290` statusPulse 视 S1 归因结果决定是否纳入。

## 5. 执行约束

- S2/S3 改 Rust 触发 cargo 重编 → **必须等 `window-default-size` 量测窗口结束后执行**
  （memory `measure-window-exclusive-env`：采样期间禁并发 cargo/yarn build）。
- S1 栈归因同样占机，须排在量测窗口之后。

## 6. 已裁定：栈归因走 Instruments Time Profiler

用户 2026-07-29 拍板：**跑 Instruments Time Profiler**（非 `sample`）。理由是要带时间轴 +
调用树权重，比 `sample` 的扁平栈更能区分「持续合成」与「周期性尖峰」。

执行方式走 CLI 无 GUI 路径（`xctrace` 随 Xcode CLT 附带）：

```bash
xctrace record --template 'Time Profiler' --attach <pid> --time-limit 30s --output idle.trace
xctrace export --input idle.trace --xpath '/trace-toc/run/data/table[@schema="time-sample"]'
```

对三个进程各录一轮：`aidog(main)` / 主窗口 `WebContent` / `GPU`。pid 取
`.scratch/perf-200mb/assets/.pids`。**须等 `window-default-size` 量测窗口结束后跑**（占机）。
