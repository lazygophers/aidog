# 01 量测基线与内存归因

Type: task
Status: open
Blocked by: —
Parent: [深度性能优化：全进程峰值内存 ≤200MB + 三场景 CPU 下降](../map.md)

## Question

当前 1G+ 内存到底花在哪、CPU 到底谁在烧？在没有这份分解之前，任何优化方案都是猜。

产出一份**可复现的量测手法 + 基线数据**：

**内存分解**（`vmmap` / `footprint` / Instruments Allocations）
- 按进程拆：aidog 主进程（Rust）、WKWebView 的 `com.apple.WebKit.WebContent`、`Networking`、`GPU` 各 helper
- 主进程内再拆：tokenizer 单例、SQLite（page cache / mmap）、reqwest 连接池与 rustls session、in-flight 请求缓冲、其余
- 三个采样点：① 冷启动刚可用 ② 空闲驻留 10 分钟 ③ mock 协议 50 路并发流式转发持续 5 分钟的峰值

**CPU 分解**（`sample` / Instruments Time Profiler）
- 场景 A 空闲态（无请求，窗口最小化 / 前台各采一次）
- 场景 B 转发态（mock 50 并发）
- 场景 C UI 驻留态（Logs / Stats / Platforms 各停留 1 分钟）

**手法要能复跑**：脚本或步骤清单落到 `.scratch/perf-200mb/assets/`，后续每张票验证效果都用同一套。

## 验收

- 三个采样点的全进程 RSS 分解表（每项 ≥1MB 的都列出，标 file:line 或进程名）
- 三个 CPU 场景各一份火焰图或 top-N 栈，标出占比 >2% 的栈
- 量测脚本可重复执行，两次运行结果偏差 <10%
