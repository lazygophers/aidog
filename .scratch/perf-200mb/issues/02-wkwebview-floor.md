# 02 WKWebView 常驻内存下限与可调项

Type: research
Status: resolved
Blocked by: —
Parent: [深度性能优化：全进程峰值内存 ≤200MB + 三场景 CPU 下降](../map.md)

## Question

macOS 上一个 Tauri 2.0 应用的 WKWebView 侧（WebContent + Networking + GPU helper）常驻内存的**物理下限**是多少？有哪些可调项能压下去？

这张票直接决定 200MB 目标是否成立：如果 WebView 侧下限就是 300MB，那「全进程 ≤200MB」在当前架构下不可达，[03] 必须重设目标或重划口径。

调研内容：
- WKWebView 多进程模型在 macOS 上各 helper 的典型常驻量，以及是否随页面复杂度线性增长
- Tauri 2.0 暴露了哪些 WebView 配置面（`WebviewWindowBuilder` 的相关选项、`WKWebViewConfiguration` 能否透传、进程池复用、`suspend` / 隐藏时释放）
- macOS 是否支持在窗口隐藏 / 最小化时让 WebContent 进程释放内存或被系统 jetsam 回收
- 本仓有两个 HTML 入口（`index.html` + `popover.html`）与托盘，是否意味着多个 WebView 实例 / 多套 helper 进程
- 已知的 Tauri 社区实践：把 WebView 内存压到最低的做法与代价

**来源要求**：优先 Apple 官方文档（WebKit / WKWebView）、Tauri 官方文档与 issue、WebKit 源码或邮件列表。二手博客只作线索，结论必须回到一手源。

## 验收

- 一份带引用的 Markdown 结论文件，写明「WebView 侧下限约 X MB，依据是 <一手来源>」
- 可调项清单：每项写明「怎么调 / 预期省多少 / 代价是什么（是否踩四条体验红线）」
- 明确回答：当前架构下「全进程 ≤200MB」是否可能

## Answer

全文见 `assets/research-wkwebview-floor.md`（带引用）。以下是**与 [01] 实测对账后的结论**，有冲突处以实测为准。

### 调研确认的事实（代码 / 官方源）

- **WebView 实例 = 2**：主窗口（`tauri.conf.json` app.windows[0] → `index.html`）+ popover（`app_setup.rs:487-514` 的 `prebuild_popover()`，`WebviewUrl::App("popover.html")`、`.visible(false)`、create-once 永不销毁）。托盘走原生 `NSStatusItem`（`tray_render.rs:125-139`），**不产生 WebContent 进程**。这解释了 [01] 实测里为什么恒有两个 WebContent。
- **Tauri 层无 WebView 内存调优面**。Tauri 维护者原话："we have no real control about the webview memory usage, so that's something that has to be improved upstream"（[discussion #3162](https://github.com/orgs/tauri-apps/discussions/3162)）。
- **macOS 无 suspend API**。没有 WebView2 的 `TrySuspendAsync` 对等物；隐藏后是否回收内存完全由 WebKit 与 OS 内存压力决定，app 层不可触发（[Apple 论坛 thread/22795](https://developer.apple.com/forums/thread/22795)）。
- **关窗也不还内存**：`WebviewWindow` 关闭后进程与内存都不释放，macOS/Win10 均复现（[tauri #5397](https://github.com/tauri-apps/tauri/issues/5397)）。**推论：把 popover 改成「用完即关」拿不回内存，还踩红线 4（冷启动速度）—— 这条路直接排除。**
- **`WKWebViewConfiguration` / `WKProcessPool` 无公开绑定**，要共享进程池得越过 wry 绕到底层句柄（[wry #621](https://github.com/tauri-apps/wry/issues/621)）；本仓 `grep -rn "WKProcessPool\|WKWebViewConfiguration" src-tauri` 无命中，即当前两窗口各自默认进程池。
- Apple 明确表态 WebContent 的内存限制"isn't a fixed number — it depends on total device RAM and current system load"（[Apple 论坛 thread/663084](https://developer.apple.com/forums/thread/663084)）。**不存在官方公布的固定下限数字**，这也是调研自陈不完整的原因。

### 与 [01] 实测的对账（重要校正）

调研的地板估算（「单实例 helper ~56MB」「WebView 侧下限 100–150MB」）**基于 `ps rss`，口径错**。[01] 已证 `rss` 与 `vmmap` 都漏算 `Owned physical footprint (unmapped) (graphics)`。同一时刻 `footprint` 口径下冷启动 WebContent 是 **300MB** 而非 24MB。

因此两处结论要翻转：

1. **调研高估了 popover 的代价**。实测第二个 WebContent 只有 **22–39MB**（[01] 全部采样点），不是估的 25–30MB 的独立完整一份 helper。GPU / Networking 实际只各一份。**「合并 popover 进主窗口」这条架构级路线，收益仅 ~22–39MB，不值那个改造量 —— 排除。**
2. **调研低估了总量，但高估了「地板」**。619.8MB 峰值里 **423MB 是 graphics region（合成面 / IOSurface）**，这**不是固定地板，是可变成本** —— 它随合成层数量与窗口面积走（[01] 已证：窗口缩小 GPU CPU 减半；窗口隐藏立降 212MB）。

扣掉合成面后的**真实不可压地板**约为：

| 项 | MB |
|---|---|
| aidog 主进程（Rust，含 tokenizer 单例） | 44 |
| WebContent #1 WebKit malloc | 53 |
| WebContent #2 WebKit malloc | 23 |
| GPU 进程 MALLOC_SMALL + IOAccelerator | ~22 |
| Networking | 6.7 |
| **地板合计** | **≈ 149** |

### 明确回答：当前架构下「全进程 ≤200MB」是否可能

**可能，但只有一条路：砍合成层。**

- 地板 ≈149MB，离 200MB 只剩 **~51MB 的合成面预算**；当前实际用掉 423MB。
- 换句话说，**需要把 GPU 合成面砍掉约 88%**。
- 调研列出的所有「配置项级」可调项（`incognito`、共享 ProcessPool、精简 popover 打包、关 GPU 进程）**加起来也够不到这个量级**，且多数踩红线或不可行 —— 尤其「关 GPU 进程」与本仓 Liquid Glass 风格（`backdrop-filter` / `transform`）直接冲突。
- **[01] 的根因发现给了这条路可行性**：227 个 graphics region 不是 UI 复杂度的必然产物，而是 `globals.css:828-870` 那套 `@property` 动画 + conic-gradient + mask 三件套逼出来的强制提层，叠加散落的 `backdrop-filter` / `will-change`。这是**自己造的**成本，不是 WebKit 的地板。

**结论交给 [03] 裁定**：200MB 不是「架构不可达」，是「视觉方案不可达」。真正的取舍不在进程模型，在 Liquid Glass 的实现方式 —— 需要用户对「视觉 vs 内存」拍板。[03] 应据此重述问题，而不是去讨论拆进程 / 换 WebView。
