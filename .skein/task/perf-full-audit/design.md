# perf-full-audit — 详细设计

## 上轮复验结论(7 项全保留无回归)
1. STREAM_BODY_MAX_BYTES=16MB ✓ | 2. record_upstream_body 受 log_upstream_request ✓ | 3. snapshot into_snapshot_meta ✓ | 4. emit 仅终态 ✓ | 5. tray debounce 200ms ✓ | 6. cost 仅终态 ✓ | 7. feed_sse 借用化 ✓

## 本轮根因(新发现, 按优先级)

### P0 止血
1. **非流式 body 无 cap + 4× String 分配** — `forward.rs:463 resp.bytes().await` 裸读无上限; `finish.rs:24,27,66,74` + `passthrough.rs:129-139` 多次 `from_utf8_lossy().to_string()` + clone。上轮 fix 只 cap 流式 + gate 流式 record, **非流式分支漏**: body 先分配后 from_log strip, 分配已发生。单 10MB 响应 → 40-50MB 瞬时; N 并发 × 50MB
2. **retention 无自动调度** — `app_setup.rs:206-254` 每日 spawn 含 backup/defaults_sync/notification/purge_soft_deleted/stats_agg, **缺 run_retention_cleanup**。`purge_all_soft_deleted` 走 `deleted_at>0`, 对正常 INSERT 行(deleted_at=0)零作用。长跑 + log_upstream_request 曾开 → response_body 累积(maintenance.rs:219 注释实测 376MB) → SQLite 文件膨胀 → 写放大/页分裂 → CPU 渐升 + RSS 涨

### P1 跨层
3. **proxy-log-updated 8 listener 各 reload** — popover(1000ms) / Home(500ms, 7 并行 invoke) / Stats(2 listener) / platforms usePlatformsState(500ms 整表 list) / Groups(500ms) / Logs(refreshList 500ms + refreshDetail 1000ms)。每终态请求 emit 1 次, 活跃流量下 8 页并发重拉

### P2 Rust
4. **settings 每请求 DB 缓存读** — handler.rs:111 get_log_settings / :150 get_lang / http_client.rs:90 load_proxy_client_settings / forward.rs:261 get_system_timeout / finish.rs:67 get_middleware_settings。每请求 ≥4 次 RwLock read + Value clone + serde_json::from_value

### P3(待 profile, 本轮盲做谨慎)
5. **log_snapshots 单 Mutex** — s1 去 body 后 clone 轻, 但锁全局串行。高并发(≥50 RPS)值得 DashMap
6. **Liquid Glass backdrop-filter 层叠** — globals.css 5 处 backdrop-filter, 82 文件 155 处 glass className。滚动容器内多层 glass-surface 嵌套触发 GPU 合成叠加

## 修复设计(6 subtask)

### s1 P0 非流式 body cap + record gate(Rust 代理热路径)
- `NONSTREAM_BODY_MAX_BYTES = 16MB`(对齐流式); `forward.rs:463` / `passthrough.rs:129` resp.bytes() 前用 `resp.content_length()` 预检 + 超限截断(标 truncation, 同流式 idiom)
- 非流式 record gate: `finish.rs` / `passthrough.rs` 非流式分支 `log.response_body` / `log.user_response_body` 赋值整体 gate 到 `record_upstream_body`(与流式旁路对齐); usage 提取 `extract_usage(&bytes)` 借用不必先 to_string
- 与上轮 fix 对称: 流式 cap + gate 已有, 非流式补齐

### s2 P0 retention 每日调度 + VACUUM(数据层)
- `app_setup.rs:206-254` 每日 spawn 补 `run_retention_cleanup`(复用 commands_proxy/proxy_log.rs:122 逻辑); 启动首跑补「关机错过」
- VACUUM: 后台 spawn(独立 task, 不阻塞主线程), 低频(每日 retention 后, 或启动偶发阈值触发如 db > 100MB); 注 VACUUM 锁库耗时, 大库数秒, 必须后台 + 用户可感知(日志)
- 不改 retention 默认天数(7/7/90)

### s3 P1 前端集中 proxy store(跨层重构)
- 引入集中 store(AppContext 扩展, **不加新依赖 Zustand**): 持 today_stats / platforms / groups / logs 共享数据
- app 根挂单一 `onProxyLogUpdated` listener, 按 payload.platform_id 用 selector 精准刷新对应 slice
- 各页(Home/Stats/Logs/Groups/Platforms/popover)订阅 store, 去各自 listener + reload
- 兼容: 现有页面数据流不改逻辑, 仅数据源从「各自 fetch」改「store 订阅」

### s4 P2 ProxyState settings 缓存(Rust, deps s1)
- ProxyState 持 `Arc<RwLock<ProxyLogSettings>>` + lang + middleware_settings + system_timeout + proxy_client_settings; settings_set command 写时更新
- 请求路径(handler.rs/forward.rs/finish.rs/http_client.rs)直接 read lock 一借, 零 DB 缓存往返
- deps s1: 共享 forward.rs, s1 改完再上

### s5 P3 log_snapshots DashMap(Rust, deps s4)
- `proxy/mod.rs:144` `Mutex<HashMap>` → `DashMap<String, ProxyLogColumns>`; log.rs get/insert/remove 改 DashMap api(entry/shard)
- s1 后 clone 已轻, DashMap 主要降锁竞争; deps s4(mod.rs/log.rs 共享)

### s6 P3 Liquid Glass 滚动容器审计(前端, 独立)
- 审计 globals.css backdrop-filter 层叠 + Logs ListView(4 glass-surface 嵌套) + 其他滚动容器
- 最小改: 滚动容器内避免多层 glass-surface 嵌套; 滚动祖先禁 backdrop-filter(或 will-change 提示)。**不盲改主题架构**, 仅去最显眼的嵌套
- 若审计发现需大改, 落报告交后续 task, 本轮仅最小改

## 数据流(修复后)
```
请求 → ProxyState(settings 缓存, 零 DB 读) → 转发
     → 非流式: body cap 16MB + record gate(流式同款)
     → 终态 upsert + emit proxy-log-updated(单 payload)
     → app 根单 listener → store selector 精准刷新 → 各页订阅更新
     → 每日 spawn: retention cleanup + VACUUM(后台) → 库不胀
```

## 取舍
- **非流式 cap 对称**: 上轮漏, 本轮补, 与流式同 16MB 同 gate, 逻辑对称易维护
- **VACUUM 后台低频**: 锁库耗时 vs 缩库收益, 选每日 retention 后 + 阈值触发, 后台 spawn 不阻塞
- **store 不加 Zustand**: 用 AppContext 扩展现有方案, 零新依赖(ponytail)
- **DashMap 盲做风险**: 静态中置信, 但 ponytail 评估 s1 后仍值得(锁竞争独立于 clone 开销), 本轮做
- **Liquid Glass 最小改**: 静态低置信, 仅去显眼嵌套, 大改落报告

## 回归风险点
- s1 非流式 cap 截断: 超长非流式响应(批量 API)被截 — 已有 truncation 标记, 可接受
- s3 store 重构: 各页数据流逻辑改数据源, 需逐页验证不破坏(Home 7 invoke / Stats / Logs / Groups / Platforms)
- s4 settings 缓存: settings_set 写时更新需覆盖全路径, 否则缓存陈旧
- s5 DashMap: entry/shard api 与 HashMap 不同, 需逐调用点改
- VACUUM: 大库锁库数秒, 需后台 + 不在请求路径
