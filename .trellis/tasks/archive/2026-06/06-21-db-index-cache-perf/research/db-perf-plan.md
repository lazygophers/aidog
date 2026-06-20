# aidog DB 性能审计 —— 分组加载 + 索引 + 缓存

> 只读审计，含实测 EXPLAIN QUERY PLAN（DB: `~/.aidog/aidog.db`, proxy_log=13680 行 / group_platform=59 / platform=39 / group=15）。
> 标 `推测:` 者无实测，仅静态推断。

## 1. 分组 → 平台加载瓶颈

### 真因：前端 N+1 逐组 invoke（不是 DB，不是缺索引）

`src/pages/Groups.tsx:601-621` `load()` 主路径：
```
for (const g of groups) {
  detail = await groupDetailApi.get(g.id);   // line 605 —— 每组一次 Tauri invoke (command group_detail)
  ...
  for (const gp of filled.platforms) {
    upsertPlatform(...); reportCount();
    await Promise.resolve();                   // line 618 —— 每平台让出一个 microtask
  }
}
```
- **频率**：每次进入 Groups 页 / reload。15 个组 = 15 次串行 `invoke("group_detail")`（每次跨 JS↔Rust↔SQLite 往返），外加 59 次 `await Promise.resolve()` microtask 让出。
- **证据**：
  - 后端**已存在**单次批量命令 `group_detail_list`（api.ts:720 → db.rs:2164 `list_group_details`），但 `load()` 主路径**没用它**，只在 `refreshStats()`(Groups.tsx:647) 用。逐组 `get` 是为「平台级流式上屏」体验刻意写的。
  - 单组 JOIN 查询已最优，无缺索引：
    `EXPLAIN: SEARCH gp USING INDEX sqlite_autoindex_group_platform_1 (group_id=?)` → `SEARCH p USING INTEGER PRIMARY KEY (rowid=?)`。`group_platform` 的 `UNIQUE(group_id,platform_id)` 自动索引已覆盖 group_id 等值查找（db.rs:2093 JOIN）。
  - DB 侧极快：batch group stats（含全部 SUM 列）实测 **24ms / 13680 行**，走 `idx_proxy_log_group_key`。
- **结论**：分组加载慢 = **前端串行 N+1 invoke + 强制 microtask 让出的累积延迟**，DB 层不是瓶颈。15 组 × (invoke RTT ~几 ms + 反序列化) 串行叠加 + 59 个 await，在慢机/大组数上体感明显。

### 修复方向（前端，非 DB）
- 主路径改用 `groupDetailApi.list()`（已有的 `group_detail_list`，db.rs:2164 一次拉全部组+平台），一次 invoke 拿全量，前端再做「流式上屏」动画（用 setTimeout/rAF 分帧 append，而非靠串行 await 网络往返制造节奏）。
- `list_group_details`(db.rs:2164-2178) 本身是 Rust 内 for 循环逐组调 `get_group_platforms`（每组一条 SQL）——**后端 N+1**，但都在同一后台连接串行、无 invoke RTT，59 行规模可忽略（推测: <5ms）。若组数增长可改单条 `JOIN ... ORDER BY group_id, priority` 一次取全量再分组，非当前优先。

## 2. 建议新增索引清单

现有索引（实测 `.indexes`）：
- proxy_log: idx_proxy_log_stats(覆盖,Mig019) / _group_key(Mig) / _platform_id(Mig) / _created / _group / _model / _actual_model / _platform / _status
- group_platform: 仅 sqlite_autoindex(UNIQUE group_id,platform_id)
- middleware_rule: idx_mw_rule_lookup(enabled,rule_type,scope) db.rs:273
- notification: **无二级索引**
- setting/group/platform: 仅 autoindex(UNIQUE)

| 优先 | 表 | 索引 | 针对查询 | 收益 | 代价 | 实测依据 |
|---|---|---|---|---|---|---|
| 中 | proxy_log | `idx_proxy_log_group_key_stats ON proxy_log(group_key, est_cost, input_tokens, output_tokens, cache_tokens, status_code) WHERE deleted_at=0` | get_all_group_usage_stats(db.rs:3303) GROUP BY group_key + SUM | 现走 `idx_proxy_log_group_key` 但**回表取 SUM 列**(EXPLAIN: SCAN USING INDEX, 非 COVERING)。改覆盖索引 → index-only scan 免回表 | 写放大 + 磁盘(6 列宽索引) | 实测当前 24ms，量小收益有限；proxy_log 增长到数十万行才显著。**推测: 当前不必加** |
| 低 | notification | `idx_notification_created ON notification(created_at)` | db.rs:2522 ORDER BY created_at DESC LIMIT / db.rs:2559 DELETE WHERE created_at< | 通知量小，ORDER BY 全扫无所谓 | 极小 | 静态：表无索引但行数少，**推测: 非必要** |
| —— | group_platform | 无需新增 | get_group_platforms(db.rs:2093) | 已走 UNIQUE 自动索引 | —— | 实测 EXPLAIN 已最优 |
| —— | platform/group/setting | 无需新增 | 全量 list + 按 PK/UNIQUE | 表 <40 行，全扫即最优 | —— | 行数极小 |

**结论：当前数据规模下没有真正"缺失致命索引"。** Migration 019/group_key/platform_id 已覆盖统计热路径。最有前瞻价值的是 `idx_proxy_log_group_key_stats` 覆盖索引（proxy_log 持续增长场景），但属"为未来扩容"而非"修当前慢"。

## 3. 缓存方案

### 现有缓存（db.rs:68 DbCache）
- `settings: RwLock<HashMap<(scope,key), Option<Value>>>` —— setting 表读，写时失效（命中路径零分配，commit bb89b3d）。
- `groups: RwLock<Option<Vec<Group>>>` —— `list_groups()`(db.rs:1847-1866) 命中即返回 clone，写 group 表整体失效。

### 热点 1：group_platform / GroupDetail（读多写少，强烈建议缓存）
- **现状**：`list_groups` 已缓存，但**关联的 group_platform / GroupDetail 没缓存**。每次 Groups 页加载、每次 resolve_group 都重查 group_platform JOIN platform。
- **读频**：Groups 页加载（15 次 get_group_platforms）+ 代理热路径 resolve_group 选平台。
- **写点（失效触发，file:line）**：
  - `set_group_platforms` db.rs:1979（DELETE+INSERT 批量改组成员）
  - `reorder_group_platforms` db.rs:1757
  - `set_group_platform_level_priority` db.rs:1782
  - `move_group_platform` db.rs:1806
  - `delete_platform` db.rs:1102（清 group_platform）
  - purge/孤儿清理 db.rs:1228 / 2047
  - platform 表更新（platform 列随 GroupDetail 返回）→ platform create/update/delete 也须失效
- **方案**：DbCache 加 `group_details: RwLock<Option<Vec<GroupDetail>>>`，`list_group_details`(db.rs:2164) 命中返回 clone；上述全部写点统一 invalidate（清 None）。与 DB 一致性：写时失效（同 groups 缓存模式），单后台连接串行保证读到的是失效后重建值，无脏读窗口。
- **注意**：platform 写点多（model_test 更新 last test / quota 更新 est_balance_remaining），失效会频繁。`est_balance_remaining` 高频更新 → 若缓存 GroupDetail 含 platform.est_balance_remaining，每次余额刷新都失效，缓存命中率低。**建议缓存「组结构+platform 静态字段」，余额/统计走独立路径**（前端 fetchGroupStats 已把 balance 拆出，Groups.tsx:461-468 用 platformApi.list 的 est_balance_remaining 求和，不依赖 GroupDetail 内的值）。

### 热点 2：platform 全量 list（读多写少，建议缓存）
- **读频**：Groups 页（Groups.tsx:626 platformApi.list）、Platforms 页、resolve 路径。
- **写点（失效）**：platform create(db.rs:766)/update/delete(db.rs:1102)；est_balance_remaining 更新（quota）；last_test 更新（model_test）。
- **方案**：DbCache 加 `platforms: RwLock<Option<Vec<Platform>>>`。**但** est_balance/last_test 高频写 → 失效频繁，命中率存疑。**推测: 收益不如热点1**，建议先只缓存 group_details。

### 热点 3：settings —— 已缓存，无需动（db.rs:71）

### 一致性总原则（用户明确要求维持关系）
- 所有缓存走**写时失效**（清 None，下次读重建），不做 TTL/惰性过期 → 永不返回过期数据。
- tokio-rusqlite 单后台线程串行执行所有 call → 失效与重建天然有序，无并发脏读。
- 失效务必**穷举所有写点**（上面列的 file:line），漏一个 = 静默陈旧 bug（参照记忆 mount-fetch-late-resolve / cache-invalidation 类坑）。

## 4. 实施分解

1. **前端（最高优先，真正修分组慢）**：Groups.tsx `load()` 主路径用 `groupDetailApi.list()` 一次拉全量，流式上屏改为前端分帧（rAF/setTimeout）而非串行 await invoke。预计消除 15 次 invoke RTT + 59 microtask 串行 → 体感大幅改善。无 DB / migration 改动，回归面仅前端渲染。
2. **缓存层（次优先）**：DbCache 加 `group_details` 缓存（db.rs:68 struct + list_group_details:2164 命中逻辑 + 6 处写点失效）。代理 resolve 热路径与 Groups 页同时受益。
3. **索引（可选/前瞻）**：proxy_log 增长后再上 `idx_proxy_log_group_key_stats` 覆盖索引（一条 migration，幂等 IF NOT EXISTS）。当前 13680 行不必。

## 5. 风险

- **缓存失效遗漏**：group_platform/platform 写点分散（11+ 处，含 delete_platform 级联、purge、import apply），漏一处即陈旧。导入(import apply 绕过 platform_create，见记忆 import-apply-bypasses-platform-create)路径尤须显式失效。
- **est_balance/last_test 高频写降低缓存命中**：若缓存对象含这些字段，频繁失效退化为无缓存。建议缓存结构与余额/测试态解耦。
- **前端流式改造回归**：去掉串行 await 后须保证 alive() 守卫、StrictMode 双跑、reload 竞态仍正确（参照记忆 mount-fetch-late-resolve-overwrites-optimistic）。
- **索引写放大**：proxy_log 是最高频写表（每请求一行），新增宽覆盖索引增加写成本，需权衡读收益。
