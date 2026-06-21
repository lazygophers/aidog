# 最终日志 + request-id 缺口 实现 spec（待 stats-agg workflow 落地后启动）

> ⚠️ 依赖：必须在 stats-agg workflow (wqjb37aeu) 完成提交后启动。
> migration 编号：stats-agg 占 011，本任务用 **012**（启动前 grep migrations/ 确认最新编号，递增）。
> 文件重叠提醒：本任务改 proxy.rs/db.rs/lib.rs/notification.rs/migration/models.rs —— 与 stats-agg 同批文件，必须串行，单写者。

## 用户决策
- 最终日志 = **两者都要**：(A) 终态汇总条 + (B) is_final 标记位。
- 三处覆盖：控制台/文件日志 + proxy_log + notification。
- request-id 缺口：日志带完整 32-hex id 串联 + 补 3 命令追踪 + 应用行为 key 入 notification。
- 不做：Logs 列表按 id 搜索。

## 已确认现状（request-id 调研，file:line 为准）
- request_id 生成：`proxy.rs:693` `uuid::Uuid::new_v4().simple().to_string()`（32-hex），即 `proxy_log.id`（PK）。create/update 已按它增量 upsert（proxy.rs:508-537）。
- span：`proxy.rs:691-695` `tracing::info_span!("req", trace_id = %&request_id[..8])` —— 仅 8-hex 进日志。
- 终态判定：`proxy.rs upsert_log()` is_terminal ~513；终态 emit "proxy-log-updated"/"tray-refresh" ~539-552。
- 本地 API span：`proxy.rs:171` group_info 用 new_trace_id()（8-hex）。
- 3 个 command 缺 instrument：`lib.rs:59 about_info`、`lib.rs:2164 backup_settings_get`、`lib.rs:2172 backup_settings_set`。其余 143/146 已有 `#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]`。
- notification：notification.rs dispatch + render 模板 + vars（见 memory notification-module / notification-default-templates）。

## 实现清单
### 1. 日志带完整 id 串联
- 改 `proxy.rs:691` span：在保留 8-hex trace_id 同时增加完整 `request_id = %request_id` 字段（或把 trace_id 直接改全 32-hex —— 与用户确认偏好，默认**增字段保留两者**，兼容现有日志习惯）。使该请求所有子 tracing 行都带完整 id。

### 2. 最终日志汇总条（A）
- 在 proxy.rs 终态路径（is_terminal 处）输出一条结构化「最终日志」：`tracing::info!(target:"final", request_id=%id, status=..., input_tokens=..., output_tokens=..., cache_tokens=..., est_cost=..., duration_ms=..., "request final")`。控制台/文件各一行。
- ⚠️ 注意与 stats-agg 写入挂载点是同一终态区域，协调好顺序，别重复计算。

### 3. is_final 标记位（B）
- migration 012：proxy_log 加列 `is_final INTEGER NOT NULL DEFAULT 0`（inline ALTER 兼容旧库，参考现有 004-010 ALTER 模式）。
- proxy.rs upsert_log：终态 UPDATE 时置 is_final=1。
- api.ts ProxyLogDetail 加 is_final；Logs 详情页可显示「最终」标记（可选小徽章）。

### 4. notification 带最终日志
- 请求完成通知（已有 notification 体系）的 vars 注入：request_id（唯一 key）+ 最终状态 + tokens + cost。render 模板可引用。确认默认模板是否需加这些字段展示（参考 notification-default-templates，至少 vars 可用）。

### 5. 补 3 命令追踪
- lib.rs:59/2164/2172 三处加 `#[tracing::instrument(skip_all, fields(trace_id = %crate::logging::new_trace_id()))]` + 入口 `tracing::debug!(command=..., "command invoked")`，与其余 command 一致。

### 6. 应用行为 key 入 notification
- 用户行为(tauri command) 触发的通知，notification vars 带上该 command 的 trace_id（唯一 key），与代理请求口径一致。定位 command→notification 的触发链（hooks.rs / notification dispatch），把 key 传进去。

## 门禁
- `cd src-tauri && cargo clippy -- -D warnings`（make lint）零 warning；`cargo test` 不回归（is_final/最终日志/汇总条加单测）。
- `yarn build`；`node scripts/check-i18n.mjs` 零缺失（若动前端文案）。
- git commit（`feat(logging): 请求最终日志汇总+is_final标记 + 完整id串联 + 补命令追踪`），禁 push。

## 验证维度（并行只读审查）
- 最终日志：终态确实输出汇总条 + is_final 置位 + notification vars 带 key/汇总，三处齐。
- id 串联：proxy 请求所有日志行带完整 request_id，能从日志串回 proxy_log。
- 不回归：终态汇总不与 stats-agg/proxy_log 写入重复或冲突；重试/流式只在最终终态发一次。
- 命令追踪：3 命令补齐，无遗漏。
