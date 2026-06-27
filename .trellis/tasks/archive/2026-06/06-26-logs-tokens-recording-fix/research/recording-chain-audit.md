# Research: Logs 页 tokens/cost/条目/聚合/字段 记录链审计

- **Query**: 审计 Logs 页四类缺陷（① tokens/cost 记 0/偏低 ② 条目缺失 ③ 聚合数值不对 ④ 字段显示错乱）真实根因
- **Scope**: internal（src-tauri/src/gateway/** + src/pages/Logs.tsx）+ DB 实证（~/.aidog/aidog.db）
- **Date**: 2026-06-26

## 代码结构现状（与任务描述偏差）

任务引用的单文件已重构为模块目录，审计按下表实际定位：

| 旧引用 | 现实路径 |
|---|---|
| `gateway/proxy.rs` | `gateway/proxy/`：`handler.rs`(入口) / `forward.rs`(单候选转发) / `finish.rs`(usage 提取+响应转换) / `stream.rs`(StreamAggregator+guard) / `log.rs`(upsert_log+聚合写) / `count_tokens.rs`(count_tokens 子端点) |
| `gateway/db.rs` | `gateway/db/`：`proxy_log.rs`(CRUD+filter) / `stats_agg.rs`(物化聚合写) / `usage_stats.rs`(读聚合) / `query_stats.rs` |
| `gateway/adapter/converter.rs` | `gateway/adapter/converter/` + `adapter/openai/`(request.rs/response.rs/sse.rs) |

记录链：`handler` → 渐进式 `upsert_log`（每阶段写）→ `forward_attempt` → `finish_nonstream`(extract_usage) / `finish_stream`(StreamAggregator + StreamLogGuard.flush)。
聚合：`upsert_log` 内 `first_agg` gate → `upsert_stats_agg` 写 `stats_agg_hourly`；读统计走 `usage_stats.rs`。
`proxy_log` 主键列名是 **`id`**（== request_id，完整 32-hex）。

---

## DB 实证（已取证，~/.aidog/aidog.db 1.09GB）

三个实证 request_id 查到：

| id | src→tgt | is_stream | status | upstream_status | in | out | cache | est_cost | model→actual |
|---|---|---|---|---|---|---|---|---|---|
| `ac90...` | anthropic→anthropic | 0 | 200 | **0** | 47997 | **0** | 0 | **0.143991** | claude-opus-4-8→MiniMax-M3 |
| `33ac...` | anthropic→anthropic | 1 | 200 | 200 | 224 | 478 | 113536 | 0.002106 | claude-opus-4-8→mimo-v2.5-pro |
| `ec3a...` | anthropic→anthropic | 1 | **400** | **400** | 0 | 0 | 0 | 0 | claude-opus-4-8→claude-opus-4-8 |

**三 id 都不是 openai 协议** → 原先推测的 P0(openai stream_options 缺失) 对这两条不适用。真正暴露的问题：

- **`ac90` = `/v1/messages/count_tokens?beta=true` 子端点**。`upstream_request_url=https://api.minimaxi.com/anthropic/v1/messages/count_tokens`，上游连接失败（`upstream_status_code=0`），走本地估算兜底，`response_body="upstream error (local estimate fallback): error sending request for url (...count_tokens)"`。
  - `output_tokens=0`（count_tokens 只算 input，语义正常）；`input_tokens=47997`（本地估算）；但 **`est_cost=0.143991` 是 bug** —— count_tokens 是纯计数调用、不发生推理，不该计费。47997 × opus input 价 ≈ 0.14。
- **`33ac` = 正常 anthropic 流式（xiaomi mimo）**。`cache_tokens=113536` 是 anthropic prompt caching 正常值，tokens/cost 都正确。**此 id 无 bug**。
- **`ec3a` = 上游 400 失败请求（xiaomi mimo sgp）**。anthropic→anthropic 流式，`status=400 / upstream_status=400`，tokens/cost 全 0。`response_body` + `attempts` 显示上游报 `{"error":{"code":"400","message":"Param Incorrect","param":"Not supported model claude-opus-4-8"}}` —— **平台不认 `claude-opus-4-8` 模型名，且未做模型映射（`actual_model==model==claude-opus-4-8`，未改写）就直发**。
  - tokens/cost=0 **语义正常**（400 失败请求无 usage、不该计费，记录链正确）。**此 id 无记录链 bug** —— 是模型映射 / 上游模型兼容问题，不属本次四类记录症状（① token=0 是"请求本就失败"非"漏记 usage"）。
  - 旁证症状④线索：Logs 列 model 与 actual_model 都显示 `claude-opus-4-8`，用户若期望看到映射后的真实平台模型名会觉得"没映射/显示原样"，但这是路由层未配置映射，非记录/显示 bug。

---

## 症状①：tokens / cost 记成 0 或偏低 —— 真因是 count_tokens 计费污染

### 根因 1-A：count_tokens 子端点误计 est_cost（真 bug，高置信，4204 条实证）★ P0

`src-tauri/src/gateway/proxy/count_tokens.rs:84,194`（落 input_tokens）→ `upsert_log` → `log.rs:31,90`（cost gate 仅看 token>0，不排除 count_tokens）

```rust
// count_tokens.rs:84 / :194 —— 把估算/上游返回的 input_tokens 写进 log
log.input_tokens = est_tokens as i32;
// count_tokens.rs:15 注释自相矛盾：「count_tokens 仅用于客户端预估，不参与计费」
```
```rust
// log.rs:31 & :90 —— cost gate 无 count_tokens 排除
if est_cost == 0.0 && (log.input_tokens > 0 || log.output_tokens > 0) {
    est_cost = calc_est_cost(...);   // ← count_tokens 也会被计费
}
```

- **机理**：count_tokens 落了 `input_tokens` 进 proxy_log，`upsert_log` 的 cost gate 只判 `input>0||output>0` → 对纯计数调用算 est_cost。注释声称"不参与计费"是假——实际计了，且同时进 `stats_agg`（`first_agg` gate, log.rs:26-28 也不排除 count_tokens）→ Stats 页/托盘成本虚高。
- **全库污染规模（实测）**：
  ```
  SELECT count(*), sum(est_cost), sum(input_tokens), sum(output_tokens)
    FROM proxy_log WHERE request_url LIKE '%count_tokens%' AND deleted_at=0;
  → 4204 条 | est_cost $195.89 | input 65.3M | output 0
  ```
  全库终态总 est_cost = **$1115.75**，count_tokens 误记约 **$195.89 ≈ 17.6%**。
- **修复方向**：count_tokens 路径落库时 est_cost 强制 0（或不落 input_tokens 进计费列 / 加 is_count_tokens 标记列，统计与 cost 都排除）。注意同时修 proxy_log 列与 stats_agg 聚合两条路径。
- **置信度**：高。**真 bug，4204 条实证，症状①"cost 偏高/错记"头号真因**。

### 候选根因 1-B：OpenAI 出站流式未注入 `stream_options.include_usage`（待证候选，中置信）

`src-tauri/src/gateway/adapter/openai/request.rs:152-162` + `openai/mod.rs:15-30`

```rust
OpenAIRequest { model, messages, max_tokens, ..., stream: req.stream, tools, tool_choice }
// 结构体无 stream_options 字段；全仓 grep "stream_options|include_usage" 主 chat 路径零命中
```

- **机理**：OpenAI Chat Completions 流式默认不返 usage，须请求带 `stream_options:{include_usage:true}` 才回。出站 `to_openai` 不注入 → 上游流式不回 usage → token=0 → cost=0。
- **影响面**：anthropic/openai 客户端 → **openai 协议平台 + 流式**。两个实证 id 都不是这条（都是 anthropic→anthropic），但库里 openai-family 流式仍可能受影响。**需补查实证**：`SELECT count(*) FROM proxy_log WHERE target_protocol='openai' AND is_stream=1 AND status_code=200 AND input_tokens=0 AND deleted_at=0;`
- **置信度**：中（代码确证缺失，但缺命中实证；非这两 id 的成因）。**真 bug 但优先级低于 1-A**。

### 候选根因 1-C/D：流式 usage 提取链（历史已修，确认未复发）

| 历史记忆 | 现行代码 | 结论 |
|---|---|---|
| ① fetch_max 不能 store | `accumulate_sse_usage` stream.rs:274/281/294 全 `fetch_max` | 已修，未复发 |
| ② is_stream 并入上游 content-type | `resolve_is_stream` stream.rs:302-304 + forward.rs:327-332 | 已修，未复发 |
| ③ 跨 chunk 重组 SSE 行 | `StreamAggregator.sse_line_buf` + feed_sse_usage 残行保留(stream.rs:45,63-89) | 已修，未复发 |
| ④ est_cost 走 resolve_price 回退链 | `calc_est_cost` log.rs:39/100 | 已修，符合 |

非流式 usage 提取（`extract_usage` finish.rs:25 + stream.rs:306-332）在协议转换前对上游原始 body 提取，字段映射完整，非根因。

---

## 症状②：日志条目缺失 / 异常 —— 765 条 status=0 卡死行

### 根因 2-A：status_code=0 的请求落库但永不达终态（真现象，高置信，765 条实证）★ P1

`src-tauri/src/gateway/proxy/log.rs:26`（终态 gate `status_code != 0`）

```
SELECT status_code, count(*) FROM proxy_log WHERE deleted_at=0 GROUP BY status_code;
→ 200:81581 | 429:2151 | 502:1760 | 400:790 | 0:765 | 404:268 | 529:200 | ...
```

- **765 条 `status_code=0`**：upsert 写了占位/中间节点但**从未收到终态 HTTP 状态**（连接中断 / 客户端断开 / 上游无响应 / 流式占位后崩溃）。这些行 `est_cost=0`、**不进 stats_agg**（终态 gate `status_code != 0` 挡住，已验：status=0 行 est_cost 合计 0.0）。
- 画像：350 条是 `/v1/messages?beta=true` 流式（占位 `[stream]` 后未 flush 终态），415 条混合端点。
- **现象**：Logs 页这些行 status 列显示 0/空，用户感知"条目异常/缺失（无状态无 tokens）"。**这是症状② 的库内真实形态**，不是行丢失而是行卡在非终态。
- **修复方向**：(1) 请求结束（含错误/断连）路径补写终态 status（如客户端断开记 499/0→显式标记）；(2) 前端 Logs 对 status=0 给"未完成/中断"语义标签而非空白；(3) 排查流式占位 `[stream]` 后 guard 未 flush 的分支（StreamLogGuard Drop 兜底是否覆盖连接中断）。
- **置信度**：高（765 条实证）。**真现象，部分真 bug（终态未补写）+ 部分前端展示语义缺失**。

### 候选根因 2-B：日志主开关 OFF → proxy_log 零写（配置态）

`log.rs:81-83`：`if !settings.enabled { return; }`。主开关 OFF 时 proxy_log 一行不写，但聚合（line 26-79）早于此 return 仍写 stats_agg → "Logs 空但 Stats 有数"。**需用户确认是否关了主开关**。非 bug 但产品语义可议（master OFF 连脱敏元数据都不留）。

### 候选根因 2-C：请求体>10MB 落 400（边缘）/ 健康端点不落库（设计）

`handler.rs:103` `to_bytes(body, 10MB)` 超限落 400（仍落库非缺失）。`GET / + /proxy` 健康探测设计上不落库。早期失败路径（handler.rs 各早退分支）均带 upsert_log，失败请求落库完整。

---

## 症状③：统计 / 聚合数值不对 —— 实测对账基本一致，无明显 bug

### 对账实证（关键，否定 8x 虚高复发）

```
request_count:  stats_agg sum=87223  vs  proxy_log(status!=0)=87040   差 183 (0.2%)
est_cost:       stats_agg sum=$1115.75  ==  proxy_log(status!=0)=$1115.75   完全一致
```

- **est_cost 聚合精确对账，无虚高**。历史 8x 虚高（[[group-stats-aggregation]] / stats_agg.rs:239-241 注释）确已修。
- request_count 多 183（0.2%）：极小，疑似 [[mod.rs:144-161]] `agg_done` 纯内存态在**进程重启/极端高并发 FIFO 淘汰**下的偶发重复计数（`AGG_DEDUP_CAP=8192`）。影响可忽略，非用户日常症状主因。

### 候选根因 3-A：agg 去重纯内存 FIFO（残留风险，低置信）

`proxy/mod.rs:144-161`（agg_mark_first）+ `log.rs:26-28`。`agg_done` 重启丢失；理论上同请求终态调用间隔涌入 >8192 其他请求会重复计数。对账差 183/87040 印证残留极小。**主 bug 已修**。

### 候选根因 3-B：bucket 时区 localtime（历史记忆⑤，已修，残留待验）

`stats_agg.rs:185` 增量 `strftime('...','localtime')` + `:32/64` 回填 `utc_ms_to_local_hour_key`。两条路径都带 localtime（已修）。**残留隐患**：增量用 SQLite `strftime('localtime')`（进程 TZ），回填用 Rust chrono local，DST 边界/异 TZ 下若算出不同小时桶 → rebuild 时桶键不一致致重复。**待核对二者 DST 一致性**（无需用户）。

### 候选根因 3-C：聚合 model 维度 actual_model 优先（写读一致，非 bug）

`stats_agg.rs:38` / `log.rs:51` `CASE WHEN actual_model!='' THEN actual_model ELSE model`，读 `usage_stats.rs` 同维度 GROUP BY。一致。

---

## 症状④：字段显示错乱 —— 前端列表无错位，疑点是空 model 行 + path idx latent bug

### 根因 4-A：1334 行 model 与 actual_model 双空（展示空白，中置信）

```
status=200 共 81584 行，其中 model='' AND actual_model='' 的 1334 行 (1.6%)
```

- 这些行 Logs 列 model/actualModel 显示空白（前端 `log.actual_model || "-"` 但 model 列直读空字符串）。多为非标准端点（embeddings/models/health）或异常请求。
- **用户感知"字段显示不全/错乱"的最可能线索**。需 pinpoint 用户截图确认是否指这批空行（参考 [[ui-feedback-pinpoint-element]] 禁凭泛词猜）。
- **修复方向**：前端空 model 给占位符；后端排查为何这批落库时 model 列为空。

### 候选根因 4-B：`build_filter_where` path 过滤漏 `idx += 1`（latent bug，未修）

`src-tauri/src/gateway/db/proxy_log.rs:396-402`

```rust
if let Some(ref v) = filter.path {
    if !trimmed.is_empty() {
        parts.push(format!("AND request_url LIKE ?{idx}"));
        p.push(Box::new(format!("%{}%", trimmed)));
        // ← 无 idx += 1
    }
}
```

命中 [[logs-path-search-idx-bug]]。path 是最后分支，单独用不报错，但若后续加绑定参数会错位。**真 bug 但 latent，非当前显示错乱直接成因**。修复=补 `idx += 1`。

### 候选根因 4-C：前端列表字段映射本身（确认对齐，非错乱）

`src/pages/Logs.tsx:550-561`(表头 10 列) vs `616-662`(LogRow 10 TdCell) 一一对应（time/group/platform/model/actualModel/status/duration/in/out/操作）。snake_case 直读，类型同名。列表**不显示 cache_tokens/est_cost**（仅详情面板，est_cost 在详情是否展示待复核 Logs.tsx 详情段）。**列结构无错位**。

---

## 5 条历史记忆逐条对照结论

| # | 记忆 | 结论 |
|---|---|---|
| 1 | 流式 usage fetch_max 不能 store | **已修，未复发**（stream.rs:274/281/294） |
| 2 | is_stream 并入上游 content-type | **已修，未复发**（resolve_is_stream stream.rs:302） |
| 3 | 跨 chunk 重组 SSE 行 sse_line_buf | **已修，未复发**（stream.rs:45,63-89） |
| 4 | est_cost 走 resolve_price 回退链 | **已修，符合**（log.rs:39/100），但 count_tokens 不该走（见 1-A） |
| 5 | Stats eff_pid + bucket localtime | **已修**（strftime localtime + utc_ms_to_local_hour_key）；est_cost 对账精确印证；残留 SQLite-vs-Rust DST 一致性待验(3-B) |

---

## 按优先级排序的待修清单

| 优先级 | 症状 | 根因 | file:line | 类型 | 置信度 | 实证 |
|---|---|---|---|---|---|---|
| **P0** | ① cost 错记/虚高 | **1-A count_tokens 误计 est_cost（proxy_log + stats_agg 两路）** | count_tokens.rs:84,194 + log.rs:26-28,31,90 | 真 bug | 高 | **4204 条 / $195.89 / 17.6%** |
| **P1** | ② 条目异常 | **2-A status=0 卡死行未补终态 + 前端无语义** | log.rs:26（终态 gate）+ StreamLogGuard | 真 bug + 展示 | 高 | **765 条** |
| **P2** | ④ 显示空白 | 4-A 空 model 行展示空白 | Logs.tsx 列渲染 + 后端落库 model 列 | 展示 + 待查 | 中 | **1334 条 (1.6%)** |
| **P3** | ① token=0 | 1-B openai 流式缺 stream_options | adapter/openai/request.rs:152-162 + mod.rs:15-30 | 真 bug | 中 | 待补查命中数 |
| **P4** | ④ latent | 4-B path 过滤漏 `idx += 1` | db/proxy_log.rs:396-402 | 真 bug(latent) | 高 | 代码确证 |
| P5 | ③ 聚合 | 3-B SQLite-localtime vs Rust-local DST 一致性 | stats_agg.rs:185 vs utc_ms_to_local_hour_key | 需验证 | 低 | 对账已基本一致 |
| P6 | ② 配置 | 2-B 日志主开关 OFF | log.rs:81-83 | 配置/产品语义 | 中 | 需用户确认 |

**注**：原推测的「openai stream_options 缺失」从 P0 降为 P3 —— 两个实证 id 均为 anthropic 协议，count_tokens 计费污染才是有 4204 条实锤的头号 cost bug。聚合数值（症状③）经 est_cost 精确对账（agg==proxy_log），实测**无明显 bug**。

## Caveats / 需补充

1. **需要: 用户确认 count_tokens 是否应完全不计 cost / 不计入 stats**（1-A 修复语义确认，预计是）。
2. **需要: 用户提供"字段显示错乱"具体截图** —— 代码层列结构对齐，最可能是 4-A 的 1334 条空 model 行，需 pinpoint。
3. **需要: 用户确认是否关了日志主开关**（2-B）。
4. **待研究者后续核对（无需用户）**: 1-B openai 流式 token=0 命中数实证查询；status=0 的 765 条是否含流式 guard 未 flush 的真 bug（StreamLogGuard Drop 是否覆盖客户端断连）；utc_ms_to_local_hour_key 与 SQLite localtime DST 一致性；Logs.tsx 详情面板是否展示 est_cost。
