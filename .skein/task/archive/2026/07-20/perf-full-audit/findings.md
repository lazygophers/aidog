# perf-full-audit — 审计收敛

二轮全量静态审计(aidog-perf-audit), 无运行时 profile。结论标代码实证或静态推断。

## 上轮复验(7 项全保留无回归)
| # | 项 | 位置 | 状态 |
|---|---|---|---|
| 1 | STREAM_BODY_MAX_BYTES=16MB | stream.rs:5 | ✓ |
| 2 | record_upstream_body 受 log_upstream_request | finish.rs:186 / passthrough.rs:160,528 | ✓ |
| 3 | snapshot into_snapshot_meta | log.rs:139,147 / db/proxy_log.rs:227 | ✓ |
| 4 | emit 仅终态 is_terminal | log.rs:126,154,163 | ✓ |
| 5 | tray debounce 200ms | app_setup.rs:409 | ✓ |
| 6 | cost 仅终态 status_code!=0 | log.rs:31,46 | ✓ |
| 7 | feed_sse 借用化 | stream.rs:65 | ✓ |

## 本轮新发现

### P0 止血
1. **非流式 body 无 cap + 4× String 分配** — `forward.rs:463 resp.bytes().await` 裸读无上限; `finish.rs:24,27,66,74` + `passthrough.rs:129-139` 多次 `from_utf8_lossy().to_string()` + clone。上轮 fix 只 cap 流式 + gate 流式 record, **非流式分支漏**(body 先分配后 from_log strip)。单 10MB 响应 → 40-50MB 瞬时; N 并发 × 50MB。**HIGH**
2. **retention 无自动调度** — `app_setup.rs:206-254` 每日 spawn 缺 `run_retention_cleanup`; `purge_all_soft_deleted` 走 `deleted_at>0` 对正常 INSERT 行零作用; maintenance.rs:219 注释实测单库 376MB。长跑胀库 → 写放大/页分裂 → CPU 渐升 + RSS 涨。**HIGH**

### P1 跨层
3. **proxy-log-updated 8 listener 各 reload** — popover.tsx:131(1000ms) / Home.tsx:94(500ms, 7 并行 invoke :75-90) / Stats.tsx:179,190(2 listener) / usePlatformsState.ts:720(500ms 整表 list :456) / useGroupData.ts:246(500ms) / useLogsData.ts:174,226(refreshList 500ms + refreshDetail 1000ms)。每终态请求 emit 1 次, 活跃流量下 8 页并发重拉。**HIGH**

### P2 Rust
4. **settings 每请求 DB 缓存读** — handler.rs:111 get_log_settings / :150 get_lang / http_client.rs:90 load_proxy_client_settings / forward.rs:261 get_system_timeout / finish.rs:67 get_middleware_settings。每请求 ≥4 次 RwLock read + Value clone + serde_json::from_value(db/settings.rs:14-21)。**MED-HIGH**

### P3(待 profile, 盲做谨慎)
5. **log_snapshots 单 Mutex** — mod.rs:144 `Mutex<HashMap>`, 每请求 lock 3-4 次(get/insert/remove)。s1 去 body 后 clone 轻, 但锁全局串行。**MED**
6. **Liquid Glass backdrop-filter 层叠** — globals.css 5 处 backdrop-filter, 82 文件 155 处 glass。Logs ListView 外层 4 glass-surface(ListView.tsx:59,163,168,226), 单行 LogRow memo 无 glass。滚动祖先 backdrop-filter 触发 GPU 合成叠加。**LOW-MED**

## 推测触发场景
- 推测: OOM 主因 = 非流式大响应(批量 API / 大上下文 / 错误堆栈)body 累积 + proxy_log response_body 胀库 RSS 涨
- 推测: CPU 主因 = 活跃代理流量下 8 listener 并发 reload + settings 每请求 4 次 DB 缓存读 + 长跑胀库写放大
- 用户确认: 无运行时数据, P0-P3 全扫

## 关键证据文件
- Rust 代理: `gateway/proxy/{forward,finish,passthrough,handler,log,mod}.rs` / `db/{settings,maintenance,proxy_log}.rs`
- 调度: `src-tauri/src/app_setup.rs`
- 前端: `src/services/api/proxy.ts` / `src/popover.tsx` / `src/pages/{Home,Stats,Logs,Groups}.tsx` / `src/pages/platforms/usePlatformsState.ts` / `src/pages/Groups/useGroupData.ts` / `src/pages/Logs/useLogsData.ts` / `src/styles/globals.css`
