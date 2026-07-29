# 空闲期唤醒源全量勘察（6 类）

勘察范围：`src-tauri/crates/aidog_core/**`、`src-tauri/src/**`、`src/**`（前端仅轮询调用点）。
方法：`rg` 静态检索，**未运行 app、未 profiling**（主会话量测窗口占用）。
所有「实际 CPU 占比」判断均为 **推测:**，需后续 `sample`/`Instruments` 归因验证。

---

## 类 1 — tokio interval / sleep 循环 / 定时任务

检索模式：`rg -n "interval|sleep\(|tokio::spawn|thread::spawn|from_secs|from_millis"`（排除 `target/`）。

### 常驻循环全清单（生产路径，非 test）

| # | 位置 | 周期 | 空闲期是否触发 | 每轮做什么 |
|---|---|---|---|---|
| 1 | `src-tauri/crates/aidog_core/src/gateway/backup/scheduler.rs:84-91` | **60s**（`(interval_hours*3600).clamp(1,60)`，默认 24h → 恒为 60） | **是，无 gate** | `app.state::<Db>()` + `BackupSettings::load(&db).await`（一次 DB 读）→ `maybe_backup`（`scheduler.rs:25-26` 时间节流，未到点即返回） |
| 2 | `src-tauri/src/app_setup.rs:428-450` | **300s**（`coarse`），并与「距下一次本地 00:00」取小 | **是**，仅 `#[cfg(target_os = "macos")]`（`app_setup.rs:427`） | `refresh_tray_menu`：查 `today_stats` SQL + 重建托盘菜单 + `set_title`（走 `tray_render.rs` objc2 绘制） |
| 3 | `src-tauri/src/app_setup.rs:145-156` | 24h | 是（极低频） | `defaults_sync::maybe_sync_on_startup`（platform-presets 同步，含 30s 超时 HTTP） |
| 4 | `src-tauri/src/app_setup.rs:162-173` | 24h | 是（极低频） | `client_types_sync::maybe_sync_on_startup` |
| 5 | `src-tauri/src/app_setup.rs:205-212+` | 24h | 是（极低频） | `purge_all_soft_deleted` + proxy_log 三级 retention + 阈值 VACUUM（见类 6） |

**注**：#1 的 tick 是「先 sleep 后判定」型（`scheduler.rs:88-89` sleep 在 `maybe_backup` 之前），因此 60s 唤醒是硬性的，即使 `enabled=false` 也照唤醒 —— `enabled` 判定在 `maybe_backup` 内部（`scheduler.rs:17` 注释「未启用 / 距上次 < interval → 跳过」），**不阻止唤醒本身**。

### 事件驱动、空闲期不触发（已排除）

- `src-tauri/src/app_setup.rs:404-415`：`tray-refresh` 事件监听 + 200ms trailing 防抖。**仅由请求/quota/配置变更 emit 驱动**，空闲无请求 → 零唤醒。
- `gateway/proxy/log.rs:73-83` `spawn_log_writer`：`while let Some(msg) = rx.recv().await`，纯 mpsc 阻塞消费，空闲 0 唤醒。
- `gateway/proxy/mock.rs:39/98/136`：mock 协议 sleep，仅 mock 请求路径。
- `gateway/proxy/mod.rs:239` `loop`：bind 端口重试，一次性。
- `gateway/mitm/ca.rs:308`、`cert_signer.rs:128/153`、`proxy/forward.rs:584`、`proxy/connect.rs:638`、`proxy/devin.rs:764`：请求/证书路径内循环，非定时器。
- `gateway/notification/tts.rs:46/69/106` `std::thread::spawn`：TTS 播报，通知触发。
- 全部 `*test*.rs` 内 spawn/sleep：不进生产二进制。

---

## 类 2 — 托盘图标重绘

- `crates/aidog_core/src/tray_render.rs`（614 行）：objc2 直连 `NSStatusItem.button`，`NSAttributedString` 富文本标题绘制（`tray_render.rs:124-125` 说明为何绕开 `set_title`）。
- `crates/aidog_core/src/popover.rs`（109 行）。
- **重绘触发源只有两个**：
  1. `tray-refresh` 事件 → 200ms 防抖（`app_setup.rs:395-416`）—— 空闲不触发。
  2. macOS 5 分钟 tick（`app_setup.rs:432`，类 1 #2）。
- **无逐帧/逐秒重绘**：未检索到任何 <60s 的托盘刷新定时器。
- 结论：**空闲期托盘重绘 = 每 5 分钟 1 次**。推测: CPU 占比可忽略（<0.05%），除非 `today_stats` SQL 在大库上很贵（当前库已知可达 200MB 级，见 `.scratch/perf-200mb/`）。

---

## 类 3 — 前端定时 invoke 轮询

- **`setInterval` 全项目 0 处**（`rg "setInterval" src/` 无匹配）。
- `setTimeout` 约 60 处，**全部为一次性**：toast 自动消失（3000/2000ms）、debounce（300/350ms）、copy 状态复位、focus 延迟。无自重排（无 `setTimeout` 递归自调用）。
- `requestAnimationFrame` 4 处，均非常驻：
  - `src/utils/motion.ts:77/79` — 计数动画，`p < 1` 时递归，动画结束即止。
  - `src/popover.tsx:210` — ResizeObserver 回调内单发。
  - `src/pages/platforms/usePlatformsState.ts:229` — 拖拽期间。
- `src/pages/Skills/useSkillsData.ts:171` — `visibilitychange` 监听触发 revalidate，**事件驱动非轮询**。
- 结论：**未发现前端定时 invoke 轮询**（已查 `setInterval` / `requestAnimationFrame` / 递归 `setTimeout`）。此类否定。

### 但发现相邻问题：常驻 CSS 无限动画（不是 invoke，但是持续 CPU）

| 位置 | 动画 | 周期 |
|---|---|---|
| `src/styles/globals.css:186` | `shimmer 1.4s ease-in-out infinite` | 常驻 |
| `src/styles/globals.css:204` | `spin 0.9s linear infinite` | 常驻 |
| `src/styles/globals.css:215` | `pulseGlow 3s ease-in-out infinite` | 常驻 |
| `src/styles/globals.css:290` | `statusPulse 2s ease-in-out infinite` | 常驻 |
| `src/styles/globals.css:909` | `flowBorder 3s linear infinite` | 常驻 |
| `src/styles/globals.css:955` | `progressStripes 0.6s linear infinite` | 常驻 |
| `src/styles/popover.css:77` | `statusPulse 2s ease-in-out infinite` | 常驻（**popover 窗口预建常驻**，见下） |

已有部分缓解：`globals.css:965` `animation-play-state: paused`（条件待核）、`globals.css:968` + `popover.css:80` `@media (prefers-reduced-motion: reduce)`。**无 `document.hidden` / 窗口失焦 gate**（`rg "document.hidden"` 无匹配）。

**popover 窗口空闲期常驻存活**：`src-tauri/src/app_setup.rs:306` `prebuild_popover(app.handle())` → `app_setup.rs:494-517` 启动即创建隐藏 popover 窗口（webview 提前 boot + React 提前 mount）。`app_setup.rs:513` `apply_popover_hides_on_deactivate` 仅控制显隐，窗口与其 React 树、CSS 动画持续存活。

---

## 类 4 — 文件系统 watcher

检索模式：`rg "notify::|RecommendedWatcher|new_debouncer|watcher|inotify|FSEvent" src-tauri --glob '*.rs'`。

- 唯一匹配 `crates/aidog_core/src/gateway/proxy/mod.rs:108` `pub(crate) use notify::handle_notify;` —— 这是**项目内 `gateway/proxy/notify.rs` 模块**（Anthropic 协议的 notify 端点处理），**不是 `notify` crate**。
- `Cargo.toml` 中**无 `notify` / `notify-debouncer` 依赖**（`rg "^notify" src-tauri/crates/aidog_core/Cargo.toml` 无匹配）。
- 结论：**未发现文件系统 watcher，已查 `notify::` / `RecommendedWatcher` / `new_debouncer` / `inotify` / `FSEvent`**。此类否定。

---

## 类 5 — 网络监听态

- **axum accept loop**：`gateway/proxy/mod.rs:239-257` bind → `axum::serve`。accept 阻塞在 kqueue/epoll，**无连接时 0 CPU**（tokio 标准行为）。空闲无唤醒。
- **代理是否常驻**：`app_setup.rs` 尾部 `settings.autostart` → `proxy_start`。开了 autostart 则 listener 常驻，但仍是阻塞 accept。
- **reqwest 连接池 keepalive**：`gateway/http_client.rs:100-140` builder 只设 `timeout` / `connect_timeout` / `proxy`，**未配置 `pool_idle_timeout` / `tcp_keepalive` / `http2_keep_alive_interval`**（`rg` 全项目 0 匹配）→ 走 reqwest 默认（默认 `pool_idle_timeout=90s`，无 TCP keepalive、无 HTTP/2 ping）。空闲期无上游连接 → 池为空 → 无唤醒。
- **健康探测**：`GET /` `/proxy` `/models` 是**被动端点**（CLAUDE.md 记载），无主动出站探测。
- **未发现主动心跳 / 轮询上游**：已查 `tcp_keepalive` / `http2_keep_alive` / `pool_idle_timeout`。
- 结论：类 5 **无空闲唤醒源**。

---

## 类 6 — SQLite WAL checkpoint / retention / VACUUM

- **retention + VACUUM 调度**：`src-tauri/src/app_setup.rs:205-212+`，**24h 循环**，`app_setup.rs:210` sleep 在前（启动不立即跑）。每轮做 `purge_all_soft_deleted` + proxy_log 三级 retention + `cleanup_notifications` + 阈值 100MB 触发 `compact_database`（全量 VACUUM）。
- **VACUUM 执行位置**：`app_setup.rs` 注释说明经 `db.call_traced` 跑在 DB 专属后台线程；`compact_database` 内含 `wal_checkpoint(TRUNCATE)` + `ANALYZE`。
- **WAL checkpoint**：未检索到 `wal_autocheckpoint` 自定义设置（`rg "wal_autocheckpoint"` 仅 maintenance.rs 内 `wal_checkpoint(TRUNCATE)` 显式调用，位于 `gateway/db/maintenance.rs:192` 的 auto_vacuum 迁移路径）。→ 走 SQLite 默认 `wal_autocheckpoint=1000 页`，**写触发、非定时**，空闲无写 → 无 checkpoint。
- **`auto_vacuum=INCREMENTAL`**：`gateway/db/maintenance.rs:150-211`，`incremental_vacuum` 是显式调用，非后台线程。
- 结论：类 6 **空闲期唤醒频率 = 每 24h 一次**，占比可忽略。24h 那一轮的 VACUUM 会是**尖峰**（锁库 + 全库重建），但不是稳态 3% 的来源。

---

## 归因缺口（必须 profiling 才能闭合）

静态检索出的全部 Rust 定时器加总：**60s 一次 DB 读 + 300s 一次托盘刷新 + 24h×3**。
推测: 这个频率**解释不了 3.0% 的稳态 CPU** —— 60s 一次轻量 DB 读的均摊占比应在 0.01% 量级。

因此 3.0% 的大头 **推测:** 在下列之一（静态检索无法证实）：
1. 主窗口 + 预建 popover 窗口的常驻 CSS `infinite` 动画 → 持续合成/CA 提交，前台前提下计入 app 进程（类 3 附录）。
2. Tauri/WKWebView host 侧的 IPC / 事件循环基线开销（与本项目代码无关的框架底噪）。
3. 某个未在源码显式出现的第三方 crate 内部线程（如 `tao`/`wry`/`tray-icon` 的事件循环、`rustls` 后台、`objc2` autorelease pool）。

**需要: 是否允许在量测窗口结束后（16:16 之后）跑一次 `sample <pid> 10 -f out.txt` 或 `Instruments Time Profiler` 做栈归因？** 没有栈归因就无法区分上述 3 者，优化方向会是猜的。
