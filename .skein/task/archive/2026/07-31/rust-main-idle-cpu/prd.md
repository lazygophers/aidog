# Rust 主进程空闲 CPU 归零 — PRD (主入口)

## 目标
- [ ] 空闲前台态 aidog 全进程 (main + WebContent + GPU) CPU 总和压到 <0.5%，现状实测主进程常驻 1.8%
- [ ] 先用 Instruments Time Profiler 出栈归因，把 1.8% 分解到具体调用栈，替代静态检索的推测
- [ ] 消除本项目代码侧全部可控空闲唤醒源：backup scheduler 60s 空转轮询、托盘 300s coarse tick、预建 popover 隐藏窗口的常驻 CSS 动画
- [ ] 用户价值：常驻后台应用不再无谓耗电，笔记本续航与风扇噪音可感改善
## 边界
- 范围内：Rust 侧定时器 (gateway/backup/scheduler.rs、src-tauri/src/app_setup.rs 托盘 tick)
- 范围内：预建 popover 隐藏窗口的常驻动画 gate (popover.css statusPulse / globals.css spin·statusPulse 视归因结果)
- 范围内：xctrace Time Profiler 三进程栈归因，产物落 research/
- 范围外：Tauri/WKWebView 框架底噪与第三方 crate (tao/wry/tray-icon/rustls/objc2) 内部线程 — 不可控，只归因不改
- 范围外：主窗口可见态的动画与合成优化 (归 已完成的 frontend-compositing-purge，本 task 只碰隐藏 popover)
- 范围外：内存/启动耗时/bundle 体积 (归 cold-start-unblock 等其它 perf task)
- 约束：backup scheduler 改事件驱动须保留「设置变更即时生效」语义 (scheduler.rs:79 注释)
- 约束：跑 profiling 与 cargo/yarn build 互斥占机，采样期间禁并发构建 (memory measure-window-exclusive-env)
- 约束：托盘 tick 唯一职责是跨日重算，改周期不得导致跨 00:00 统计不刷新
## 验收标准
- [x] xctrace Time Profiler 已对 aidog(main)/WebContent/GPU 三进程各录一轮 30s，归因报告落 research/ 并给出 1.8% 的调用栈分解
- [x] backup scheduler：空闲期(enabled=false 或未到 interval)不再每 60s 唤醒读 DB 且设置变更仍即时生效；或 S1 归因判定其占比 <0.05% 无优化价值并记录依据后不改
- [x] 托盘 coarse tick 周期已从 300s 放宽且跨本地 00:00 统计仍正确刷新；或 S1 归因判定无价值并记录依据后不改
- [x] 预建 popover 隐藏时不再持续跑 CSS 动画、显示时正常恢复；或 S1 归因判定无价值并记录依据后不改
- [x] 复测：空闲前台态 aidog 三进程 CPU 总和 <0.5% (采样 ≥60s 取稳态均值)；若归因证明剩余占比全为框架底噪，则记录实测值 + 底噪证据，本条按「可控项全清零」判定
- [ ] 有代码改动时：cargo clippy 零 warning、cargo test 通过、yarn build 通过
## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [findings.md](findings.md) (仅真调研时生)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list rust-main-idle-cpu`)
