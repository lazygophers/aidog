# PRD — P1 CONNECT 隧道 + 元数据 + 无平台筛选

> parent: 07-01-proxy-http-relay。依赖 P0(07-02-stats-logs-filter-unify 已完成 FilterDropdown)。P2 MITM 解密另起 child。

## 目标
aidog 作为标准 HTTP 代理工作: 客户端配 `http_proxy=127.0.0.1:<port>` 后任意 HTTP/HTTPS 流量经 CONNECT 隧道转发, 记 proxy_log 元数据, Logs/Stats 可按「无平台」「无分组」筛选。

## 决策(已锁)
| 维度 | 决策 | 据 |
|---|---|---|
| 形态 | HTTP CONNECT 隧道(标准 http_proxy) | 用户锁(parent PRD) |
| HTTPS | P1 不解密(纯 TCP 隧道盲转) | P2 才 MITM |
| 字节统计 | **放弃**(P1 不记 bytes_up/down) | 用户 2026-07-02 锁 YAGNI; 免 migration, 只记 duration/status/host |
| schema 列名 | **`group_key`**(非 group_name) | Migration 010 `RENAME COLUMN group_name TO group_key`(schema_late.rs:120); PROXY_LOG_COLUMNS + struct 全用 group_key |
| 平台匹配 | CONNECT target host 比对平台 base_url host | 复用 endpoint_host() + 新增 match_platform_by_host |
| 计费 | cost/tokens = 0(P1 不解析 body) | P1 不解密 |

## 已知(codebase + research 实证)
- **axum 0.8 CONNECT**: `axum::serve` 底层 hyper_util auto + upgrades; 关键坑: hyper-util 私有 `Rewind<T>` 包 TcpStream, 需 `downcast::<TcpStream>` 取回; CONNECT 响应 `200 + 空 body`, 禁 `Connection: upgrade` header(research http-relay-research.md)
- **TCP 双向**: `tokio::io::copy`(返字节 u64, P1 不入库) + `tokio::join!`
- **平台 host 匹配**: `endpoint_host()`(endpoint.rs:72-92) + 新增 `match_platform_by_host`; P1 只 host 匹配(无 apikey, HTTPS 未解密)
- **proxy_log 写入**: 新增 `upsert_connect_log`(**不走 upsert_log 避免污染 stats_agg**), 底层 `insert_proxy_log_columns`(db/proxy_log.rs:216); 列名用 group_key
- **proxy.rs**: Axum 代理服务器, 现有路由注册点; CONNECT handler 在此或 proxy/mod.rs Router 注册(技术路径 implement 时定, research 结论 1)
- **P0 FilterDropdown**: 已完成(07-02-stats-logs-filter-unify), Logs/Stats 筛选组件已抽公共, P1 在此加「无平台」「无分组」选项
- **research**: `.trellis/tasks/07-01-proxy-http-relay/research/http-relay-research.md`(26.5K, 含 axum CONNECT 坑 + tokio 双向 + 平台匹配路径)

## 交付
1. **axum CONNECT handler**(`src-tauri/src/gateway/proxy.rs` 或 `proxy/mod.rs`) — Router 注册 CONNECT method 或连接层早期 method 分流(research 结论 1 定路径); CONNECT 响应 `200 + 空 body`(禁 Connection: upgrade); hyper-util `Rewind<T>` downcast::<TcpStream> 取回底层流
2. **TCP 隧道双向转发** — `tokio::io::copy` 双向 + `tokio::join!`; 计 duration_ms(Instant::now 起止); 字节 u64 返回但 **P1 不入库**(用户锁放弃)
3. **`match_platform_by_host`**(`src-tauri/src/gateway/router/mod.rs` 或 endpoint.rs) — CONNECT target host 段比对平台 base_url host(复用 endpoint_host()); 命中→返 platform_id + 关联 group_key; 未命中→platform_id=0, group_key=''
4. **`upsert_connect_log`**(`src-tauri/src/gateway/db/proxy_log.rs`) — 新增, 不走 upsert_log(避免污染 stats_agg); 底层 insert_proxy_log_columns; 字段: `group_key`(命中分组 else '')/`source_protocol="http-connect"`/`target_protocol="http-connect"`/`platform_id`(命中 else 0)/`request_url=<CONNECT host:port>`/`status_code`(隧道建立 200/失败)/`duration_ms`/`input_tokens=0`/`output_tokens=0`/`est_cost=0`/`is_stream=0`/`model=""`/`actual_model=""`
5. **前端 Logs/Stats 筛选加选项**(P0 FilterDropdown) — 「无平台」(platform_id=0) + 「无分组」(group_key='') 两个选项加入平台/分组筛选下拉
6. **i18n** — `logs.noPlatform` + `logs.noGroup` + `stats.noPlatform` + `stats.noGroup`(或复用现有 key 若 P0 已建), 8 locale 全补

## 验收
- 配 `http_proxy=127.0.0.1:<port>` 指向 aidog, `curl http://example.com` → 隧道通 + proxy_log 落一条 source_protocol=http-connect
- `curl https://example.com` → 隧道通(P1 不解密, body 空) + proxy_log 落元数据
- 目标 host 命中某平台 base_url host → proxy_log platform_id 关联该平台 + group_key 关联分组
- 未命中 → platform_id=0, group_key=''
- Logs 页选「无平台」→ 只看 platform_id=0 的隧道请求; 选「无分组」→ group_key=''
- `cargo clippy` + `cargo test`(proxy/router/db 相关) + `yarn build` + `check-i18n` 全绿

## 非目标(YAGNI)
- 字节统计(用户锁放弃, P2 再评估)
- HTTPS 解密(P2 MITM)
- SOCKS5(只 HTTP CONNECT)
- 非 80/443 特殊处理(隧道盲转)
- WebSocket over CONNECT 专项(MVP 盲转)
- 无平台请求计费(cost=0)

## 调度
- write-files: `src-tauri/src/gateway/proxy.rs`(或 proxy/mod.rs CONNECT handler) + `src-tauri/src/gateway/router/mod.rs`(match_platform_by_host) + `src-tauri/src/gateway/db/proxy_log.rs`(upsert_connect_log) + `src/pages/Logs.tsx` + `src/pages/Stats.tsx`(筛选选项, 复用 P0 FilterDropdown) + `src/locales/*.json`
- 依赖 P0(FilterDropdown 已完成) → 可 start
- 与 logs-detail-copy-buttons 同改 Logs.tsx 但区域不同(筛选 vs 详情), 注意合并

## 风险
- axum 0.8 CONNECT 实现路径(research 结论 1 的 Rewind<T> downcast 坑) — implement 时 cargo doc + 实测验证
- CONNECT handler 与现有 AI 协议路由(/proxy path 分流)的共存 — 早期 method 分流不能破现有 path 路由
- match_platform_by_host 性能(每 CONNECT 查全平台 host 集合) — 平台数小, O(n) 可接受
- proxy_log 不污染 stats_agg(upsert_connect_log 独立路径, 禁走 upsert_log)
