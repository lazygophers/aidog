# 01 量测基线与内存归因

Type: task
Status: resolved
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

## Answer

### 量测手法（可复跑）

`assets/measure.sh`（macOS only）与 `assets/loadgen.sh`。

**归属难点已解**：WebKit 的 WebContent / GPU / Networking 都是 XPC service，`ppid` 恒为 1，进程树反查不到属主。解法 = launch 前后各拍一次 `pgrep -f "WebKit.framework.*XPCServices"` 的 pid 集合，差集即本 app 的（`measure.sh:20,48-56`）。

**内存口径必须用 `footprint -p`，不能用 `ps rss` / `vmmap`**：主要占用挂在 `Owned physical footprint (unmapped) (graphics)` 类目 —— IOSurface 由 WebContent 拥有、映射在 GPU 进程，`rss` 与 `vmmap` 两者都漏算（`measure.sh:23-33`）。

**CPU 口径必须用 `ps -o time=` 差值**：`ps %cpu` 是进程生命周期均值，测不出当下负载。改取累计 CPU 时间在区间前后差值 / 墙钟（`measure.sh:113-116`）。

子命令：`launch` / `mem <label>` / `cpu <label> [secs]` / `stacks <label> [secs]` / `track <label> [n] [gap]`。

压测遵守用户硬约束：`loadgen.sh` 只打分组 `mock`，其唯一成员平台 `platform_type = "mock"`（已用 `SELECT g.name,p.name,p.platform_type FROM group_platform gp JOIN "group" g ...` 核实），绝不碰真实平台。单请求靠 body 顶层 `mock` 对象控制 `chunk_count=200 / delay_ms=50 / input_tokens=4000 / output_tokens=2000`（`gateway/adapter/mock/config.rs:83-114`），造出 ~10s 长流。

**mock 无需扩充**：`MockConfig` 已含 status_code / delay_ms / stream_override / response_text / finish_reason / input·output·cache_tokens / error_mode / chunk_count，且 `body.mock` 为最高优先级层，逐请求可控。因此不触发「扩 mock 必须同步补前端展示」的约束。

**复现性**：空闲前台 CPU 两次独立运行 50.2% / 51.3%，偏差 2.2%，达标（<10%）。

### 三采样点内存分解（MB，phys_footprint）

| 采样点 | aidog(main) | GPU | Networking | WebContent | WebContent2 | TOTAL |
|---|---|---|---|---|---|---|
| ① 冷启动刚可用 | 45.0 | 25.0 | 6.7 | 300.0 | 22.0 | **398.7** |
| ② 空闲驻留 10 分钟 | 44.0 | 92.0 | 6.7 | 453.0 | 22.0 | **617.7** |
| ③ mock 50 路并发峰值 | 51.0 | 81.0 | 6.8 | 442.0 | 39.0 | **619.8** |
| （旁证）窗口隐藏 | 44.0 | 23.0 | 6.7 | 310.0 | 22.0 | **405.7** |

采样点 ② 是 10 分钟逐分钟追踪（`results/track-*`），t=60s 起即到 619.7 并走平至 t=600s 的 615.7。**内存不是持续增长，是一次性跳升后走平** —— 与「泄漏」假设不符。

**归因（采样点 ③）**：
- WebContent 442 MB 中 **369 MB 在 227 个 graphics region**（合成层 / IOSurface），53 MB WebKit malloc
- GPU 81 MB 中 54 MB 同为 graphics
- 合计 **423 / 619.8 MB ≈ 68% 是 GPU 合成面**

**Rust 主进程只有 44–51 MB**，且 50 路并发相对空闲只 +7 MB。这否掉了图上「已知起点」里 tokenizer 的分量假设 —— tokenizer 常驻（glm-4.json 19MB + qwen2 6.7MB + 两个 tiktoken 单例）即便全算进去也只是这 44 MB 的一部分，**在 619 MB 的总盘里不是主要矛盾**。主进程内更细的拆分（SQLite page cache / reqwest 连接池 / in-flight 缓冲）在这个量级下已无优化杠杆，不再深挖。

### CPU 分解

| 场景 | aidog | GPU | Networking | WebContent | TOTAL |
|---|---|---|---|---|---|
| A 空闲前台 | 4.5 | 36.2 | 0.0 | 9.5 | **50.2%** |
| A 空闲前台（复现） | 4.8 | 36.9 | 0.0 | 9.6 | **51.3%** |
| A 窗口隐藏 | 0.0 | 0.1 | 0.0 | 0.1 | **0.2%** |
| A 小窗 500×400 | 5.4 | 16.6 | 0.0 | 12.1 | **34.1%** |
| B mock 50 并发 | 11.6 | 33.5 | 0.4 | 10.3 | **55.8%** |
| C UI 驻留 | — | — | — | — | **未采，见下** |

**>2% 的栈（`results/stacks-*`）**：

GPU 进程：**1417 / 2638 采样（54%）落在 `CA::CG::DrawConicGradient::draw_color`** —— 带 mask 的 conic-gradient 走软件光栅化。

WebContent 主线程栈证明每帧全量样式重算：
`RemoteLayerTreeDrawingArea::updateRendering` → `Page::updateRendering` → `Page::layoutIfNeeded` → `Document::updateLayout` → `Document::resolveStyle` → `TreeResolver::resolvePseudoElement` → `createAnimatedElementUpdate` → `Style::Builder::applyNonHighPriorityProperties` → `CSSVariableReferenceValue::resolveTokenRange` → `CustomProperty::tokens` → `numberToCSSString`

**根因链（有证据）**：`src/styles/globals.css:828-870` 用 `@property --flow-ang` 注册自定义属性 + `@keyframes flowBorder` 逐帧动画它，该属性被 `conic-gradient(from var(--flow-ang), ...)` 消费，再叠 `mask-composite: xor/exclude`。注册型自定义属性被动画 → 每帧强制 `Document::resolveStyle` 全量重算 + 伪元素重解析，**根本无法走 compositor-only 路径**；conic-gradient + mask 又落到软件光栅化。选择器是 `.glass, .glass-surface`，全仓 116 处 `.glass-surface` 用法，**全页面共有**。

旁证：
- 窗口隐藏 → CPU 50.2% 掉到 0.2%，内存降 212 MB。**代价全部来自可见窗口渲染**，与转发无关。
- 窗口缩到 500×400 → GPU 36.9% 降到 16.6%（面积相关，符合光栅化成本模型），但 WebContent 反升到 12.1% —— 说明存在与面积无关的重绘驱动源。
- 50 路并发相对空闲只多 5.6 个百分点（55.8 vs 50.2），其中主进程 +6.8。**Rust 转发路径不是 CPU 问题，UI 渲染才是。**

### 未完成项

**场景 C（Logs / Stats / Platforms 各驻留 1 分钟）未采**。原因：aidog 未对 WKWebView 开启 accessibility，`System Events` 的 entire contents 只见到 window / button 外壳，拿不到侧栏元素；`screencapture -R` 亦无屏幕录制权限，无法定位坐标盲点。需人工驱动 UI 或应用侧开 AX。**转由 [07 UI 驻留态 CPU 归因] 承接**（该票本就是这个议题），量测手法已就绪，只缺切页动作。

### 顺带发现（不属本票，已入图）

`~/.aidog/log.db` = **7084.9 MB**，`log.db-wal` = **4446.8 MB**。WAL checkpoint 实质没在跑。与本票的内存目标无直接关系（不占进程内存），但是独立的严重问题。
