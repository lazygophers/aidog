# WKWebView 内存下限调研

不完整：网络检索未找到 Apple/WebKit 官方发布的"WebContent 进程固定最小值"数字（Apple 明确表态该值不是固定的，随设备 RAM/系统负载动态变化——见 §1）。以下结论以本仓实测数据为主锚点，外部检索作交叉验证。

## 结论：WebView 侧下限约 100–150 MB（≥2 个 WKWebView 实例常驻时）

单 WKWebView 实例冷启动 helper 进程（GPU + Networking + WebContent）合计约 **50–55 MB**（本会话实测，见下）；aidog 架构在 app 生命周期内至少同时存活 **2 个** WebviewWindow（main + 预建 popover，`src-tauri/src/app_setup.rs:501`），Networking/GPU 进程按 WebKit 多进程模型可能按 App 或按 WKProcessPool 复用（未在本仓证实 Tauri/wry 是否共享池，见 §2），WebContent 进程通常**不跨窗口共享**。保守估算 WebView 侧下限（冷态、页面简单、无用户交互）落在 **100–150 MB** 区间，随页面复杂度（DOM/JS heap/图片解码缓存）向上浮动无硬顶。

## 1. 多进程模型与常驻 RSS，是否随页面复杂度线性增长

WKWebView 在 macOS 上采用多进程架构：每个 WebView 关联一个 `com.apple.WebKit.WebContent`（渲染/JS）+ 共享/独立的 `com.apple.WebKit.Networking`（网络栈）+ 按需启动的 GPU 进程。三个 WebView 若各自独立进程池，理论上可产生 7 个 helper 进程。[embrace.io](https://embrace.io/blog/wkwebview-memory-leaks/) [tauri-apps discussion #11553](https://github.com/orgs/tauri-apps/discussions/11553)

本仓实测（team-lead 提供的冷启动数据，作为已知基线）：aidog 主进程 phys_footprint 68 MB（peak 84 MB）；配套 WebKit helper ps rss ≈ GPU 25.7 MB + Networking 3.7 MB + WebContent 24.0 MB + 2.8 MB（约 56 MB helper 合计，单实例口径）。

Apple 官方文档给出的 `footprint` 工具示例输出（非严格"最小值"，是示例）：WebContent phys_footprint 142 MB，phys_footprint_peak 424 MB。[WebKit Memory Inspection docs](https://docs.webkit.org/Infrastructure/MemoryInspection.html)

是否随页面复杂度线性增长：定性成立（DOM 节点数、JS heap、图片/字体解码缓存、CSS 合成层都计入 WebContent 进程），但**无公开线性系数**；Apple 工程师在开发者论坛明确表态该进程"memory usage is attributed to your app"、且限制值"isn't a fixed number — it depends on total device RAM and current system load"。[Apple Developer Forums](https://developer.apple.com/forums/thread/663084) 推测: 简单静态页（无图/无第三方 JS）复杂度对 RSS 的边际影响远小于图片解码缓存与长驻 JS heap 的影响，但未找到一手数字量化。

## 2. Tauri 2.0 暴露的 WebView 配置面

- `WebviewWindowBuilder` 支持 `incognito`（禁磁盘持久化，Tauri 2.0 stable 新增）与 `data_directory`（自定义 webview 数据目录）。[Tauri 2.0 Stable Release](https://v2.tauri.app/blog/tauri-20/) [tauri docs.rs WebviewWindowBuilder](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html)
- `WKWebViewConfiguration`/`WKProcessPool` 透传：**未在 Tauri/wry 官方 API 中找到公开绑定**；wry issue 中开发者需自行通过底层 `WKWebView` 句柄手工设置共享 `WKProcessPool` 以跨窗口共享 cookie/storage 与进程。[wry #621](https://github.com/tauri-apps/wry/issues/621) 本仓未见此类自定义代码（`grep -rn "WKProcessPool\|WKWebViewConfiguration" src-tauri` 无命中）——推测: aidog 当前每个 WebviewWindow 各自默认 `WKProcessPool`，未共享。
- 进程池复用：Tauri 官方立场（维护者原话）"we have no real control about the webview memory usage, so that's something that has to be improved upstream"，即 WebKit 侧行为 Tauri 层无法调优。[tauri-apps discussion #3162](https://github.com/orgs/tauri-apps/discussions/3162)
- 隐藏时 suspend/释放：**无此 API**。macOS/WKWebView 没有 Windows WebView2 那种显式 `TrySuspendAsync`；隐藏 WebView 后内存是否回收完全由 WebKit 内部与 OS 内存压力决定，App 层不可触发。[Microsoft WebView2 TrySuspendAsync（对照组）](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2.trysuspendasync) [Apple Developer Forums](https://developer.apple.com/forums/thread/22795)

## 3. 窗口隐藏/最小化时能否释放内存或被 jetsam 回收

不能主动触发；Apple 工程师原话"The OS decides when to release"。已知案例：隐藏的 WKWebView（含 alpha=0.01 技巧）仍可能被 WebKit 判定为不可见而暂停渲染循环，但**不等于内存被回收**——多篇报告称 WKWebView 移出视图层级后内存仍不释放，需等待系统内存压力事件。[Apple Developer Forums thread/89331](https://developer.apple.com/forums/thread/89331) [Apple Developer Forums thread/22795](https://developer.apple.com/forums/thread/22795)

Tauri 侧已知反例更差：`WebviewWindow` **关闭（非隐藏）**后进程和内存都不释放（macOS/Win10 均复现）。[tauri #5397](https://github.com/tauri-apps/tauri/issues/5397) 这意味着本仓 `prebuild_popover`（`src-tauri/src/app_setup.rs:487-514`）选择"隐藏、永不销毁"而非"关闭重建"的策略，即使换成"用完即关"也拿不回内存——当前设计并未留下明显可省的账。

Jetsam（内存压力回收）在 macOS 上主要面向后台 App 整体，而非单个前台 App 内的 helper 进程；WebContent 进程若被判定为可回收会由系统触发，但这是系统级行为，App 无法调用触发，也无法保证时机。推测: 对本仓这种常驻托盘应用（非典型"退到后台"App），触发概率更低。

## 4. 双 HTML 入口（index.html + popover.html）+ 托盘，是否意味着多个 WebView 实例

**是，已在代码中确认 2 个 WebviewWindow 同时存活：**

- 主窗口：`tauri.conf.json` 声明的 `main` 窗口，`WebviewUrl` 指向 `index.html`（`src-tauri/tauri.conf.json` app.windows[0]）。
- Popover：`prebuild_popover()` 在 setup 阶段主动预建，`WebviewUrl::App("popover.html")`，`.visible(false)` 隐藏但**不销毁**，注释明确写"create-once...首次 tray click 直接 show（不再冷启建窗）"，`src-tauri/src/app_setup.rs:487-514`。命令注册表也确认该窗口全程存在：`window.label() == "popover"` 事件 handler 常驻（`src-tauri/src/startup.rs:34-37`）。

托盘本身**不是 WebView**：`tray_render.rs` 走 `tauri::tray::TrayIcon` → `ns_status_item()` 原生 NSStatusItem/NSMenu 路径（`src-tauri/crates/aidog_core/src/tray_render.rs:125-139`），无 HTML 渲染，不产生额外 WebContent 进程。

结论：WebView 实例数 = 2（main + popover），非 3；托盘不计入。

## 5. 社区把 Tauri WebView 内存压到最低的已知做法与代价

见下方可调项清单。整体判断：社区/官方均未给出能把 WKWebView 常驻内存打到显著低于其"天然量级"（数十 MB 级/实例起）的方法——多数手段是**避免叠加**（少开窗口、少加载重资源），而非压低单实例地板。

## 可调项清单

| 可调项 | 怎么调 | 预期省多少 | 代价 / 踩哪条红线 |
|---|---|---|---|
| Popover 改"用时创建、关闭即销毁"而非常驻隐藏 | 去掉 `prebuild_popover` 的预建逻辑，首次 tray click 时才 `WebviewWindowBuilder::new(...).build()` | 推测: 省一份常驻 WebContent（~25-30MB），但 Tauri `#5397` 显示 close 后进程/内存**不一定真正释放**（macOS/Win10 均有报告），实测前无法确认净收益 | 直接违反 ④ 冷启动速度红线（当前设计注释明确写"首次 tray click 直接 show（不再冷启建窗）"就是为了避免弹窗延迟），且可能不省钱（见上）——**不推荐** |
| `incognito: true` | `WebviewWindowBuilder` 加 `.incognito(true)`（Tauri 2.0 API） | 推测: 省磁盘 IO 与部分持久化缓存写入路径，内存收益未知，量级应远小于 10MB | 需验证是否影响需要持久化的功能（如 popover 若无本地存储需求则安全）；未在本仓确认 popover 是否依赖 localStorage |
| 共享 `WKProcessPool` 跨 main/popover 窗口 | 需绕过 Tauri/wry 公开 API，走底层 `WKWebView` 句柄手工设置（wry 未原生支持，`#621`） | 推测: 若两窗口进程能合并，省一份 WebContent 基线（~24-30MB） | 工程量大（越过框架边界操作平台句柄，未来 Tauri 升级易破），且会让 main 与 popover 共享 cookie/storage，可能引入非预期状态耦合；风险/收益比差，**不建议为省 <30MB 承担此维护成本** |
| 减少 popover.html 打包体积/JS 依赖 | 检查 popover 页面是否引入了整套前端框架/组件库而非精简子集 | 推测: 省 JS heap 部分（个位数~十几 MB），非 helper 进程基线部分 | 无红线冲突，是最低风险选项，但收益上限有限（不解决多进程固定开销） |
| Networking/GPU helper 是否可关闭 | 无公开开关；GPU 进程按需启动（是否用 GPU 合成/视频解码触发），若页面纯 DOM+CSS 无 canvas/video 可能不拉起 GPU 进程 | 推测: 若本身未触发 GPU 进程可省 ~25MB（本仓实测 GPU helper 单项最大），但这是"不做某些渲染"的被动结果，非主动可调开关 | 若当前 UI 已用到需要 GPU 合成的效果（本仓 CLAUDE.md 明确 UI 风格 = Liquid Glass，`backdrop-filter`/`transform` 类效果通常走 GPU 合成层），关闭会直接破坏既定 UI 风格——**大概率与产品要求冲突，不可行** |
| 不新建任何 WebView 实例（架构级） | 唯一能把「实例数」本身压下来的手段：把 popover 功能重做成主窗口内的一个 view/panel，而非独立 WebviewWindow | 推测: 省一整份 WebContent + 可能的 GPU/Networking helper（~50MB 量级，本仓单实例实测口径） | 改动面大，涉及窗口失焦自动隐藏（`hidesOnDeactivate`）等平台特性目前依赖独立 NSWindow 实现（`app_setup.rs:34-37`, `apply_popover_hides_on_deactivate`），合并进主窗口需要重新设计交互；超出"调参"范畴，是架构级重构 |

## 明确回答：全进程总和 ≤200MB 在当前架构下是否可能

**有条件可能，条件苛刻。**

- 本会话实测冷启动基线：aidog 主进程 phys_footprint 68MB + 单份 WebKit helper ~56MB（GPU 25.7 + Networking 3.7 + WebContent 24.0+2.8）= **~124MB**（这还只是 main 窗口一份，未计入常驻的 popover WebContent）。
- 若 popover 的 Networking/GPU 进程与 main 共享（Tauri/wry 是否默认共享未证实，§2 推测为不共享），则总和落在 **~150-180MB**（124MB + 一份额外 WebContent ~25-30MB）冷启动静态区间，尚未计入题面要求的"50 路并发转发的持续峰值口径"——即请求处理期间 Rust 侧连接池/序列化缓冲区膨胀（本调研范围外，不覆盖）。
- 若 popover 与 main 各自独立 WKProcessPool（更可能的默认行为），总和会突破 **200MB**（124MB + 独立一份 GPU+Networking+WebContent ~56MB ≈ 180MB 冷启动，仍未算并发峰值）。
- 结论：**冷启动静态口径下"WebView 侧 ≤200MB"本身处于临界边缘、大概率不满足**；若再叠加题面要求的"50 路并发持续峰值"（Rust 侧内存会进一步增长），**全进程总和 ≤200MB 在当前双 WebView 常驻架构下大概率不可行**，除非：① 证实 main/popover 确实共享 WKProcessPool（把双份 helper 压成一份），或 ② 接受架构级重构（把 popover 合并进主窗口，从根上消除第二个 WebContent 实例）。两条路径都需要额外验证/工程投入，非配置项调优可达成。
