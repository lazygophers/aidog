# 启动不做定时操作 — PRD (主入口)

## 目标
应用启动时不再立即执行任何周期定时操作的「首跑」；周期定时器保留，但仅在间隔到期后首次触发。
前端 5 处可见性轮询（usePolling）改为后端事件推送（emit → listen），消除定时轮询。
用户语义：「直接启动的时候不主动跑，定时器自动触发的不算」。

## 边界
范围内：
- [ ] app_setup.rs 中 4 处 spawn 后立即首跑的周期定时器：
  1. `gateway::backup::spawn_scheduler` (backup/scheduler.rs L82-91 启动 maybe_backup + cleanup_expired)
  2. `defaults_sync` (app_setup.rs L145-163 启动 maybe_sync_on_startup)
  3. `client_types_sync` (app_setup.rs L167-185 启动 maybe_sync_on_startup)
  4. `scheduled_cleanup` (app_setup.rs L212-301 loop 先 cleanup 后 sleep → 改先 sleep)
- [ ] 前端 usePolling 调用点 5 处：Logs/useLogsData (list 30s + detail 5s)、RequestLog (list 30s + detail 5s)、PopoverConfigTab/usePopoverConfig (stats 30s)、TrayConfigTab (stats 30s)
- [ ] 后端补 emit 事件：proxy_log 终态落库后 + retention 清理后 emit `proxy_log_changed`；Popover/TrayConfig stats 触发点 emit（复用 tray-refresh 或专用 event）

范围外（保留不动）：
- [ ] 一次性迁移任务（非定时）：migrate_auto_vacuum / rebuild_stats_agg_once_if_needed / correct_count_tokens_agg_once_if_needed / logo_sync 预热 / cold_start_init_tray_estimates
- [ ] `tray_refresh_tick` (app_setup.rs L470-496) 已是先 sleep 后 run，不动
- [ ] UI debounce / toast setTimeout（非轮询）

已知约束：
- [ ] 不改 DB schema；不改外部文件写入路径
- [ ] 保留可见性前台立即刷新语义（前端 listen 仍需 visibilitychange 处理）

## 验收标准
- [ ] backup spawn_scheduler 启动不立即 maybe_backup/cleanup，先 sleep tick 后进入 loop
- [ ] defaults_sync / client_types_sync spawn 后仅 24h loop，不立即 maybe_sync_on_startup
- [ ] scheduled_cleanup loop 改为先 sleep(24h) 后 cleanup
- [ ] 一次性迁移任务（5 处）保留不动
- [ ] proxy_log 终态落库后 emit `proxy_log_changed`；retention 清理后 emit
- [ ] Popover/TrayConfig stats 数据变更后 emit 推送事件
- [ ] 前端 5 处 usePolling 移除，改为 @tauri-apps/api/event listen
- [ ] 保留 visibilitychange 前台立即刷新语义
- [ ] `cargo clippy` 净、`cargo build` 过
- [ ] `tsc --noEmit` 净、`yarn test` 全绿

## 索引
- [ ] 详细设计: [design.md](design.md)
- [ ] 调研收敛: [findings.md](findings.md)
- [ ] 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list startup-no-scheduled`)
