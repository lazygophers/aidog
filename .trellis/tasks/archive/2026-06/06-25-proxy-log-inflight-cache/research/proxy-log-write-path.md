# Research: proxy_log 写入/更新路径 — 用户「in-flight 内存缓存消除 SELECT」提案可行性评估

- **Query**: 评估「proxy_log 缓存 5 分钟内未完成请求，首次 INSERT + 后续 UPDATE，零 SELECT」提案
- **Scope**: 内部代码只读 (Rust backend)
- **Date**: 2026-06-25

---

## TL;DR（结论先行）

**用户提案 = 已实现**。当前 proxy 热路径的 `upsert_log`（`proxy/log.rs:118-140`）已经做到了：
- 首次 INSERT（`insert_proxy_log_columns`，`db/proxy_log.rs:205-225`）
- 内存快照存 row（`ProxyState.log_snapshots`，`proxy/mod.rs:129`）
- 后续更新只发 `UPDATE`（`update_proxy_log_columns`，`db/proxy_log.rs:231-259`），**且 UPDATE 前对内存快照 diff，无变化直接 no-op，连 SQL 都不发**
- **完全不 SELECT proxy_log 读旧值**（旧值从内存快照拿）

这一机制由 memory [[perf-hotpath-optimization]] 记录，对应代码 `06-14-deep-perf-optimization` 任务的产出。提案描述的能力**已存在**，新做收益 = 0。

唯一仍带 SELECT 风险的是 **3 处非热路径一次性写**（`upsert_proxy_log`，全列 INSERT OR REPLACE），它们不读旧值，本质是单次写入，无优化空间。

---

## 1. 当前 proxy_log 写入链路全貌

### 1.1 热路径写入（代理请求渐进式日志）

所有代理请求经 `proxy/log.rs::upsert_log` 写库（每请求生命周期被调 N 次：建行 + 多次部分更新 + 流式 flush）：

| 节点 | 触发时机 | 函数 | SQL 类型 | 是否 SELECT |
|---|---|---|---|---|
| 首节点 | 请求开始（首次有任意可写信息） | `insert_proxy_log_columns` (`db/proxy_log.rs:205`) | `INSERT INTO proxy_log (...)` | 否 |
| 后续节点 | 请求进行中（每次 `upsert_log` 被调） | `update_proxy_log_columns` (`db/proxy_log.rs:231`) | `UPDATE proxy_log SET <变化列> WHERE id=?` | 否（对内存快照 diff） |
| 终态 | `status_code != 0 && response_body != "[stream]"` (`proxy/log.rs:116`) | 同上（走 UPDATE 分支） + `remove_log_snapshot` | UPDATE + 内存 remove | 否 |
| 流式终态 | `[DONE]` / `message_stop` / Drop 兜底 (`proxy/stream.rs:161-237`) | `upsert_log` + `remove_log_snapshot` | UPDATE | 否 |

**关键源码（`proxy/log.rs:118-140`）**：

```rust
// 取上一快照决定 INSERT(首节点) 还是 部分列 UPDATE(后续节点)。
let prev = {
    let map = state.log_snapshots.lock().unwrap();
    map.get(&id).cloned()           // ← 内存拿，不 SELECT
};
let write_ok = match prev {
    None => {
        // 首节点：建行。成功后存快照供后续 diff。
        let ok = super::db::insert_proxy_log_columns(&state.db, cols.clone()).await.is_ok();
        if ok {
            state.log_snapshots.lock().unwrap().insert(id.clone(), cols);
        }
        ok
    }
    Some(prev) => {
        // 后续节点：仅 UPDATE 变化列；成功后刷新快照。
        let ok = super::db::update_proxy_log_columns(&state.db, cols.clone(), &prev).await.is_ok();
        if ok {
            state.log_snapshots.lock().unwrap().insert(id.clone(), cols);
        }
        ok
    }
};
// 终态写完移除快照，防 in-flight map 无限增长
if is_terminal {
    remove_log_snapshot(state, &id);
}
```

### 1.2 diff-UPDATE 机制（`db/proxy_log.rs:159-201`）

`ProxyLogColumns::changed_since(&self, old)` 对 32 列逐字段 `if self.x != old.x { push }`，`update_proxy_log_columns` 只把变化列拼成 `UPDATE ... SET col1=?,col2=? WHERE id=?`。**若零列变化，函数早退 `return Ok(())`，连 SQL 都不发**（`proxy_log.rs:235-237`）。

### 1.3 非热路径写入（保留旧全列 REPLACE）

`upsert_proxy_log`（`db/proxy_log.rs:50-69`）是老的 `INSERT OR REPLACE` 全列写，不持快照、每写整行重写。仍在用的调用方：

| 调用点 | 文件:line | 用途 |
|---|---|---|
| model_fetch error log | `commands/model_fetch.rs:88` | fetch-models 上游 502 失败日志（一次性） |
| model_fetch success log | `commands/model_fetch.rs:100` | fetch-models 响应日志（一次性） |
| quota fetch log | `gateway/quota/http.rs:208` | quota 接口出站日志（一次性，源 memory [[platform-egress-http-logging]]） |

这 3 处都是 **one-shot**（整条日志一次性构建完毕，无后续累积 UPDATE），INSERT OR REPLACE 不引入 SELECT（REPLACE 是 DELETE+INSERT 的语义，SQLite 内部走主键索引，非业务 SELECT）。也无优化空间。

---

## 2. 热路径 SELECT 清单

**结论：proxy_log 写入热路径当前 0 SELECT。**

完整证据（grep `SELECT.*FROM proxy_log` 全部命中点，排除测试）：

| 文件:line | 用途 | 是否热路径写时 SELECT |
|---|---|---|
| `db/proxy_log.rs:268` (`list_proxy_logs`) | Logs 页列表查询 | 否，UI 读 |
| `db/proxy_log.rs:316` (`filtered_list_proxy_logs`) | Logs 页过滤列表 | 否，UI 读 |
| `db/proxy_log.rs:340` (`filtered_count_proxy_logs`) | Logs 页计数 | 否，UI 读 |
| `db/proxy_log.rs:416` (`get_proxy_log`) | Logs 详情 / 单行查 | 否，UI 读 |
| `db/usage_stats.rs:58` | 最近 5 次状态码滑动窗 | 否，UI 聚合 |
| `db/usage_stats.rs:343` | 平台维度 usage | 否，UI 聚合 |
| `db/query_stats.rs:524` | 按模型/组统计 | 否，UI 聚合 |
| `db/stats_agg.rs:37-42` | 全表扫描聚合到 `stats_agg_hourly` | **否，触发式聚合**（非每请求） |
| `db/maintenance.rs:176` | cleanup COUNT | 否，维护任务 |

**全部 0 个写时 SELECT**。`stats_agg.rs` 的全表 SELECT 也不在每请求路径上 —— 是 `upsert_stats_agg` 单行 upsert（聚合时由后台触发的全量重建）。

---

## 3. 用户提案评估

### 3.1 可行性：**已实现**，无需重复实现

| 提案点 | 现状 | 已实现位置 |
|---|---|---|
| 缓存 in-flight row | ✅ `log_snapshots: Mutex<HashMap<id, ProxyLogColumns>>` | `proxy/mod.rs:129` |
| 首次 INSERT | ✅ `insert_proxy_log_columns` | `db/proxy_log.rs:205` |
| 后续 UPDATE | ✅ `update_proxy_log_columns`（diff，更优） | `db/proxy_log.rs:231` |
| 0 SELECT | ✅ 零 SELECT（旧值来自内存快照） | `proxy/log.rs:119-122` |
| 5 分钟内 | ✅ 终态写入后立即 `remove_log_snapshot`（更紧） | `proxy/log.rs:143-145`、`stream.rs:206` |

### 3.2 收益预估：**0**（提案能力已具备）

proxy_log 写入路径 SELECT 已为 0。当前实现比用户提案**更优**：
- 提案：后续 UPDATE 直接写（假设要写全列或固定列集）
- 现状：后续 UPDATE **只写变化列**，零变化时连 SQL 都不发（节省写连接串行队列带宽）

### 3.3 风险（若强行再加一层缓存）

| 风险 | 说明 |
|---|---|
| 崩溃丢失 | 现状 `log_snapshots` 是内存，进程崩溃 in-flight 终态日志丢失，DB 已有第一节点 INSERT 行（首节点立即落库），后续更新丢但行存在 —— 已是当前语义，加新缓存不改变这点 |
| 多线程并发 | `log_snapshots` 用 `std::sync::Mutex`，handler 主链路 + 流式 guard Drop 路径共享同一 id 的快照（`proxy/mod.rs:127-129` 注释明说）。提案若再加一层会有双写一致性问题 |
| 内存膨胀 | 现状终态立即 `remove_log_snapshot`（`proxy/log.rs:144`），in-flight map 不会积压。提案的 5min TTL 反而更松，**内存占用更高** |
| 与 DB 一致性 | 现状每次 UPDATE 落库（写连接串行），内存快照是 DB 行的镜像。提案「内存持 row → 后续 UPDATE」若延迟落库则崩失，若同步落库则等价现状 |

### 3.4 单写连接的瓶颈（提案可能误判的源头）

`Db(pub AsyncConnection, ...)`（`db/mod.rs:174`）—— **单写连接**（tokio-rusqlite 后台线程串行所有 `call_traced` 闭包，`db/mod.rs:165-168`）。proxy_log 的 INSERT/UPDATE 全在这条串行队列上。

如果有人误以为「每次写都要 SELECT 慢」，真因可能是：
- 写连接串行队列堆积（多请求并发时排队）
- 每次写仍有 prepare/IPC 开销（`prepare_cached` 已缓解）

这些都是「写多」的问题，提案的「少读」对此无帮助。若真想优化写吞吐，方向是**批量延迟写**（攒一批 UPDATE 一次提交），不是 SELECT 消除。

---

## 4. 推荐方案

### 推荐：**不实施**，提案收益 = 0，能力已具备

当前 `ProxyLogColumns` + `log_snapshots` diff-UPDATE 机制完全覆盖用户提案，且更优（diff-UPDATE + 终态立即清理）。

### 若用户有进一步诉求，应澄清的真实痛点方向

| 若真实痛点 | 建议方向（仅信息，非本任务范围） |
|---|---|
| 写连接串行队列堆积 | 批量延迟写（攒 N 个变更合一次 `call_traced`），或单事务多行 commit |
| 大字段写入慢（request_body / response_body TEXT 几 MB） | 列存分离 / 压缩 / 异步落库 |
| stats_agg 全表扫描 | 已有覆盖索引（memory [[perf-hotpath-optimization]] 提到 `idx_proxy_log_stats`），增量聚合 |
| Logs 页查询慢 | UI 读路径已有 `call_read_traced` 读连接池（`db/mod.rs:171`） |

---

## 5. 不确定点

1. **是否真有性能问题驱动用户提案**：本研究是静态代码分析，未跑 benchmark。若用户基于实测慢点提出，建议先 profile 定位真瓶颈（写连接队列 vs 大字段 vs stats_agg 触发），不要假设 SELECT。
2. **memory [[perf-hotpath-optimization]] 的实施范围**：本研究核对了源码 `proxy/log.rs:118-140`，与 memory 描述一致，证明实施已落地、未被回滚。memory 的字段完整性测试（`progressive_columns_equals_full_replace` at `test_proxy_log.rs:10`、`progressive_columns_strip_equivalence` at `test_proxy_log.rs:102`）也已存在。
3. **upsert_proxy_log（旧路径）是否需迁移到新机制**：3 处 one-shot 调用方无累积写需求，保留全列 INSERT OR REPLACE 是合理的。无需迁移。

---

## Findings 引用清单

### 源码 file:line

- `src-tauri/src/gateway/proxy/log.rs:118-140` — 取 prev 快照决定 INSERT/UPDATE 的核心逻辑（零 SELECT）
- `src-tauri/src/gateway/proxy/log.rs:143-145` — 终态立即 `remove_log_snapshot`（比 5min TTL 更紧）
- `src-tauri/src/gateway/proxy/log.rs:163-165` — `remove_log_snapshot` 实现
- `src-tauri/src/gateway/proxy/mod.rs:129` — `log_snapshots: Mutex<HashMap<String, ProxyLogColumns>>`
- `src-tauri/src/gateway/proxy/mod.rs:125-129` — 内存快照设计注释（多线程共享、流式 guard 协作）
- `src-tauri/src/gateway/proxy/stream.rs:202-207` — 流式终态 flush 路径 `upsert_log + remove_log_snapshot`
- `src-tauri/src/gateway/db/proxy_log.rs:78-112` — `ProxyLogColumns` 结构定义
- `src-tauri/src/gateway/db/proxy_log.rs:114-156` — `from_log` 构造（脱敏就地应用）
- `src-tauri/src/gateway/db/proxy_log.rs:159-201` — `changed_since` 逐列 diff
- `src-tauri/src/gateway/db/proxy_log.rs:205-225` — `insert_proxy_log_columns`（首节点 INSERT）
- `src-tauri/src/gateway/db/proxy_log.rs:231-259` — `update_proxy_log_columns`（后续 UPDATE，空变化 no-op）
- `src-tauri/src/gateway/db/proxy_log.rs:50-69` — `upsert_proxy_log`（旧全列 REPLACE，仅 one-shot 调用）
- `src-tauri/src/gateway/db/mod.rs:165-174` — 单写连接（tokio-rusqlite 串行）+ 读池架构
- `src-tauri/src/commands/model_fetch.rs:88,100` — `upsert_proxy_log` one-shot 调用
- `src-tauri/src/gateway/quota/http.rs:208` — `upsert_proxy_log` one-shot 调用

### Related memory

- [[perf-hotpath-optimization]] — 直接对应，列快照 diff-UPDATE 机制的设计动机 + 踩坑
- [[streaming-sse-log-aggregation]] — 流式终态 flush 路径
- [[est-cost-persistence]] — est_cost 写入点（已并入 `upsert_log`）

### 相关 spec / 文档

- `CLAUDE.md` "Proxy 日志" 章节 — retention 分级、headers vs body 入库策略
