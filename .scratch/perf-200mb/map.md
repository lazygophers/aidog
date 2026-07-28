# 深度性能优化：全进程峰值内存 ≤200MB + 三场景 CPU 下降

Label: `wayfinder:map`

## Destination

一份可交接的**优化 spec**：明确「改哪里 / 改成什么 / 性能与体验的取舍已拍板」，可直接走 `/to-tickets` → `/implement` 落地。

目标数字：aidog **全进程总和**（Rust 主进程 + WKWebView 相关全部子进程）在**持续转发峰值**（约 50 路并发）下 ≤ **200MB**；CPU 在**空闲态 / 转发态 / UI 驻留态**三个场景均下降。

本图产出**决策**，不产出交付物。施工归 spec 之后。

## Notes

**平台**：仅 macOS。WebView = WKWebView。profiling 工具链用 Instruments / `vmmap` / `leaks` / `footprint` / `sample`。Windows、Linux 不在本图内。

**负载模型**：单用户、约 50 路并发流式转发。基准负载**用仓内 mock 协议造**（mock 不在 `platform-presets.json` 内，见 `src/domains/platforms/platformPaste.ts:15`），不打真实上游。

**硬约束（用户明确指示）**：一切测试与压测**只允许用 mock 协议的平台与分组**，禁止拿真实平台做测试。允许为适配测试场景**扩充 mock 能力**（如可控的 token 数 / 延迟 / 并发行为），但扩充 mock 后**必须同步完善对应的前端展示**，不允许后端能力前端看不到。

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

- [01 量测基线与内存归因] — 实测全进程峰值 **619.8MB**（非 1G+），其中 **68% 是 WebContent/GPU 的合成面（graphics region / IOSurface）**；Rust 主进程仅 44–51MB，50 路并发只 +7MB。CPU 空闲前台 50.2%，窗口隐藏后 0.2%；根因 = `globals.css:828-870` 的 `@property --flow-ang` 逐帧动画驱动 conic-gradient + mask，GPU 54% 采样落在 `DrawConicGradient`，WebContent 每帧全量 `Document::resolveStyle`。手法落 `assets/measure.sh` + `assets/loadgen.sh`（只打 mock 分组）。

- [02 WKWebView 常驻内存下限] — WebView 实例 = 2（main + 预建 popover，`app_setup.rs:487-514`），托盘走原生 NSStatusItem 不算。**不可压地板 ≈149MB**（Rust 44 + WebKit malloc 76 + GPU malloc 22 + Networking 6.7），离 200MB 还剩 ~51MB 合成面预算，当前用了 423MB。Tauri 层无 WebView 内存调优面、macOS 无 suspend API、关窗也不还内存（[tauri #5397]）。**合并 popover 只省 22–39MB，不值 —— 排除**。结论：200MB **不是架构不可达，是 Liquid Glass 实现方式不可达**。

## Not yet specified

- **合成层 / IOSurface 数量治理** —— [01] 指认这是内存的**支配性杠杆**（423/619MB），不是 Rust 侧。需要弄清：227 个 graphics region 分别对应哪些元素（`backdrop-filter` / `will-change` / `transform` / `opacity` 动画 / `mask` 都会强制提层），砍掉多少层能省多少 MB，砍层是否踩「UI 流畅度不得下降」红线。这张票在 [02] 回来后与它一起决定 200MB 是否可达。
- **liquid glass 的性能重构口径** —— 根因已定位到 `@property` 动画 + conic-gradient + mask 三件套。但「怎么改」是决策不是实现：删流光边框 / 换静态渐变 / 只在 hover 时用非注册属性 / 限制 `.glass-surface` 施用面 —— 各自的视觉代价不同，需要用户对「视觉 vs 性能」拍板。等 [07] 补齐场景 C 后成票。
- **log.db 体积治理** —— 实测 `~/.aidog/log.db` 7084.9MB + WAL 4446.8MB，checkpoint 实质未跑。不占进程内存，故不影响 200MB 目标，但属独立严重问题（磁盘 + 查询延迟 + 可能拖慢 UI 列表）。是否纳入本图待定；本仓 memory `sqlite-retention-vacuum` 已有相关结论。
- **SQLite 侧占用与调参** —— page_cache / mmap_size / 连接池大小 / WAL 体积对常驻内存的贡献。待 [01 量测基线与内存归因] 分解出 DB 占比后才能问准。
- **前端具体改法** —— React 重渲染、大列表虚拟化、bundle 拆分、liquid glass 的 `backdrop-filter` 合成成本。待 [07 UI 驻留态 CPU 归因] 指出热点后才能落成票。
- **异步化边界** —— 哪些工作可以挪出转发热路径（入库、统计聚合、价格同步、日志落盘）。待 [05] 与 [06] 给出热路径构成。
- ~~**是否需要架构级手段**~~ —— **已由 [02] 关闭**：拆进程 / 合并 popover / 换 WebView 策略经调研与实测均无收益或不可行（Tauri 无调优面、关窗不还内存、popover 只占 22–39MB）。[03] 不必再走这条线。
- **est_cost / 统计聚合的计算成本** —— 是否在热路径上重复算。待 [05] 火焰图。

## Out of scope

- Windows（WebView2）与 Linux（WebKitGTK）平台 —— 本图只管 macOS，另开一图。
- 上游 API 侧的性能（供应商响应速度、网络链路）—— 不在本 app 控制内。
- 功能删减换性能 —— 四条体验红线已排除。
