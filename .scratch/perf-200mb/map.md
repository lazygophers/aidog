# 深度性能优化：全进程峰值内存 ≤200MB + 三场景 CPU 下降

Label: `wayfinder:map`

## Destination

一份可交接的**优化 spec**：明确「改哪里 / 改成什么 / 性能与体验的取舍已拍板」，可直接走 `/to-tickets` → `/implement` 落地。

目标数字：aidog **全进程总和**（Rust 主进程 + WKWebView 相关全部子进程）在**持续转发峰值**（约 50 路并发）下 ≤ **200MB**；CPU 在**空闲态 / 转发态 / UI 驻留态**三个场景均下降。

本图产出**决策**，不产出交付物。施工归 spec 之后。

## Notes

**平台**：仅 macOS。WebView = WKWebView。profiling 工具链用 Instruments / `vmmap` / `leaks` / `footprint` / `sample`。Windows、Linux 不在本图内。

**负载模型**：单用户、约 50 路并发流式转发。基准负载**用仓内 mock 协议造**（mock 不在 `platform-presets.json` 内，见 `src/domains/platforms/platformPaste.ts:15`），不打真实上游。

**体验红线（四条全是硬约束，任何方案不得违反）**：
1. 转发延迟与首 token 时延不得变差
2. token 计数与费用精度不得下降 —— **推论：tokenizer 相关方案中「降级为估算 / 纯依赖上游 usage」这一支已被排除**，只允许不损精度的手段（按需加载、用完释放、子进程隔离等）
3. UI 切页与列表流畅度不得下降
4. app 冷启动速度不得变慢 —— **推论：「启动时预热」类手段被排除**

红线之外的空间：非热路径工作可异步化 / 延迟化 / 降频。

**每次会话先读**：`CLAUDE.md`（含 Proxy 日志三级开关、retention、peak_hours 等既有约束）、`.wiki/modules/`、本仓 memory 中已有的性能结论（`streaming-snapshot-meta-only`、`symmetric-body-cap`、`sqlite-retention-vacuum`、`high-freq-path-min-diff`、`dual-db-aggregate-is-memory-shortcut`）。

**已知起点（charting 时的侦察，未经实测，仅为线索）**：
- `assets/tokenizers/glm-4.json` = 19MB、`qwen2.json` = 6.7MB，经 `include_bytes!` 编入二进制（`gateway/proxy/tokenizer.rs:19-20`），由 `Tokenizer::from_bytes` 解析进 `OnceLock`（`tokenizer.rs:71,84`）
- 另有 tiktoken `cl100k_base_singleton()` / `o200k_base_singleton()` 两个常驻单例（`tokenizer.rs:60,65`）
- 四个单例均 lazy，**一旦触发永不释放**
- 前端零 `setInterval`；Rust 仅 1 处后台 sleep 循环（`gateway/backup/scheduler.rs:89`）
- `aidog_core` 约 76k 行 Rust；前端 315 个 ts/tsx 约 51k 行

## Decisions so far

<!-- 一行一个已关闭的票：够判断相关性即可，细节回票里看 -->

（暂无）

## Not yet specified

- **SQLite 侧占用与调参** —— page_cache / mmap_size / 连接池大小 / WAL 体积对常驻内存的贡献。待 [01 量测基线与内存归因] 分解出 DB 占比后才能问准。
- **前端具体改法** —— React 重渲染、大列表虚拟化、bundle 拆分、liquid glass 的 `backdrop-filter` 合成成本。待 [07 UI 驻留态 CPU 归因] 指出热点后才能落成票。
- **异步化边界** —— 哪些工作可以挪出转发热路径（入库、统计聚合、价格同步、日志落盘）。待 [05] 与 [06] 给出热路径构成。
- **是否需要架构级手段** —— 拆进程、换 WebView 策略、代理与 UI 分离。只有在 [03 200MB 可达性裁定] 判为「当前架构不可达」时才会graduate。
- **est_cost / 统计聚合的计算成本** —— 是否在热路径上重复算。待 [05] 火焰图。

## Out of scope

- Windows（WebView2）与 Linux（WebKitGTK）平台 —— 本图只管 macOS，另开一图。
- 上游 API 侧的性能（供应商响应速度、网络链路）—— 不在本 app 控制内。
- 功能删减换性能 —— 四条体验红线已排除。
