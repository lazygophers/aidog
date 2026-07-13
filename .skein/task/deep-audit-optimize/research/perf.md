# aidog 性能审计清单（deep-audit-optimize / perf 维度）

范围：`src-tauri/crates/aidog_core/src/gateway/**` + `src/**/*.{ts,tsx}`，跳过 target/dist/build/node_modules。
仅列发现，未改码。位置均为绝对路径或仓库相对路径 + 行号。

---

### F1: 每请求重建 reqwest::Client，连接池/TLS 复用全失效
- 严重度: high
- 位置: `src-tauri/crates/aidog_core/src/gateway/http_client.rs:33`（`build_http_client`）→ 调用点 `src-tauri/crates/aidog_core/src/gateway/proxy/forward.rs:267`
- 问题: `forward_attempt` 每次尝试都 `build_http_client(&state.db, req_timeout, conn_timeout, Some(&route.platform.extra), None).await`，内部 `reqwest::Client::builder()...build()` 构造全新 client。reqwest 的连接池 / keep-alive / TLS 会话缓存都是 client-scoped，client 用完即弃 = 每个上游请求重新 TCP 握手 + TLS 协商（HTTPS 上游 ~50–200ms/次）。熔断失败重试切候选时同请求内还会建多个 client。`load_proxy_client_settings`（http_client.rs:40）虽走 settings 内存缓存代价小，但 client 构造本身才是大头。
- 修复方向: 按 `(use_proxy: bool, conn_timeout_bucket)` 维度缓存少量共享 `reqwest::Client`（如 `Arc<RwLock<{true: Client, false: Client}>>`，settings 变更时 invalidate）。timeout 是请求级语义，移到 `RequestBuilder::timeout()`（reqwest 支持）而非 client 级，client 即可长驻复用。`platform_extra.proxy_enabled` 仅决定走哪条 use_proxy 分支，不影响 client 复用。
- 预期收益: 高并发下端到端延迟降数十至数百 ms（消除重复 TLS）；socket / 文件描述符消耗显著下降。
- 跨维度: 无

### F2: tray-refresh 每请求多次同步触发主线程菜单重建
- 严重度: high
- 位置: emit `src-tauri/crates/aidog_core/src/gateway/proxy/log.rs:164`；listen `src-tauri/src/app_setup.rs:395`；rebuild `src-tauri/crates/aidog_core/src/tray_render.rs:350`
- 问题: `upsert_log` 写库成功后无条件 `app.emit("tray-refresh", ())`。单请求生命周期内 `upsert_log` 被调 4–6 次（handler.rs 的 Upsert #1/#2/#3 + 每次 forward_attempt + finish / StreamLogGuard::flush），每次都 emit。监听端 `app.listen("tray-refresh", move |_| { let _ = tauri::async_runtime::block_on(refresh_tray_menu(...)); })` —— **无防抖**，且 `block_on` 在监听线程同步等 `refresh_tray_menu`（`build_menu(app).await` + `layout(app).await` 各做一组 today_stats / quota SQL 查询 + `set_menu` + `set_title` / `set_icon`）。流式请求伴随多次中间 upsert，菜单重建在主线程串行排队。
- 修复方向: 监听端加防抖（trailing 200–500ms，复用前端 `onProxyLogUpdated` 同款 setTimeout 合并突发）；或 emit 侧仅终态 emit（`is_terminal` 才发 tray-refresh，中间节点只发 proxy-log-updated）。菜单数据本身是今日聚合值，中间态刷新无意义。
- 预期收益: 主线程 menu rebuild 频次从每请求 4–6 次降到 ≤1 次；高并发 / 长流式下 UI 卡顿消除。
- 跨维度: 无

---

### F3: 上游请求体无条件 pretty 二次序列化
- 严重度: medium
- 位置: `src-tauri/crates/aidog_core/src/gateway/proxy/headers.rs:445`（`format_pretty_json`）→ 调用点 `src-tauri/crates/aidog_core/src/gateway/proxy/forward.rs:291`
- 问题: `forward.rs:258` 刚 `serde_json::to_string(&req_body)` 序列化得到 `req_body_str`，紧接着 `log.upstream_request_body = format_pretty_json(&req_body_str)` 又把它 `from_str` 反序列化回 `Value` 再 `to_string_pretty`。等于对请求体做了 serialize → parse → pretty-serialize 三趟，O(n) × 2。该赋值**无视 `log_settings.log_upstream_request`**（脱敏在后续 `upsert_log::from_log` 才发生），即便用户关了上游日志也在每次 forward_attempt 付全费。长上下文 / 大 tools schema 请求体可达数十 KB–MB。
- 修复方向: `format_pretty_json` 仅在 `log_settings.log_upstream_request == true` 时调用；关时直接赋空串（与 from_log strip 后结果一致）。或保留 `req_body_str`（compact）入库，前端展示侧按需 pretty。
- 预期收益: 大请求体热路径 CPU 降约一倍序列化开销；关上游日志时该路径趋零。
- 跨维度: 无

### F4: upsert_log 内 est_cost / get_platform 重复计算
- 严重度: medium
- 位置: `src-tauri/crates/aidog_core/src/gateway/proxy/log.rs:39-55`（first_agg 块）+ `src-tauri/crates/aidog_core/src/gateway/proxy/log.rs:102-118`（日志列块）
- 问题: 同一次 `upsert_log` 调用里，当 `est_cost == 0` 时两块各自 `super::db::get_platform(&state.db, log.platform_id).await` + `calc_est_cost(...).await`。即同一 (platform_id, model, tokens) 组合在同函数内做两次 DB 读 + 两次价格解析。终态 upsert（first_agg=true 且日志开启）必命中双算。`get_platform` 不在 settings 缓存内，每次走 read pool。
- 修复方向: 函数顶部算一次 `(platform_type, est_cost)`，两块共用；或把 est_cost 计算上移到 caller（forward/handle 已有 platform_type 上下文，直接传入，免掉 get_platform）。
- 预期收益: 终态写库路径 DB 往返减半（省一次 get_platform + 一次 model_price 查询）。
- 跨维度: 无

### F5: 路由层每候选重复 JSON 解析 platform.extra
- 严重度: medium
- 位置: `src-tauri/crates/aidog_core/src/gateway/router/candidates.rs:88`（单平台分支）/ `:257-258`（多平台 map）→ `peak_hours.rs:259 parse_platform_peak_hours` / `peak_hours.rs:208 peak_hours_for` / `time_models::parse_platform_time_models`
- 问题: `resolve_effective_models`（candidates.rs:292）对每个候选依次调 `parse_platform_time_models(&gp.platform.extra)`（解析 extra JSON）+ `default_peak_models` + `peak_hours_for(&platform.extra, &ptype)`（内部 `parse_platform_peak_hours` 再解析同一 extra JSON 一次）。加上 `is_peak_disabled`（candidates.rs:80/125）也调 `peak_hours_for`。同一平台的 `extra` 字符串在一次 select_candidates_ctx 内被 `serde_json::from_str` 解析 2–3 遍。N 平台组 = O(N) 次重复解析。
- 修复方向: 候选循环顶部把每平台的 `(time_rules, peak_windows, disable_during_peak)` 一次性解析进一个临时 struct，后续 `is_peak_disabled` / `resolve_effective_models` 复用；或在 Platform 模型加载时（db::platform）缓存 parse 后结构（lazy / OnceCell）。
- 预期收益: 大分组路由决策 CPU 线性下降；extra 越大收益越明显。
- 跨维度: 无

### F6: 流式 usage 聚合每 chunk 全量 split + Vec<String> 分配
- 严重度: medium
- 位置: `src-tauri/crates/aidog_core/src/gateway/proxy/stream.rs:63-90`（`feed_sse_usage`）
- 问题: 每个 SSE chunk 进来都 `buf.push_str(text)` 把整段 chunk 拷进 `sse_line_buf`，再 `buf.split('\n').map(|s| s.to_string()).collect::<Vec<String>>()` —— 对 chunk 内每一行分配一个新 String + 一个 Vec。这是流式转发热路径（长响应数百 chunk），每 chunk O(line_count) 分配 + 拼接 buf 的 O(n) copy。`sse_line_buf` 还会随 chunk 累积直到遇到换行。
- 修复方向: 用 `str::lines()` 借迭代（零分配）+ 仅对残行做一次 `to_string()`；或直接在 bytes 层面找 `\n` 切片，避免 `push_str` 整段拷。usage 提取本只需找 `data: ` 前缀行，不需要全行 String 化。
- 预期收益: 长流式响应 chunk 处理分配数降一个量级，GC / allocator 压力下降。
- 跨维度: 无

### F7: 请求体 JSON 双重解析
- 严重度: medium
- 位置: `src-tauri/crates/aidog_core/src/gateway/proxy/handler.rs:216`（model 提取）+ `:305`（req_value 全量解析）
- 问题: line 216 `serde_json::from_slice::<Value>(&bytes)` 先整段 parse 一遍只为提 `.model` 字段；line 305 又 `serde_json::from_slice(&bytes)` 得 `req_value` 全量 parse 第二遍。大请求体（长对话 / 大 tools）parse 两遍 O(n)。
- 修复方向: 把 line 216 的提前解析删掉，model 字段从 line 305 的 `req_value` 取（`req_value.get("model")`）；提前 model 仅用于 Upsert #1 的 log.model，可把 Upsert #1 推迟到 parse 之后或临时留空。或仅用轻量 `memchr`/手写扫 `"model"` key 取值，免整段 Value 树。
- 预期收益: 大请求体入站 parse 次数减半。
- 跨维度: 无

---

### F8: ep_proto / Protocol 枚举名小写化反复分配
- 严重度: low
- 位置: `src-tauri/crates/aidog_core/src/gateway/proxy/forward.rs:40`（`ep_proto` 闭包 `format!("{:?}", ep.protocol).to_lowercase()`）→ 调用点 `:56`（UA passthrough `find` 循环内）/ `:92-94`（passthrough 判定）
- 问题: `format!("{:?}", Protocol).to_lowercase()` 每次构造一个 `String`（Debug 格式化 + 小写化两次分配），用于字符串比对协议名。UA passthrough 分支 `route.platform.endpoints.iter().find(|ep| ep_proto(ep) == p)` 对每个 endpoint 都调一次。forward.rs 多处同款 `format!("{:?}", ...).to_lowercase()`（如 handler.rs:403、candidates.rs:302）。
- 修复方向: 给 `Protocol` 加 `fn as_str(&self) -> &'static str`（match 各变体返静态字面量，serde rename 已有裸名表），全代码替 `format!("{:?}", x).to_lowercase()`。
- 预期收益: 热路径 String 分配减少；候选 / 端点扫描常数因子下降。
- 跨维度: 与 audit-code-quality 协同（重复 idiom）

### F9: HeaderMap 每请求两次全量 clone
- 严重度: low
- 位置: `src-tauri/crates/aidog_core/src/gateway/proxy/handler.rs:196`（`orig_headers = req.headers().clone()`）+ `src-tauri/crates/aidog_core/src/gateway/proxy/forward.rs:344`（`upstream_resp_headers = resp.headers().clone()`）+ `forward.rs:281`（`passthrough_convert_headers(orig_headers, ...)`）
- 问题: 整个 `HeaderMap`（含所有 header 值的 `Bytes` clone）在请求开始 clone 一次（orig_headers 贯穿整请求生命周期，含重试所有候选），上游响应头又 clone 一次。重试场景 orig_headers 被 N 次候选 forward_attempt 借用。
- 修复方向: 大多数情况下 orig_headers 用于 read-only 查询（鉴权 / UA / host），可改借引用（生命周期随 `_parts`），仅在真要灌入 passthrough 时 clone。响应头 clone 用于后续 finish_*，可评估只 clone 需要的几个 key。
- 预期收益: 每请求少一次 HeaderMap 深拷；header 多时（Claude Code 带 x-stainless-* / anthropic-* 一堆）节省明显。
- 跨维度: 无

### F10: proxy_log 路径过滤用前缀通配 LIKE → 全表扫
- 严重度: low
- 位置: `src-tauri/crates/aidog_core/src/gateway/db/proxy_log.rs:410`（`AND request_url LIKE ?{idx}` 绑定 `"%{trimmed}%"`）
- 问题: Logs 页「搜索路径」用 `request_url LIKE '%xxx%'`，前缀通配无法走索引，对 90 天 retention 的 proxy_log 全表扫 + ORDER BY created_at DESC + LIMIT。status「失败」过滤是 `status_code < 200 OR status_code >= 300`（OR 范围，index_unfriendly）。虽分页封顶，但大库下 COUNT(*) 同款无索引扫。
- 修复方向: 短期：UI 侧把路径搜索改为前缀匹配（`LIKE 'xxx%'` 可走 `request_url` 索引，需加索引）；或限定时间窗（强制选 time filter）。长期：FTS5 虚表索引 request_url。COUNT 可考虑近似或缓存。
- 预期收益: 大库下 Logs 页过滤响应从秒级回 ms 级。
- 跨维度: 无

### F11: PlatformListView 拖拽态致全部 PlatformCard 重渲染
- 严重度: low
- 位置: `src/pages/platforms/PlatformListView.tsx:133-151`（PlatformCard props 含 `dragActive={!!platDrag}` / `index={i}` / `isDragging`）
- 问题: `PlatformCard` 已 `memo`（PlatformCard.tsx:77），但 `dragActive` 是全局态（任一卡拖拽时所有卡该 prop 翻 true），导致拖拽期间所有 PlatformCard memo 失效全重渲染。`index={i}` 在重排后也会变。卡片本身 914 行、渲染重（quota / usage / badge / form sections），平台数多时拖拽掉帧。
- 修复方向: `dragActive` 仅对正在拖拽的卡有意义，改为只在 `isDragging` 卡传 true（其余卡传 false 常量）；或把 dragActive 上下文经 ref / CSS class 而非 prop 下发，避免 memo 失效。
- 预期收益: 拖拽操作不再触发 N 卡重渲染，帧率稳定。
- 跨维度: 与 audit-code-quality（组件粒度）协同

### F12: 路由层 per-candidate `to_string().trim_matches('"')` 取 protocol 裸名
- 严重度: low
- 位置: `src-tauri/crates/aidog_core/src/gateway/router/candidates.rs:302-305`（resolve_effective_models 内）+ 多处同款
- 问题: 取 protocol serde 裸名（如 "glm_coding"）用 `serde_json::to_string(&platform.platform_type).unwrap_or_default().trim_matches('"').to_string()`，每次 alloc 一个 `String`（serialize 出带引号 JSON 字符串 → trim → 再 collect 成 String）。候选 map 内每候选一次。
- 修复方向: 同 F8，`Protocol::as_str()` 静态切片替代。
- 预期收益: 路由决策少量分配消除；与 F8 合并修一次到位。
- 跨维度: 与 F8 同根因

---

## 未覆盖

- `src-tauri/crates/aidog_core/src/gateway/proxy/connect.rs`（P1 CONNECT 隧道 / MITM CA）39.7K 行未细读 —— 仅在 P1 明文代理场景命中，非默认 AI 转发热路径，按优先级跳过。
- `mitm/ca.rs`（1223 行，证书生成）启动 / 按需路径未深审。
- 前端 bundle 体积 / 启动慢：未跑 `vite build` 分析（仅静态扫描，package.json 无 build-analyzer 脚本）。code split / 动态 import 情况未量化。
- `quota.rs` 余额查询（外部 HTTP）未细审 —— 阈值触发真查，非每请求热路径。
- `import_export/`、`mcp/`、`skills/` 子系统未扫（非热路径）。
