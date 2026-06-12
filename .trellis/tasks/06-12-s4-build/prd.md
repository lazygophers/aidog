# S4 构建 / 启动配置优化

> Parent: [06-12-p0-p1](../06-12-p0-p1/prd.md) · 收尾，与代码 subtask 基本独立（tray/cold_start 触碰 lib.rs，须在 S1/S2 后或协调）。

## Goal

降低 release 二进制体积、启动内存与时间，去除 UI 启动阻塞。功能零回归。

## Requirements

- R5.1 Cargo.toml 加 `[profile.release]`：`opt-level=3` + `lto="thin"` + `codegen-units=1` + `strip=true` + `panic="abort"`。**先排查代码是否依赖 panic unwind（catch_unwind）**，有则保留 `panic="unwind"`。
- R5.2 vite.config.ts 加 `build.rollupOptions.output.manualChunks`：react / pinyin-pro / i18n / dnd-kit 分包。
- R5.3 tokio runtime 显式限 `worker_threads`（2~4），保留 multi-thread（**不改 current-thread**）。
- R5.4 tray 菜单 `block_on`（lib.rs:1969）→ `spawn` + `emit` 事件，前端监听更新；cold_start 配额查询（lib.rs:2011）延迟到窗口 ready。

## Acceptance Criteria

- [ ] `cargo build --release` 成功，二进制体积较优化前显著下降。
- [ ] `vite build` 后主 chunk 明显减小，分包产物正确加载。
- [ ] `panic="abort"` 不破坏现有 panic 处理（catch_unwind 排查通过，否则降级 unwind 并记录）。
- [ ] tray 启动/停止代理不卡 UI；cold_start 不阻塞首屏。
- [ ] 功能零回归。

## Out of Scope

- 删 reqwest stream feature（已否决，stream 在用）。
- tokio current-thread（已否决）。
- bundle targets 裁剪 / CSP（非运行性能，另议）。

## Technical Notes

- R5.4 触碰 lib.rs，与 S1/S2 同文件 → 排在其后或独立 worktree 合并时协调。
- 遵 cross-layer-rules：新增 Tauri event 需前端监听对应。
