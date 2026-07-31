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

- [03 200MB 目标可达性裁定] — **目标维持 200MB 不变**（用户拍板）。新实验推翻了「层数」归因：`mask` 挪进 hover 对 graphics 字节零影响（231→230MB），且 region 数在同 CSS 下 157/211/218 乱跳是噪声；**决定性证据是 `graphics ≈ 7.35e-5 × 面积(px) + 16.7`** —— 合成面是窗口面积的物理函数，代码优化不了。因非合成面 176.7MB 已占预算 88%，必须两头压：非合成面 ≤129MB + 合成面 ≤71MB → **反推出新的产品级约束「必须限制窗口尺寸」（约束成立，但具体数字未定 —— release 复验不支持 dev 拟合式，见下）**。预算表见票内。CPU 目标定 **空闲前台 <0.5%**（参照隐藏窗口 0.2%），要求**消灭**软件光栅化与逐帧 style 重算，非降频。流光边框**改实现保视觉**：`@property` 动画角度 → `transform: rotate` 静态 conic 层。

- [08 SQLite page cache 常驻] — **主进程 44MB(冷启动) vs 150MB(稳态) 的矛盾解除：两个数都对，[01] 测的是曲线冷端。** 根因 = `db/mod.rs:12` `READ_POOL_SIZE=8` × 3 池 = 24 只读 + 3 写 = **27 条连接，全仓无 `cache_size` 设置**，走 SQLite 默认 2MB/连接 ≈ **54MB page cache**。证据：`heap` 显示 5KB 块数 1051(5.6min) → 12436(22min)，单一尺寸档解释全部 58MB 增长，且 12436 ≈ 24×500 pages（误差 3.6%）。log.db 7GB 保证 cache 必被填满，是稳态非偶发。排除泄漏（`leaks` 仅 5.7MB，全是 macOS `CryptKit` TLS 框架泄漏，非本仓）。**性质关键：这 54MB 是可配置项，不是物理成本**，与合成面完全不同。**已落地**（`sqlite-page-cache-residency` task）：只读连接 `PRAGMA cache_size=-64`（KB），实测稳态−冷启动 5MB / heap 5KB 块数 1899（< 2500 阈值），三条查询 p95 相对基线上升均 ≤10%（不压红线 3），小库对照证实对小库场景安全；写连接维持默认不动。数值固化进 `gateway/db/mod.rs`，`AIDOG_SQLITE_READ_CACHE_KB` 保留为 debug 旋钮。

## 图已收敛 → 已转 9 个 skein task（2026-07-28）

雾区已推开，本图不再产出新决策。全部落地工作转 `.skein/task/`，DAG 如下（`skein list` 为真值源）：

| task | deps | subtask | 对应本图哪些 fog |
|---|---|---|---|
| `mock-loadgen-capability` | — | 5 | 压测台前提（下方「mock 能力边界」） |
| `logs-query-ipc-slimming` | — | 7 | 前端具体改法 / 异步化边界（Logs+Stats 侧） |
| `proxy-hotpath-buffers` | mock | 7 | 异步化边界 / est_cost 计算成本 |
| `tokenizer-residency-trim` | — | 6 | 起点线索里的 tokenizer 四单例 |
| `frontend-compositing-purge` | — | 7 | 常驻动画全量清点 + transform-rotate 视觉等价性 |
| `sqlite-page-cache-residency` | logs | 6 | SQLite 侧占用与调参（[08]） |
| `cold-start-unblock` | frontend | 7 | 红线 4 相关 + bundle 拆分 |
| `window-default-size` | frontend | 6 | 窗口尺寸具体数字 + 怎么落地 |
| `perf-final-verification` | 全部 8 个 | 6 | 总收口：200MB 到底达没达 |

### 本轮 grill 补上的四条用户裁决

1. **Logs 分页** — 去掉精确 `COUNT(*)`，改 `LIMIT+1` 探测。UI 退化为「有更多」是**已接受**的可见变化。
2. **前端视觉** — 三项改动**全部执行**：删 `bgShimmer 32s`、`.glass::after` 光晕收进 hover、`.input`(47处)+`.btn`(12处) 去 `backdrop-filter`。
3. **窗口** — 删 `maximized: true` 改默认 `1026×759`，**不加** `maxWidth`/`maxHeight`。达标口径 = 默认尺寸下 ≤200MB。「拉大就超」正面写进 spec。
4. **CPU** — 空闲前台 <0.5% 若清理后仍达不到，**继续深挖直到达标**，禁下调目标值。

### 本轮新发现（补进图，非新 fog）

- `tauri.conf.json` 的 `maximized: true` **让 width/height 形同虚设** —— 应用永远最大化启动，这是合成面被推到最大的直接原因。上面「窗口尺寸具体数字」这条 fog 的一半答案。
- `mock` 能力边界已勘察（`proxy/mock.rs:5-158`，forward 层短路不起 server）。**阻断项**：`mock.rs:96-104` 每请求进 `apply_manual_budgets` 的 platform 写连接（tokio-rusqlite 单后台线程串行），50 路并发下污染全部量测数据。故 `mock-loadgen-capability` 排在最前。
- `delay_ms` 语义重载（`mock.rs:22` 首包 + `:113-118` chunk 间隔共用同值），无独立 TTFT 旋钮；`error_mode` 确定性单值，做不到比例注入。

## Not yet specified

> 下列条目多数已被上表的 task 承接，保留原文供追溯；未被承接的只剩 `log.db 体积治理`。

- **窗口尺寸约束的具体数字** —— 约束**存在性**已定（合成面 ∝ 窗口面积，dev 干净实验证实），但**数字未定**。[03] 补做的 release 复验推翻了用 dev 拟合式反推：release 两点拟合常数项 67.3MB（dev 只有 16.7MB），单常数项就吃掉整个 71MB 合成面预算，1150×750 达不到。且那轮 release 量测本身不干净（同进程内主进程 116→150MB、GPU 28→64MB，缩窗后反升），两点不同稳态，系数不可信。**需要一次干净的 release 长稳态窗口-内存曲线量测**（每尺寸独立重启 + 等满 10min 增长期）才能定数。
- **窗口尺寸约束怎么落地** —— 限死 `tauri.conf.json` 的 `maxWidth/maxHeight`？还是不限制、只承诺默认尺寸下达标并在文档写明大窗超预算？**「用户手动拉大窗口就会超 200MB」是物理事实，代码规避不了**，spec 必须正面写。这是本图剩下唯一的产品级取舍。
- **常驻动画全量清点** —— CPU <0.5% 的目标下**任何常驻动画都不能留**，不止流光边框一处：`body::before` 的 `bgShimmer 32s`（本机因 `reduceMotion=1` 未跑，默认用户会跑）、CSS 内 13 处 `animation:` / tsx 内 9 处。需逐个判「删 / 改 compositor-only / 保留」。等 [07] 补齐逐页数据后成票。
- **transform-rotate 版流光边框的视觉等价性** —— [03] 定了方向但留了风险：当前 1px 边环靠 `mask-composite: exclude` 做，旋转带 mask 的层会破坏与 `border-radius` 的贴合，可能要拆成「外层固定 mask + 内层旋转渐变」。需先做视觉比对再落实现，红线 3 卡着。
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
