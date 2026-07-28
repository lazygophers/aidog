# 02 WKWebView 常驻内存下限与可调项

Type: research
Status: open
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
