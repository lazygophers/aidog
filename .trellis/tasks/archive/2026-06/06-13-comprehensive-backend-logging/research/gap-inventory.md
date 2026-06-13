# 日志盲区清单 + 静默吞错点分类（实施前必读）

来源：grep 扫描 src-tauri/src，2026-06-13。

## 关键триage：哪些 `let _ =` 该记，哪些不该

### db.rs（10 处 `let _ =`）— 全部是幂等 migration，**不要记 error**
`src/gateway/db.rs:71-85` 全是 `ALTER TABLE ... ADD COLUMN`，列已存在时故意失败吞掉。
→ 这些是**有意静默**，最多加 `tracing::trace!` 不加 warn。不要当 bug 处理。
真正要补的是 db.rs 里 settings 读写、cleanup、stats 更新的失败路径（grep `map_err` 上抛已覆盖，仅找额外 `.ok()` 静默处）。

### lib.rs（21 处 `let _ =`）— 分两类
**该记 warn（失败有业务影响）：**
- `lib.rs:354,814` `do_sync_group_settings` — 设置同步失败影响 statusline
- `lib.rs:996,997,1000` cleanup_* — 清理失败影响存储
- `lib.rs:1325` `std::fs::write` settings 文件 — 写失败丢配置
- `lib.rs:1435` `save_proxy_settings_to_db` — 持久化失败
- `lib.rs:1437,903` `std::fs::remove_file` — 删除失败（可降级 debug）
- `lib.rs:2042,2078` `proxy_start` / `:2047` `proxy_stop` — 代理启停失败必须 error
- `lib.rs:675,710,732` `upsert_proxy_log` — 日志写库失败可 debug

**不必记（纯 UI fire-and-forget）：**
- `lib.rs:1104` `emit("tray-refresh")`、`2052/2053` `w.show()/set_focus()`、`2093` `w.hide()`、`2068` refresh_tray_menu

## 命令面（lib.rs 69 个 `#[tauri::command]`）
每个入口加 `tracing::debug!(command="<fn名>", <脱敏关键参数>, "command invoked")`。
含密钥参数（api_key/token）→ 不打值或 `[REDACTED]`。
每个命令的 `Err` 终止点加 warn/error（含命令名）。

## 后台模块（全 0 日志）
- `price_sync.rs`：4 处 Err（line 见 grep `return Err`）；同步启停加 info
- `codex.rs`：TOML 读/写/parse（10 处 map_err）失败 → warn/error
- `manual_budget.rs`：耗尽/重置判定 → debug
- `estimate.rs`：calibrate_from_quota 失败 early-return（estimate.rs:97/105/312）→ warn

## 出站（quota.rs）
`err_quota` 辅助 + 各配额函数失败路径 → `tracing::warn!(platform, error)`。
出站请求/响应已有 `quota_get_json`（info/debug），勿重复。

## 脱敏既有模式（复用，勿新造）
- proxy.rs:462 `authorization` → `[REDACTED]`
- proxy.rs:1170 passthrough header redact
日志中所有 api_key/Bearer/token 值套同款 redact。

## 运行期可见性
- dev：logging.rs 已强制 console = debug（所有级别进终端）✅
- release：console 跟随 settings.level，file 同。确认无「仅落文件」的 log（当前架构 console layer 始终在）。
