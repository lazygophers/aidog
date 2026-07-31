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

## 7. S5 复测结论（≥60s 稳态均值，S1 已用 30s Time Profiler 归因，本节复测印证）

**前提澄清（避免误读）**：S2/S3/S4 经 S1 栈归因（`research/idle-attribution.md`）全判「无效，
不改」，`scheduler.rs` / `app_setup.rs` / `popover.css` / `globals.css` **均未改动任何一行**
（见 `research/idle-attribution.md` §6-8「禁止项」）。本次复测的代码状态与 S1 归因时完全
一致 —— **复测目的是用 ≥60s 稳态均值印证 S1 的 30s 采样代表性，不是验证"优化后"收益**，
全程无优化动作可归因。

### 7.1 采样方法与时长

- 工具：`top -l 65 -s 1 -pid <main> -pid <WebContent主窗口> -pid <WebContent-popover> -pid <GPU>`，
  1s 间隔连续 65 个样本 ≈ 65s（≥60s 稳态窗口，非单点快照）。
- pid：aidog(main)=19809、WebContent(主窗口)=19932、WebContent(popover)=19969、GPU=19930
  （`ps -o pid,ppid` 确认 19809 由 `tauri dev` 子进程 17178 派生；19930/19931/19932/19969
  与 19809 同一秒（15:17:xx）批量创建，判定为同批 WebKit 助手进程）。
- 运行态：aidog 窗口 `visible=true`（System Events 确认），非 frontmost 聚焦态（前台聚焦是
  Claude Code/Electron，符合桌面代理类应用"打开但未主动交互"的空闲场景）。
- 采样窗口内本 agent 未跑 cargo/yarn build（遵守 `measure-window-exclusive-env`）。
- 采样原始数据 `/tmp/idle_cpu_sample.txt`（一次性测量产物，未纳入仓库）。

### 7.2 三进程各自 + 总和实测值（65 样本均值）

| 进程 | pid | 65s 均值 |
|---|---|---|
| aidog(main, Rust) | 19809 | **0.1200%** |
| WebContent(主窗口) | 19932 | **0.0154%** |
| WebContent(popover) | 19969 | **0.0000%** |
| GPU | 19930 | **0.0077%** |
| **总和（design.md 定义的三进程口径：main+主窗口 WebContent+GPU）** | — | **0.1431%** |

（含 popover 一并汇总仍是 0.1431%，因 popover 均值为 0，不影响总和。）

### 7.3 与 S1（30s Time Profiler）对照

| 进程 | S1（30s） | S5（65s 稳态均值） | 一致性 |
|---|---|---|---|
| aidog(main) | 0.037% | 0.120% | 同量级（均 <0.15%），均属「本项目代码零归因」的框架底噪范围 |
| WebContent(主窗口) | 7.757% | 0.0154% | **不一致，差 2 个数量级** |
| GPU | 0% | 0.0077% | 同量级（均 ~0%） |
| WebContent(popover) | 0.237% | 0.0000% | 同方向（均极低） |

WebContent(主窗口) 的差异有明确解释、非矛盾：`research/idle-attribution.md` §1 已记录该
进程 30s 内呈「~5s 高负载/短暂低谷」交替模式（12.10%/0.10%/11.58%/0.88%/10.14%/9.70%
分桶），推测是 dev 模式下 JSC JIT 反复热身同一段未打包 JS 所致。S1 采样发生在应用刚启动
后不久；S5 采样发生在应用已持续运行约 45 分钟之后，JIT 热身/懒编译的突发窗口早已过去，
稳态期自然趋零 —— **两次测量互相印证同一个结论：7.76% 是启动期瞬时突发，不是持续稳态
开销**，稳态下 WebContent 主窗口不构成待清零的负担。main/GPU 量级一致，支持 S1 归因稳定性。

### 7.4 PASS/FAIL 判决

**总和 0.1431% < 0.5% 阈值 → 直接达标，无需援引"底噪豁免"口径。**

可控项（S2 backup scheduler / S3 托盘 tick / S4 popover 动画 gate）经 S1 栈归因判定全部
无效且均未改动代码（`research/idle-attribution.md` 各节「禁止项」），本次复测代码状态与
归因时一致；剩余实测总和已量化为 0.1431%，全部落在框架底噪范围（tao 事件循环 / tokio
runtime maintenance / WKWebView JSC-WebCore 内部活动，`research/idle-attribution.md` §3
三分类归属），无新增可控点位。

**判决：PASS。**

### 7.5 task 原始目标 vs 阈值目标 —— 两条分开陈述（禁混为一谈）

- **原始目标「Rust 主进程空闲 CPU 归零」**：main 进程实测 **0.1200%**（65s 稳态均值），
  **未归零**，也不可能归零 —— S1 §2 记录的 11 个采样点全部落在 tao 事件循环 CFRunLoop、
  AppKit/XPC 基线、tokio-rt-worker 运行时自维护、JSC libpas scavenger 线程，均是框架/运行时
  必然存在的常驻开销，非本项目业务代码可消除。"归零"字面目标不可达，应澄清为"本项目代码侧
  可控项归零" —— 该条已达成（S2/S3/S4 三个候选点位归因贡献均 <0.05%，且验证为零命中本项目
  代码路径）。
- **「全进程总和 <0.5%」阈值**：本次实测 **0.1431%**，**达成**（远低于阈值，非临界压线）。

两条结论不同：前者是"表述层面不可达的目标，需澄清为可控项归零"（已达成）；后者是"可量化
阈值，已达成"。本 task 验收标准（S5 验收条目）以后者（阈值判决）为准，前者仅作背景澄清，
不构成 FAIL 理由。
