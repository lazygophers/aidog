# perf-oom-cpu-optimize — 详细设计

## 根因(审计实证, 高置信度)

### OOM 主因
1. **StreamAggregator 累积 512MB/流式请求** — `proxy/stream.rs:5` `STREAM_BODY_MAX_BYTES=512*1024*1024`,`upstream_body: Mutex<Vec<Bytes>>` + `client_body` 累积完整 SSE 字节流至 [DONE]/Drop 才 flush。`record_upstream_body = log_settings.enabled`(master switch 默认开)→ 默认每流式请求累积上游原文 + 客户端 SSE 双份。N 并发 × 512MB。
2. **flush 时 join 翻倍峰值** — `join_stream_body` `Vec::with_capacity(512MB)` + `String::from_utf8_lossy().into_owned()` = 512MB 累积 + 512MB String 拷贝。
3. **log_snapshots HashMap 持完整 body** — `ProxyLogColumns` 含 5 body String,随快照存 in-flight HashMap,叠加效应。

### CPU 主因
1. **emit 风暴** — `upsert_log`(proxy/mod.rs:147 注释: 单请求生命周期调 40+ 次)在 `write_ok` 分支无条件 emit `proxy-log-updated` + `tray-refresh`。tray-refresh listener(app_setup.rs:391-413)trailing debounce 仅 50ms → 每 50ms 完整重建菜单(build_menu + 多次 SQL + set_menu/title/icon 原生调用)。
2. **重复查询** — upsert_log 内 `est_cost==0 && tokens>0` 时每次调 `get_platform(db, platform_id)` + `calc_est_cost`,per request 40+ 次重复 DB 查询(同 platform_id 同 model)。
3. **锁竞争 + MB clone** — 单一 `Mutex<HashMap<String, ProxyLogColumns>>` 所有并发请求共享,每次 upsert 三次拿锁 + 全字段 clone(5 body String, MB 级)。
4. **SSE feed 每 chunk 全 split + String 化** — `feed_sse_usage` 每 chunk `buf.push_str + split('\n').map(to_string).collect<Vec<String>>()`,长流持续小对象分配。

## 修复设计(3 subtask, 串行 — proxy/ 文件高度重叠)

### s1 OOM 止血(stream.rs + log.rs + mod.rs + db/proxy_log.rs)
- `STREAM_BODY_MAX_BYTES`: 512MB → **16MB**(超出截断 + 已有 truncation 标记)
- `record_upstream_body` 默认: master switch(`log_settings.enabled`)→ **`log_upstream_request` 同侧控制**(false 时流式上游不累积;true 时仍受 16MB 上限)
- **snapshot 去 body 字段**: `ProxyLogColumns` 移除 5 body String,仅存 meta(id/status/tokens/model/platform_id/created_at 等数字/小字段)。body 字段每次 UPDATE 直接绑定 ProxyLog 当前值,不参与 diff(`db/proxy_log.rs::changed_since` 同步调整, body 字段视为恒变 / 不比较)
- **flush join 峰值**: join 时若超 16MB 截断(已有逻辑, 上限降后自动生效); 不再 with_capacity(16MB)预分配(按实际 chunk 总和)
- **body 三份拷贝**(handler.rs:200-212): `log.request_body` 改仅 `log_user_request` 开启时克隆; 默认走 `Arc<Bytes>` 共享或借用
- 向后兼容: ProxyLog DB schema 不变(body 列仍在, 照常写入); snapshot 是内存结构, 改不影响落库

### s2 CPU emit 节流(log.rs + handler.rs + app_setup.rs)
- **upsert_log 仅终态 emit**: 加 `is_terminal: bool` 参数(或由调用方判定: 请求结束 / StreamLogGuard.flush / status 终态), 仅终态 upsert 才 emit `proxy-log-updated` + `tray-refresh`。中间 upsert 静默写库
- grep `upsert_log(` 全调用点(proxy/log.rs, stream.rs, finish.rs, handler.rs), 按请求生命周期标终态点
- **tray-refresh debounce**: `app_setup.rs` listener 50ms → **200ms**(平衡响应性与风暴)
- 前端不受影响: Home/Stats/Logs/Groups/Popover 的 proxy-log-updated listener 已 500-1000ms debounce

### s3 CPU 计算优化(handler.rs + log.rs + mod.rs + stream.rs)
- **platform/price 请求级缓存**: `forward_attempt` 选定 route 时算一次 `get_platform` + `calc_est_cost`(ResolvedPrice), 存入 ProxyState / log context; 后续 upsert_log 复用, 不重复查 DB
- `upsert_log` 签名加 `cached_platform: Option<&Platform>` / `cached_price: Option<&ResolvedPrice>` 参数, 命中则跳过 get_platform + calc_est_cost
- **log_snapshots 锁**: s1 已去 body 字段后, snapshot clone 变轻(meta 仅); 锁竞争自然降。若仍瓶颈, 后续 task 再 DashMap(YAGNI 当前)
- **feed_sse 借用化**: `buf.split('\n').map(to_string)` → `buf.lines()`(str::lines 借用迭代); 残行用 byte range 跟踪而非 String 重写。保持 usage 解析正确性

## 数据流(修复后)
```
请求 → forward_attempt(算 platform+price 一次, 存 ProxyState)
     → 转发(流式 chunk 进 StreamAggregator, 上限 16MB, record 受 log_upstream_request)
     → upsert_log(中间态: 静默写库不 emit; 复用 cached platform/price)
     → flush(终态: join≤16MB + emit proxy-log-updated + tray-refresh)
     → tray listener(200ms debounce)→ 重建菜单一次
```

## 取舍
- **snapshot 去 body**: body 一直随 upsert 写入, diff 无意义, 去之降内存 + clone 开销, 风险低
- **emit 仅终态**: 中间态不 emit, tray/前端短暂滞后(≤请求周期), 可接受
- **16MB 上限**: 截断超长 body(已有 truncation 标记), 审计能力略降但换内存安全
- **platform 缓存放 ProxyState**: 请求级生命周期, 无并发一致性问题(同请求同 route)
- **不改 schema / 不加依赖**: 向后兼容优先, 改动面集中 proxy/

## 回归风险点
- `changed_since` 改后(去 body diff), 前端轮询拿到的增量字段需验证仍正确(前端用 meta 字段刷新, body 按需单独查)
- upsert_log 签名改(加 cached 参数)波及全调用点, 需 grep 全覆盖
- feed_sse 借用化保 usage 解析正确性(有 SSE usage 单测覆盖)
