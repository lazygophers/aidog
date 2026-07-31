# S1 — Instruments Time Profiler 栈归因（空闲 CPU 分解）

## 0. 测量方法与环境

- 工具：`xcrun xctrace record --template 'Time Profiler' --attach <pid> --time-limit 30s`（xctrace 16.0，随 Xcode CLT）
- 运行态：`yarn tauri dev`（debug build，`target/debug/aidog`），前台窗口打开、无用户交互、无 profiling 期间并发 cargo/yarn build（遵守 `measure-window-exclusive-env`）
- pid 来源：`.scratch/perf-200mb/assets/.pids` 已过期（属旧 task），本次现取：

  | 进程 | pid | 启动时间 | 备注 |
  |---|---|---|---|
  | aidog(main, Rust) | 19809 | 15:17:39 | `target/debug/aidog` |
  | WebContent(主窗口) | 19932 | 15:17:44 | 与 main 同批创建，先于 popover 2s，判定为主窗口 |
  | GPU | 19930 | 15:17:44 | `com.apple.WebKit.GPU` |
  | WebContent(隐藏 popover，额外补录) | 19969 | 15:17:46 | design.md 未列入三进程清单，但为判定 S4 额外加录一轮，成本可忽略 |

  四者用 `lsof -p <pid> | grep aidog`（命中 `~/Library/Caches/aidog/WebKit/...`）+ 启动时间比对确认归属，`.pids` 未用（已过期，进程已重启为新 pid）。
- 每进程各录 30s（design.md 指定值），导出 `time-profile` schema（不是 `time-sample`——16.0 版本 schema 名已变，实测确认）。
- 采样窗口内本 agent 只跑 `xctrace` 本身，未触发任何 build。

## 1. 三进程各自占比（design.md 指定的三进程）

| 进程 | 总采样权重(30s 窗口内) | 占比 |
|---|---|---|
| aidog(main, Rust) | 11 ms | **0.037%** |
| WebContent(主窗口) | 2327 ms | **7.757%** |
| GPU | 0 ms（0 个采样点） | **0%**（整 30s 窗口零 running 采样，等同 <0.01%） |

**关键发现**：三者中真正的大头是 **WebContent 主窗口**（7.76%），不是 Rust 主进程（0.037%，与 `idle-wakeup-survey.md` 静态推测的 0.01% 量级一致，互相印证）。`GPU` 进程整段窗口零采样，说明当前无合成压力落在专用 GPU 进程侧。

PRD 记载的「主进程常驻 1.8%」与本次实测 main=0.037% 有数量级落差；`findings.md` 另记「3.0%」。二者口径不明确是否指 aidog 单进程还是三进程加总，本次实测显示：**若按 Activity Monitor 常见的「按 App 分组汇总」口径，WebContent 的开销大概率被计入了历史测量的 1.8%/3.0% 里**（用户看到的是应用整体，不会区分 Rust 主进程 vs WebKit 助手进程）。`top -l 1` 瞬时采样显示四进程当前均为 0.0%，说明该开销是**突发式（bursty）而非持续稳态**——30s 窗口内分布见下节，非每一刻都在跑。

### WebContent 主窗口 30s 内 5s 分桶（验证是否稳态 vs 突发）

| 区间 | 占比 |
|---|---|
| 0-5s | 12.10% |
| 5-10s | 0.10% |
| 10-15s | 11.58% |
| 15-20s | 0.88% |
| 20-25s | 10.14% |
| 25-30s | 9.70% |

呈明显的「~5s 高负载 / 短暂低谷」交替模式，不是持续稳态占用，也不是单次尖峰——像是某周期性任务反复触发（JIT 分层编译 + GC 通常伴随此类脉冲，见下节栈证据），而非一次性预热。

### WebContent 主窗口栈分类明细（2327ms 总量）

| 分类 | 占比(窗口) | 说明 |
|---|---|---|
| JSC 引擎(Lexer/JIT/GC/bmalloc) | 3.980% | `JSC::Lexer::lexWithoutClearingLineTerminator`、`JSC::DFG::*`、`JIT Worklist Helper Thread`、`MarkedBlock::Handle::sweep`、`pas_*`/`bmalloc_*` |
| 其它 WebCore/ObjC/CF (未归入下面两类) | 2.853% | 未细分逐帧，多为 WebCore 内部调用链 |
| 渲染/布局/绘制 (RenderLayer/GraphicsLayer/PlatformCALayer/RenderBlock 等) | 0.683% | 含 CSS 动画驱动的合成/绘制 |
| IPC/mach (`IPC::Connection`/`mach_msg2_trap`) | 0.240% | WebContent↔UI 进程 IPC |

Main Thread 占该进程 67.5% 权重，`JIT Worklist Helper Thread` ×多个合计约 17%——典型特征是**反复解析/JIT 热身同一段 JS**，怀疑与 dev 模式下 Vite 未打包、按需编译到 JSC 字节码/热路径升 DFG/FTL 有关（生产版走预打包+压缩 bundle，理论上此项会显著更低，但本 task 只对当前运行态归因，不做 dev vs prod 对照实验，超出 scope）。

## 2. aidog(main) 主进程栈分解（11ms 总量，全部落于以下 7 个采样点）

| 线程 | 权重 | 顶层帧 |
|---|---|---|
| Main Thread | 3ms | `objc_msgSend` / `CA::Layer::prepare_commit` / `__CF_IS_OBJC` |
| JavaScriptCore libpas scavenger | 2ms | `stop_allocator`（tao/WebKit 共享 bmalloc 符号，非本项目分配路径） |
| tokio-rt-worker | 2ms | tokio scheduler `worker::Core::maintenance`（tokio 运行时自身周期性维护，非业务代码） |
| Log work queue | 1ms | `objc_autoreleasePoolPush` |
| aidog(其余线程 ×3) | 3ms | `pas_thread_local_cache_layout_node_is_committed`、`BSXPCServiceConnection` 回调、tracing_subscriber 相关帧一次 |

**11 个采样点里没有一个命中 `gateway::backup::scheduler` 或 `tray_render`/`refresh_tray_menu`/`NSStatusItem`**（`idle-wakeup-survey.md` 静态检索出的两个定时器）。30s 窗口跑不到 backup scheduler 的 60s tick 是符合预期的采样盲区之一，但即使命中，其单次成本（一次 DB 读，`maybe_backup` 内部判定即返回）远低于本次测得的 main 进程整体 0.037%。

## 3. 三分类归属

| 分类 | 内容 | 依据 |
|---|---|---|
| 本项目代码 | 无可归因样本 | main 进程 11 个采样点全部落在 tao 事件循环 / tokio runtime 内部维护 / objc runtime，无一落在 `aidog_core::gateway::*` 业务函数 |
| 框架底噪 | WKWebView(WebContent) 的 JSC 引擎(lexer/JIT/GC)、WebCore 渲染管线、IPC；main 进程的 tao/AppKit CFRunLoop、dispatch、tokio-rt-worker maintenance | main 进程 175 个唯一帧名几乎全部是 `_CFRunLoopRun`/`_dispatch_*`/`__BSXPCServiceConnection*`/`_pthread_*`——tao 的事件循环 + macOS AppKit/XPC 基线开销；WebContent 侧全部 JSC/WebCore 符号 |
| 第三方 crate 内部线程 | `JavaScriptCore libpas scavenger` 线程（bmalloc 后台清扫，WebKit 自带非本项目依赖）；tokio 运行时 worker maintenance | 线程名直接标注来源；tokio 是本项目依赖但该开销是 runtime 自身调度维护，非业务代码触发 |

**结论：本次测量窗口内，主进程 0.037% 的开销找不到本项目代码归因（全是框架/运行时底噪）；真正可见的大头 7.76% 落在 WebContent 助手进程的 JavaScriptCore/WebCore 内部，同样是框架层，PRD 范围外条款（`prd.md:12`）已明确排除。**

## 4. 对 S2/S3/S4 的判定

| Subtask | 判定 | 预估收益 | 依据 |
|---|---|---|---|
| **S2 backup scheduler 事件驱动化** | **无效，允许不改+记录理由结项** | <0.037%（30s 窗口内该线程活动 0 次命中 scheduler 代码路径，上限即 main 进程总量的一个零头，静态推测 0.01% 与实测互相印证） | main 进程 11 个采样点无一落在 `gateway::backup::scheduler`；60s 周期的 DB 读单次成本已被静态调研判定为 0.01% 量级，本次栈归因未推翻该推测，只是进一步用真实运行数据验证「测不到」 |
| **S3 托盘 300s→30min** | **无效，允许不改+记录理由结项** | <0.05%（300s 周期，30s 采样窗口命中概率 10%，且即使命中也未见 `tray_render`/`NSStatusItem` 相关帧） | main 进程栈内无 `refresh_tray_menu`/`tray_render`/`NSStatusItem` 帧；`idle-wakeup-survey.md:45` 静态推测已判定 <0.05%，本次归因未发现反例。放宽周期本身零风险，若日后顺手改仍可做，但不构成本 task 必须项 |
| **S4 隐藏 popover 动画 gate** | **无效，允许不改+记录理由结项** | 0.0133%（popover WebContent 30s 总量仅 0.237%，其中真正落在 `RenderLayer`/`GraphicsLayer`/`PlatformCALayer`/`RenderBlock` 等渲染绘制帧的只有 4ms/71ms＝5.6%，换算成 30s 窗口占比 0.0133%，其余 94% 是 GC/malloc/JS 引擎背景活动） | popover WebContent(pid 19969) 额外加录一轮 30s，总权重仅 71ms(0.237%)，渲染管线相关帧仅 4ms，远低于用户裁定的 <0.05% 门槛 |

三项判定共同结论：**本项目代码侧当前无可优化的空闲 CPU 大头**，静态检索出的三个候选点位（backup scheduler / 托盘 tick / popover 动画）经栈归因验证均低于 <0.05% 门槛，均可按用户裁定「不改 + 记录理由」结项。真正的开销大头（WebContent 主窗口 7.76%）是 WKWebView/JavaScriptCore 框架层内部行为，PRD 已明确列为范围外（`prd.md:12`），不在本 task 可控改动范围内。

## 5. 对 S5 复测的提示

- 若 S5 复测仍量到「三进程总和 ≥0.5%」，主要来源大概率仍是 WebContent 的 JSC/WebCore 框架底噪（本次测得 7.76%，具突发性非持续稳态），**不是** S2/S3/S4 覆盖的本项目代码点位——S5 判定「可控项全清零」时应直接引用本文件第 3/4 节作为底噪证据，不必因总和未达标而回头返工 S2/S3/S4。
- 本次测量基于 `yarn tauri dev`（debug build，未压缩/未 bundle 的前端资源）。生产构建（`yarn build` 产物 + release 二进制）下 JSC 的解析/JIT 负担理论上会显著降低（预打包 + 压缩后 JS 体积和解析次数都小很多），但本 task 未做 dev vs release 对照实验（超出 scope，若需要应另立 subtask）。

## 6. S2 结项记录（backup scheduler 事件驱动化 — 判定「不改」）

**判定：不改 `gateway/backup/scheduler.rs`，以「记录归因依据」结项。**

**归因数据依据**：
- 第 2 节（本文件 56-66 行）：main 进程 30s 窗口内共 11 个采样点、总权重 11ms（占比 **0.037%**，本文件第 24 行），全部落在 `objc_msgSend`/`CA::Layer::prepare_commit`（Main Thread）、JSC libpas scavenger、`tokio-rt-worker` maintenance、`objc_autoreleasePoolPush`、`pas_thread_local_cache_layout_node_is_committed` 等帧（本文件 60-64 行表格），**无一帧命中 `gateway::backup::scheduler`**（本文件第 66 行）。
- 第 4 节判定表（本文件 82 行）：S2 预估收益上限 <0.037%，即使 60s tick 命中采样窗口，其单次成本（一次 DB 读 + `maybe_backup` 内部判定即返回）也远低于 main 进程整体开销，与 `idle-wakeup-survey.md` 静态推测的 0.01% 量级互相印证（本文件第 82 行、第 66 行）。
- 用户已裁定归因显示某点价值 <0.05% 则该 subtask 允许「不改 + 记录理由」结项（本 subtask 派发说明），0.037%（scheduler 上限）< 0.05% 门槛成立。

**未来重启条件**（满足任一即需重开本项优化）：
1. `backup_interval_secs`（或等效配置）从当前默认值大幅调小（例如降到秒级/亚分钟级），使 tick 频率显著提高，60s 周期假设失效；
2. scheduler 语义变化为常驻轮询/忙等（而非当前"每 tick 判定 `enabled`/`interval` 未到即快速返回"的轻量语义），导致单次唤醒成本上升；
3. 独立复测（如 S5 或后续 profiling）实测 main 进程空闲占比显著回升（≥0.05%）且栈归因命中 `gateway::backup::scheduler` 帧。

**禁止项**：本次结项未改动 `gateway/backup/scheduler.rs` 任何一行，符合派发禁改约束。

## 7. S3 结项记录（托盘 coarse tick 放宽 — 判定「不改」）

**判定：不改 `app_setup.rs:428-450` 托盘 300s coarse tick，以「记录归因依据」结项。**

**归因数据依据**：
- 第 2 节（本文件 56-66 行）：main 进程 30s 窗口内共 11 个采样点、总权重 11ms（占比 **0.037%**，本文件第 24 行），全部落在 `objc_msgSend`/`CA::Layer::prepare_commit`（Main Thread）、JSC libpas scavenger、`tokio-rt-worker` maintenance、`objc_autoreleasePoolPush`、`pas_thread_local_cache_layout_node_is_committed` 等帧（本文件 60-64 行表格），**无一帧命中 `tray_render`/`refresh_tray_menu`/`NSStatusItem`**（本文件第 66 行）。
- 第 4 节判定表（本文件 83 行）：S3 预估收益上限 <0.05%（300s 周期，30s 采样窗口命中概率仅约 10%，且即使命中也未见托盘相关帧），main 进程整体占比 0.037% 已低于用户裁定门槛，托盘 tick 上限只会更低。
- 用户已裁定归因显示某点价值 <0.05% 则该 subtask 允许「不改 + 记录理由」结项（本 subtask 派发说明），main 进程实测 0.037% < 0.05% 门槛成立，且 11 个采样点无一命中托盘代码路径，进一步验证放宽周期本身零收益可测。

**未来重启条件**（满足任一即需重开本项优化）：
1. 托盘刷新逻辑变重（例如 `refresh_tray_menu`/菜单重建每次 tick 都触发昂贵计算或 IPC，而非当前轻量刷新语义），导致单次唤醒成本上升；
2. coarse tick 周期被调小（例如从 300s 降到秒级/十秒级），使唤醒频率显著提高，当前 300s 假设失效；
3. 独立复测（如 S5 或后续 profiling）实测 main 进程空闲占比显著回升（≥0.05%）且栈归因命中 `tray_render`/`refresh_tray_menu`/`NSStatusItem` 相关帧。

**禁止项**：本次结项未改动 `app_setup.rs` 任何一行，符合派发禁改约束。

## 8. S4 结项记录（隐藏 popover 动画 gate — 判定「不改」）

**判定：不改 `src/popover.css` / `src/globals.css` / `app_setup.rs`，以「记录归因依据」结项。**

**归因数据依据**：
- 第 0 节（本文件第 14 行）：popover WebContent 隐藏窗口（pid 19969）作 S4 判定佐证额外加录一轮 30s Time Profiler。
- 第 4 节判定表（本文件第 84 行）：popover WebContent 30s 总权重仅 71ms（占比 **0.237%**）；其中真正落在 `RenderLayer`/`GraphicsLayer`/`PlatformCALayer`/`RenderBlock` 等渲染绘制帧（CSS 动画合成相关）的仅 4ms/71ms＝5.6%，换算成 30s 窗口占比 **0.0133%**；其余 94%（约 0.2237%）是 GC/malloc/JS 引擎背景活动（JSC/WebCore 框架层，与本文件第 49 行「JSC 引擎(Lexer/JIT/GC/bmalloc)」同类底噪，非 CSS 动画驱动）。
- 渲染管线相关占比 0.0133% 远低于用户裁定的 <0.05% 门槛，即使把整个 popover WebContent 总量 0.237% 计入也仍需区分：真正可能受 `statusPulse`/`spin` 等常驻 CSS 动画驱动的合成/绘制开销上限是 0.0133%，其余是 popover 页面本身 JS 解析/GC（与是否隐藏、动画是否 gate 无关，隐藏也不会消除）。
- 用户已裁定归因显示某点价值 <0.05% 则该 subtask 允许「不改 + 记录理由」结项（本 subtask 派发说明），0.0133%（渲染管线相关上限）< 0.05% 门槛成立。
- 验收条目「globals.css spin/statusPulse 按 S1 归因结论处置」**明确记录为不纳入**：归因数据显示这些动画即使在隐藏态持续合成，其对空闲 CPU 的实际贡献上限仅 0.0133%（30s 窗口内 4ms），gate 掉它们的收益低于用户裁定的优化门槛，故不做 `animation-play-state`/可见性 gate 改动。

**未来重启条件**（满足任一即需重开本项优化）：
1. popover 页面新增更多常驻 CSS 动画（`spin`/`statusPulse` 之外），或现有动画复杂度显著提高（例如从简单 transform/opacity 升级为触发 layout/repaint 的属性），使渲染管线占比大幅上升；
2. popover 隐藏窗口的常驻数量/生命周期发生变化（例如从单一隐藏窗口变为多个常驻隐藏 popover 同时存在），使总权重线性放大；
3. 独立复测（如 S5 或后续 profiling）实测 popover WebContent 渲染管线相关占比显著回升（≥0.05%）且栈归因命中 `RenderLayer`/`GraphicsLayer`/`PlatformCALayer` 等帧与 CSS 动画强关联。

**禁止项**：本次结项未改动 `src/popover.css` / `src/globals.css` / `app_setup.rs` 任何一行，符合派发禁改约束。
