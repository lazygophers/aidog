---
updated: 2026-07-02
rewrite-version: 1
authored-by: trellisx-spec
mode: optimize
---

# Proxy CONNECT 隧道 (HTTP Relay)

何时被读: 改 `src-tauri/src/gateway/proxy/` (尤其 CONNECT handler / 隧道 / proxy_log 写入 / 平台 host 匹配) 时
谁读: main / sub-agent
不遵守的代价: CONNECT 路由失效 / 隧道字节丢失 / stats_agg 污染 / AI 协议路由回归破。P1 (`07-02-07-01-proxy-http-relay-p1`) 实证 + research (`07-01-proxy-http-relay/research/http-relay-research.md`) 预判双错修正沉淀。

---

## CONNECT 路由契约 (MUST)

> 违反代价: `.route()` 注册 CONNECT → authority-form URI `host:port` 经 axum path matcher 不可靠 → 隧道建不起。research 结论 1 被实测否决。

- **禁用 axum `.route()` 注册 CONNECT** — authority-form URI (`CONNECT host:port HTTP/1.1`) 的 path 段是 `host:port`, 经 axum path matcher 路由不可靠
- **改在 `handle_proxy_core` 头部按 `req.method() == Method::CONNECT` 早期分流** — `handler.rs` 第一行 `if req.method() == CONNECT { return connect::handle_connect(...) }`, 非 CONNECT 请求原样 fallthrough 到现有 path 路由
- **early return 不破现有路由** — CONNECT 分流必须是 handle_proxy_core 的 early return; GET `/` `/proxy` `/models` `/v1/models` + POST `/api/*` + AI path (`/proxy/v1/messages` 等) 全部走原显式 `.route()` 或 fallback, 不受 CONNECT 分流影响
- CONNECT 响应: `200 OK + Body::empty()`, **禁 `Connection: upgrade` header** (hyper h1 role.rs 规则, 加了会断隧道)

## hyper-util upgrade downcast 类型 (MUST)

> 违反代价: downcast 类型错 → 取不到底层流 → 隧道空转 / panic。research 说 `downcast::<TcpStream>`, 实测失败。

- **downcast 类型参数 = `TokioIo<TcpStream>` 非 `TcpStream`** — `axum::serve` 喂入的是 `TokioIo<TcpStream>` (impl hyper Read/Write trait), 非 raw `TcpStream` (后者不 impl hyper traits)
- 取回后 `parts.io` 再包一层 `TokioIo::new()` 转 tokio IO, 供 `tokio::io::copy` 使用
- 预读 buf (`parts.read_buf`) **须在双向 copy 前 flush 到上游** — 防 TLS ClientHello 首字节已读入 buf 未转发, 隧道建后客户端握手失败

## TCP 双向隧道 (MUST)

- `tokio::io::copy` 双向 + `tokio::join!` 同时转发两向
- 字节 u64 返回值: P1 决策**不入库** (YAGNI, 用户锁), 仅记 `duration_ms` (`Instant::now` 起止 → `as_millis() as i32`); 未来若加字节统计须 migration 加列
- 上游 TCP 连接失败 → 响应 `502` + 落 proxy_log (status=502); 客户端升级前断开 → `499` + 落 log

## proxy_log 写入契约 (MUST — 不污染 stats_agg)

> 违反代价: CONNECT 流量走 `upsert_log` → 触发 `upsert_stats_agg` + `agg_mark_first` → 统计页虚高 (隧道流量非 AI 请求, 不该进 AI 统计)。

- **`upsert_connect_log` 独立路径** — 直接走 `insert_proxy_log_columns`, **禁调 `upsert_log`** (后者含 stats_agg 聚合); grep `upsert_connect_log` 函数体必须 0 处 `upsert_log` / `agg_mark_first` / `upsert_stats_agg`
- 字段: `source_protocol="http-connect"` / `target_protocol="http-connect"` / `tokens=0` / `cost=0` (P1 不解析 body) / `request_url=<CONNECT host:port>` / `platform_id` (host 命中→平台 id, 否则 0) / `group_key` (命中→关联分组, 否则 '') / `duration_ms` / `status_code`
- **schema 列名 `group_key`** (非 `group_name`) — Migration 010 `RENAME COLUMN group_name TO group_key` (`schema_late.rs:120`); `PROXY_LOG_COLUMNS` + struct 全用 `group_key`

## 平台 host 匹配 (MUST)

- `match_platform_by_host` (新增, `endpoint.rs`) — CONNECT target host 段比对平台 base_url host (复用 `endpoint_host()`)
- 命中 `status != Disabled` 的平台 (Enabled + AutoDisabled) → 返 platform_id + 关联 group_key; 未命中 → None → 调用方写 platform_id=0, group_key=''
- O(n) 全平台扫描, 平台数小可接受; 未来平台数大可加 host→platform_id 缓存

## 前端筛选 sentinel (MUST)

- Logs/Stats 平台筛选「无平台」: value `"0"` → `Number("0")=0` → `platform_id=0` (truthy 透传)
- 分组筛选「无分组」: sentinel `__none__` → 前端映射空串 → `group_key=''` (避与「全部」`""` 撞; FilterDropdown 用严格 `value === o.value` 字符串相等)
- Stats 后端 `Option<String>`: `filter_group === SENTINEL ? "" : filter_group` → `Some("")` → SQL `group_key = ''`

## 验证

```bash
# CONNECT 分流 early return, 非 CONNECT 原 fallthrough
grep -n "Method::CONNECT" src-tauri/src/gateway/proxy/handler.rs  # handle_proxy_core 头部

# CONNECT 响应禁 Connection: upgrade
grep -n "upgrade" src-tauri/src/gateway/proxy/connect.rs  # 0 处 connection upgrade header

# downcast 类型 TokioIo<TcpStream>
grep -n "TokioIo" src-tauri/src/gateway/proxy/connect.rs  # downcast + 再包

# upsert_connect_log 不污染 stats_agg
grep -n "upsert_connect_log" src-tauri/src/gateway/proxy/log.rs  # 函数体 grep upsert_log/agg_mark_first/upsert_stats_agg = 0

# schema 列名 group_key
grep -n "group_key" src-tauri/src/gateway/db/proxy_log.rs  # PROXY_LOG_COLUMNS + struct
```
