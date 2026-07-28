# 06 空闲态 CPU 不为 0 的根因

Type: task
Status: open
Blocked by: 01
Parent: [深度性能优化：全进程峰值内存 ≤200MB + 三场景 CPU 下降](../map.md)

## Question

没有任何请求时，CPU 为什么还在烧？

侦察阶段已排除的：前端零 `setInterval`；Rust 侧只有一处后台 sleep 循环（`gateway/backup/scheduler.rs:89`）。所以根因大概率不在显式轮询上。

需要逐一证实或排除的候选：
- **托盘渲染**（`src-tauri/crates/aidog_core/src/tray_render.rs`）—— 是否周期性重绘、重绘频率与单次成本
- **liquid glass 主题的持续合成** —— `backdrop-filter` / `transform` 在 WKWebView 里是否导致 GPU 进程持续工作，即使画面静止
- **CSS 动画 / transition** —— 是否有未停止的 keyframes 或 `will-change` 导致图层常驻合成
- **Tauri 事件通道** —— 前后端 event 的空转
- **WebView 自身的 timer / rAF** —— React 或第三方库遗留的帧循环
- **backup scheduler 的 tick 频率**（`scheduler.rs:89`）与单次 tick 成本
- **窗口最小化 / 隐藏时是否停止** —— 前台与后台两种状态分别采样

采样要区分**主进程 CPU** 与 **WebContent / GPU helper CPU**，因为两者根因和改法完全不同。

## 验收

- 空闲态（前台、最小化两种）各一份 CPU 采样，主进程与各 helper 分开计
- 上述候选逐条给出「证实 / 排除」结论，证实的标 file:line 与实测占比
- 若根因在 GPU 合成侧，给出最小复现（哪个主题 / 哪个页面 / 关掉什么就归零）
