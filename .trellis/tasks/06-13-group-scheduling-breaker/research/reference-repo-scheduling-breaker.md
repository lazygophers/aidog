# Research: ding113/claude-code-hub —— group 智能调度 + 熔断器

- **Query**: 调研 ding113/claude-code-hub 的「group 级智能调度 (load balancing / 平台选择策略) + 熔断器」设计与实现，为 aidog 新建 group 调度&熔断功能提供参考
- **Scope**: external（远程 GitHub 仓库）
- **Date**: 2026-06-13
- **访问方式**: 通过 GitHub API（`/repos/.../git/trees/main?recursive=1`，2993 文件，未截断）+ `raw.githubusercontent.com` 拉源码。仓库可访问，`default_branch=main`，`pushed_at=2026-06-13T08:27:08Z`（活跃）。
- **引用基准 ref**: `ding113/claude-code-hub@main`（行号对应 2026-06-13 抓取时点 HEAD，仓库活跃，行号可能漂移；引用以「文件+符号名」更稳）。
- **与前次中间件调研的边界**: 上一份 `.trellis/tasks/06-13-request-response-middleware/research/reference-repo-middleware.md` 聚焦 GuardPipeline / 规则引擎 / 整流器。本份**只讲调度策略与熔断器**，不重复中间件内容。

---

## 结论先行

1. **"group" 在该仓库不是一等实体级调度单元**。调度的真正主体是 **provider（上游供应商）**；group 体现为两层：① `providers.groupTag`（字符串标签）做候选**预过滤**（按用户/key 的 `providerGroup` 把候选 provider 缩小到本组）；② `providers.groupPriorities`（JSONB `Record<group, number>`）做**per-group 优先级覆盖**。还有一张 `provider_groups` 表，但它**只承载 `cost_multiplier`（计费倍率）**，不承载调度策略字段。
   - 引用: `src/app/v1/_lib/proxy/provider-selector.ts:752-770`（group 预过滤）、`:1121-1132`（resolveEffectivePriority 取分组覆盖最小值）、`src/drizzle/schema.ts:172-179`（provider_groups 仅 cost_multiplier）、`:197-200`（priority/groupPriorities/groupTag）。

2. **调度算法 = 多级硬过滤 → 优先级分层（取最高层）→ 同层内按成本排序 + 加权随机**。没有"轮询/最小延迟/最小连接/粘性会话作为调度策略"这种枚举式 strategy 字段；策略是写死的固定管线（priority + weighted-random），可调的只有每个 provider 的 `weight`/`priority`/`groupPriorities`/`costMultiplier`。粘性会话(sticky session)是**独立机制**（session 复用上次 provider，可被 `disableSessionReuse` 跳过），不属于"调度策略选项"。
   - 引用: `provider-selector.ts:984-1013`（分层+加权）、`:1137-1197`（selectTopPriority/selectOptimal/weightedRandom）、`schema.ts:214-215`（disableSessionReuse）。

3. **熔断器有三层**，全部是标准 closed/open/half-open（或 closed/open）状态机，**作用粒度都是 provider 或更细，没有 group 级熔断器**：
   - **Provider 级**（主力）: `src/lib/circuit-breaker.ts`，per-provider 三态机，配置存 `providers` 表 3 个列，状态存内存 Map + Redis。
   - **Endpoint 级**: `src/lib/endpoint-circuit-breaker.ts`，per (provider×具体 URL endpoint) 三态机。默认关（env 开关）。
   - **Vendor-Type 级**: `src/lib/vendor-type-circuit-breaker.ts`，per (vendorId×providerType) 两态机（closed/open，无 half-open），用于"某 vendor 的某协议类型所有端点都超时"时临时全屏蔽 60s。默认关。
   - 引用: `circuit-breaker.ts:33-42`、`endpoint-circuit-breaker.ts:19-22`、`vendor-type-circuit-breaker.ts:13-22`。

4. **熔断器与重试/load-balancer 的分工**:
   - **熔断器作"准入门"**: provider 选择阶段 `filterByLimits` 先 `isCircuitOpen()` 把 open 的 provider 直接踢出候选（在加权随机之前）。
   - **重试在熔断器之下**: 单 provider 内重试 `maxRetryAttempts` 次（默认 2），重试**耗尽后**才 `recordFailure()` 计一次熔断失败 + 标记该 provider failed 并切换到下一个候选 provider。
   - 客户端错误（NON_RETRYABLE_CLIENT_ERROR）、404、raw passthrough、probe 请求**不计熔断**。网络错误默认**不计**熔断（env 开关 `ENABLE_CIRCUIT_BREAKER_ON_NETWORK_ERRORS=false`）。
   - 引用: `provider-selector.ts:1047-1115`（filterByLimits 含 isCircuitOpen）、`forwarder.ts:1800-1851`（NON_RETRYABLE 不计）、`:1934-1957`（网络错误默认不计）、`:2096-2103`（重试耗尽才 recordFailure+切换）。

5. **多实例架构（TS+Postgres+Redis）**: 熔断状态内存 Map + Redis 持久化（多实例共享 + 重启恢复），配置走 Redis hash 缓存（miss 回退 DB）+ pub/sub 失效通知。**aidog 单进程 Rust+SQLite 不需要 Redis 这层**，直接内存 + SQLite 即可（详见第 5 节移植分析）。

---

## 1. 智能调度（Provider 选择策略）

### 1.1 选择主算法（核心，`provider-selector.ts` `ProxyProviderResolver`）

完整顺序（`selectProvider` 主流程，约 `:740-1038`）:

```
Step 0  sticky session 复用：若 session 已绑定上次 provider 且未 disableSessionReuse 且仍健康 → 直接复用（:153-170）
Step 1  group 预过滤：按用户/key 的 providerGroup 匹配 provider.groupTag，缩小 visibleProviders（:752-770）
Step 2  enabled / 时间窗 / 模型白名单 / client 限制 过滤 → enabledProviders（:843-928）
Step 3  candidateProviders = enabledProviders
Step 4  健康过滤 filterByLimits：踢掉 [vendor-type 熔断 open] [provider 熔断 open] [金额限流超限] [总额超限] 的 → healthyProviders（:937, :1047-1115）
Step 5  优先级分层 selectTopPriority：只保留有效优先级最小（=最高优先）的那批（:984-1001, :1137-1150）
Step 6  selectOptimal：同层内按 costMultiplier 升序排序后 weightedRandom 选一个（:1003-1013, :1155-1197）
```

**没有命中的 group 严格隔离**: group 预过滤后该组无 provider 直接报错返回，不回退到全局（`:766-790`）。

### 1.2 具体"策略"实现

| 策略 | 是否实现 | 实现方式 / 配置字段 | 默认值 | 位置 (path:line) |
|---|---|---|---|---|
| 优先级 (priority) | ✅ | 取"有效优先级"最小值那批；有效优先级 = `groupPriorities[group]`（多组取最小）回退 `priority` | `priority` default 0 | `provider-selector.ts:1121-1150`；`schema.ts:197-198` |
| 加权随机 (weighted random) | ✅ | 同优先级层内 `weightedRandom`：累积权重 + `Math.random()*totalWeight`；totalWeight=0 时退化为均匀随机 | `weight` default 1 | `provider-selector.ts:1178-1197`；`schema.ts:194` |
| 成本优先 (cost) | ✅（仅排序，不决定概率） | selectOptimal 先按 `costMultiplier` 升序 sort 再加权随机（成本只影响 sort 顺序、决策仍由 weight 主导） | `costMultiplier` default 1.0 | `provider-selector.ts:1155-1172`；`schema.ts:199` |
| per-group 优先级覆盖 | ✅ | `groupPriorities` JSONB `Record<group,number>`，按用户分组覆盖全局 priority | default null | `provider-selector.ts:1121-1132`；`schema.ts:198` |
| 粘性会话 (sticky session) | ✅（独立机制非策略选项） | session 绑定上次 provider 复用；`disableSessionReuse=true` 的 provider 跳过 | `disableSessionReuse` default false | `provider-selector.ts:153-170`；`schema.ts:214-215`；迁移 `drizzle/0041_sticky_jackal.sql` |
| 时间窗调度 (active window) | ✅ | `activeTimeStart/End`(HH:mm) + 系统时区，支持跨天；非窗口期该 provider 不可调度 | 都 null = 始终激活 | `src/lib/utils/provider-schedule.ts:24-54`；`provider-selector.ts:503,850,898`；`schema.ts:234-237` |
| 轮询 (round-robin) | ❌ 无 | — | — | 全树搜 `round.?robin` 无调度实现 |
| 最小延迟 (least-latency) | ⚠️ 仅 endpoint 层 | provider **内部**的多 endpoint 选择按延迟排序并截断到 maxRetryAttempts；不是 provider 间策略 | — | `provider-selector.ts` resource endpoints / `forwarder.ts:1308-1316`、`src/lib/provider-endpoints/endpoint-selector.ts` |
| 最小连接 (least-conn) | ❌ 无 | 有并发上限 `limitConcurrentSessions`（达到则过滤），但不做"选连接最少者" | `limitConcurrentSessions` default 0(不限) | `schema.ts:276` |
| 健康感知 (health-aware) | ✅ | 即 Step 4 把熔断 open / 限流超限的 provider 排除（见熔断器章节） | — | `provider-selector.ts:1047-1115` |

**关键结论**: 该仓库的调度策略**不是可枚举切换的 strategy enum**，而是固定的「priority 分层 → cost 排序 → weighted random」组合，调参靠 per-provider 的 weight/priority/cost + per-group 优先级覆盖。

---

## 2. 熔断器（Circuit Breaker）

### 2.1 Provider 级熔断器（主力，`src/lib/circuit-breaker.ts`）

**状态机**（`ProviderHealth.circuitState: "closed"|"open"|"half-open"`，`:33-42`）:

```
closed --(failureCount >= failureThreshold)--> open  (设 circuitOpenUntil = now + openDuration)  :513-554
open   --(now > circuitOpenUntil，由 isCircuitOpen 惰性检测)--> half-open  (halfOpenSuccessCount=0)  :464-473
half-open --(成功，halfOpenSuccessCount >= halfOpenSuccessThreshold)--> closed  (清零全部)  :626-640
half-open --(失败，recordFailure)--> open  (重新计时)  :505-509 + :513
```

- **失败计数口径**:
  - `recordFailure(providerId, error)` → `failureCount++` + `lastFailureTime=now`（`:492-493`）。
  - 阈值判定 `failureCount >= failureThreshold` 才 open（`:513`），且 open 前会 **forceReload 配置再校验一次**防脏读（`:514-525`）。
  - **closed 状态下任意一次成功就把 `failureCount` 清 0**（非滑动窗口，是"连续失败计数"语义，`:651-665`）。
  - 已 open 状态再 recordFailure **不重置 openUntil**，只累加计数（`:505-509`）。
  - half-open 下成功累加 `halfOpenSuccessCount`，达到 `halfOpenSuccessThreshold` 才 closed（`:621-650`）。

- **配置字段 + 默认值**（per-provider，存 `providers` 表，`schema.ts:280-283`）:

| 字段 (DB 列) | 类型 | 默认 | 含义 |
|---|---|---|---|
| `circuit_breaker_failure_threshold` | integer | **5** | 连续失败多少次后 open；`<=0` 或非有限值 = **禁用熔断器**（强制 closed） |
| `circuit_breaker_open_duration` | integer(ms) | **1800000 (30 分钟)** | open 持续多久后转 half-open |
| `circuit_breaker_half_open_success_threshold` | integer | **2** | half-open 期间连续成功多少次才 closed |
| `max_retry_attempts` | integer (nullable) | null → env `MAX_RETRY_ATTEMPTS_DEFAULT` 默认 **2** | 单 provider 内重试次数（与熔断器协同，见 2.4） |

  - 代码侧默认常量 `DEFAULT_CIRCUIT_BREAKER_CONFIG = { failureThreshold:5, openDuration:1800000, halfOpenSuccessThreshold:2 }`（`circuit-breaker-config.ts:23-27`），用于配置读取失败/provider 不存在时降级。
  - **禁用机制**: `isCircuitBreakerDisabled` = `failureThreshold<=0`（`circuit-breaker.ts:207-209`），`handleDisabledCircuitBreaker` 把 health 强制 reset 为 closed（`:233-257`）。

- **核心 API**: `isCircuitOpen(providerId)`（准入查询，含 open→half-open 惰性转换，`:447-479`）、`recordFailure`（`:484-558`）、`recordSuccess`（`:612-671`）、`getCircuitState`（同步内存读，`:677-680`）、`getProviderHealthInfo`（决策链记录，`:435-442`）。

- **状态存储**: 内存 `healthMap: Map<providerId, ProviderHealth>`（`:45`）+ Redis 持久化（多实例共享/重启恢复，`circuit-breaker-state.ts`）。配置 5 分钟内存缓存 TTL（`CONFIG_CACHE_TTL`，`:48`），非 closed 状态最少 60s 强制 reload 一次配置以便及时响应管理员禁用（`NON_CLOSED_CONFIG_FORCE_RELOAD_INTERVAL_MS`，`:51`）。配置变更经 Redis pub/sub channel `cch:cache:circuit_breaker_config:updated` 失效（`:53,125-205`）。

- **降级/容错**: Redis 读失败 → 用内存/默认值（`:323-327`）；配置读失败 → 缓存默认配置避免高频重打（`:412-423`）。

- **告警**: open 时异步 `triggerCircuitBreakerAlert` → `sendCircuitBreakerAlert`（webhook，不阻塞主流程，`:543-607`）。webhook 配置在 `notification_settings` 表 `circuit_breaker_enabled`(default false) / `circuit_breaker_webhook`（`schema.ts:896-897`）。

### 2.2 Endpoint 级熔断器（`src/lib/endpoint-circuit-breaker.ts`，默认关）

同样 closed/open/half-open，粒度更细（per provider×具体 endpoint URL）。默认配置更激进：

```
DEFAULT_ENDPOINT_CIRCUIT_BREAKER_CONFIG = { failureThreshold:3, openDuration:300000(5min), halfOpenSuccessThreshold:1 }
```
- 引用: `endpoint-circuit-breaker.ts:19-22`。env 开关 `ENABLE_ENDPOINT_CIRCUIT_BREAKER` default **false**（`env.schema.ts:144`）。LRU 上限 `ENDPOINT_HEALTH_CACHE_MAX_SIZE=10000`（`:38`）。

### 2.3 Vendor-Type 级熔断器（`src/lib/vendor-type-circuit-breaker.ts`，默认关）

仅 closed/open 两态（**无 half-open**），用于"某 vendor 的某 protocol type 全部 endpoint 都 524 超时"时临时全屏蔽。

- 触发: `recordVendorTypeAllEndpointsTimeout(vendorId, providerType)`，open `AUTO_OPEN_DURATION_MS=60000`(60s) 后自动转 closed（惰性，`:22,131-143,145-166`）。
- 支持管理员手动 open `setVendorTypeCircuitManualOpen`（`:168-188`）。
- 复用同一 env 开关 `ENABLE_ENDPOINT_CIRCUIT_BREAKER`（`:121,151`），默认关。
- 在选择阶段优先于 provider 熔断检查（`provider-selector.ts:1050-1062`）。

### 2.4 与重试 / load-balancer 的交互（关键）

请求生命周期（`forwarder.ts`）:

```
选 provider（已排除熔断 open 的）→ 进入该 provider 内层重试循环（最多 maxRetryAttempts，默认 2）
  ├─ 成功 → recordSuccess(provider) （:1610）→ 返回
  ├─ NON_RETRYABLE_CLIENT_ERROR（4xx 客户端输入问题）→ 立即返回，不重试、不计熔断 （:1800-1851）
  ├─ 404 RESOURCE_NOT_FOUND → 先重试，耗尽后切换，不计熔断 （:1962-1979）
  ├─ 网络/系统错误 → 默认不计熔断（env ENABLE_CIRCUIT_BREAKER_ON_NETWORK_ERRORS=false）；
  │                  仅开启时 recordFailure；无论如何 markProviderFailed 切换 （:1928-1959）
  ├─ 524 且该 vendor-type 所有 endpoint 都超时 → recordVendorTypeAllEndpointsTimeout（:2147-2150）
  └─ 其他可重试错误（5xx 等）→ 未耗尽则重试；耗尽 → recordFailure(provider) + markProviderFailed → 切换下一个候选 provider （:2096-2103）
```

**核心分工原则**:
- **熔断器 = 跨请求的中长期"准入闸"**（open 后 30min 内整段拒绝该 provider，省得每次都试）。
- **重试 + provider 切换 = 单请求内的即时容错**（maxRetryAttempts 次同 provider + 切到其它候选 provider）。
- **失败只在"单 provider 重试全部耗尽"那一刻计 1 次熔断**，不是每次 attempt 都计，避免一个慢请求把计数打爆。
- `markProviderFailed` 维护 `failedProviderIds`，保证一次请求内不重复选同一 provider（与熔断 open 是两套去重，`:1932,2102`）。

---

## 3. group/provider 级配置落库与读取

### 3.1 表结构

- **`providers` 表**（调度+熔断的真正配置载体，`schema.ts:182-283`）:
  - 调度: `weight`(int default 1)、`priority`(int default 0)、`groupPriorities`(jsonb `Record<string,number>` default null)、`costMultiplier`(numeric default 1.0)、`groupTag`(varchar 255)、`disableSessionReuse`(bool default false)、`activeTimeStart/End`(varchar 5, nullable)。
  - 熔断: `maxRetryAttempts`(int nullable)、`circuitBreakerFailureThreshold`(int default 5)、`circuitBreakerOpenDuration`(int default 1800000)、`circuitBreakerHalfOpenSuccessThreshold`(int default 2)。
  - 限流（参与健康过滤）: `limit5hUsd/limitDailyUsd/limitWeeklyUsd/limitMonthlyUsd/limitTotalUsd/limitConcurrentSessions` + reset 模式（`:260-276`）。
  - 索引: `idx_providers_enabled_priority (is_enabled, priority, weight) WHERE deleted_at IS NULL`（调度查询优化，`:364`）。
- **`provider_groups` 表**（`schema.ts:172-179`）: 仅 `id/name(unique)/costMultiplier(default 1.0)/description` —— **只管计费倍率，不含调度/熔断字段**。
- **用户/key 的分组归属**: `users.providerGroup`、`api_keys.providerGroup`（varchar 200 default `'default'`，`:49,139`）—— 决定请求用哪个 group 去过滤 provider。

### 3.2 默认值机制（三层降级）

1. **DB 列 default**（建表即给，见上表）。
2. **代码常量 default**（DB/Redis 读不到时）: `DEFAULT_CIRCUIT_BREAKER_CONFIG`（`circuit-breaker-config.ts:23-27`）、`DEFAULT_ENDPOINT_CIRCUIT_BREAKER_CONFIG`（`endpoint-circuit-breaker.ts:19-22`）；调度侧 `priority ?? 0`、`weight`(notNull)、`costMultiplier`(读为字符串需 parse)。
3. **env 全局 default**: `MAX_RETRY_ATTEMPTS_DEFAULT`(default 2，range 1-10)、`ENABLE_CIRCUIT_BREAKER_ON_NETWORK_ERRORS`(false)、`ENABLE_ENDPOINT_CIRCUIT_BREAKER`(false)、`TZ`(Asia/Shanghai，给时间窗用)（`env.schema.ts:139-153`）。

### 3.3 读取路径

- **调度读取**: `findAllProviders()` → 进程级缓存（`ENABLE_PROVIDER_CACHE` default true，30s TTL，`env.schema.ts:145-148`）→ `ProxyProviderResolver.selectProvider`。
- **熔断配置读取**: `loadProviderCircuitConfig(providerId)` → Redis hash `circuit_breaker:config:{id}` 命中即返回 → miss 则 `findProviderById` 读 DB 并异步回填 Redis（`circuit-breaker-config.ts:43-97`）。内存再叠一层 5min TTL 缓存（`circuit-breaker.ts:369-430`）。

---

## 4. 对 aidog（单进程 Rust + SQLite，无 Redis）移植分析

### 4.1 可借鉴点

1. **熔断器三态机本身（直接可抄）**: closed/open/half-open + `failureThreshold`/`openDuration`/`halfOpenSuccessThreshold` 三参数语义清晰，Rust 用 `struct ProviderHealth { failure_count, last_failure_time, state: CircuitState, open_until, half_open_success_count }` + `RwLock<HashMap<provider_id, ProviderHealth>>` 即可复刻。**惰性 open→half-open 转换**（在 `is_circuit_open` 查询时按 `now > open_until` 转换）省去定时器，很适合 aidog。
2. **"连续失败计数 + 成功清零"语义**（非滑动窗口）实现成本低，aidog 直接用。
3. **失败只在单 provider 重试耗尽时计 1 次熔断**的口径，能与 aidog 现有「多平台失败重试 + 指数退避」对齐——retry 是请求内即时容错，breaker 是跨请求中期屏蔽。
4. **错误分类决定是否计熔断**: 客户端 4xx/404/probe 不计，5xx/超时才计。aidog 已有错误分类（参考 memory `platform-retry-failover` 的三态 status + `error-rule` 思路），可复用同一分类决定 `record_failure` 与否。
5. **per-group 优先级覆盖 `groupPriorities`（JSONB）**: 这正是用户要的"组内设置 + 默认值"——provider 全局 priority + 可选 per-group 覆盖。aidog 可在 platform 行加 `group_priorities` JSON 列或单开关联表，匹配 group 时取覆盖、否则回退全局。
6. **优先级分层 + 同层加权随机**的两段式调度，比纯加权更可控（高优组永远优先，组内才看 weight），实现简单且与现有 router 选平台契合。
7. **时间窗调度 `isProviderActiveNow`**（HH:mm + 时区 + 跨天）逻辑独立、79 行、零依赖，可几乎照搬到 Rust（用 `chrono-tz`）。
8. **配置三层降级（DB default → 代码常量 default → 全局 env/settings default）**，aidog 用 SQLite 列 default + Rust 常量 + settings 表即可，满足"提供默认值"诉求。
9. **熔断器"禁用"用 `failureThreshold<=0` 表达**（无需额外 enabled 布尔），减一个字段；aidog 可仿，或显式加 `circuit_breaker_enabled` bool（更符合用户"系统设置可开关"诉求）。

### 4.2 不适用 / 需改造点

1. **Redis 整层去掉**: 该仓库为多实例共享 + 重启恢复才上 Redis（config hash 缓存 + state 持久化 + pub/sub 失效）。aidog 单进程：状态用内存 `RwLock<HashMap>` 即可；如需重启恢复可选择性持久化到 SQLite（非必需，熔断状态短期、重启重新探测代价低）；配置变更直接 reload 内存缓存，**无需 pub/sub**。
2. **"group 不是调度主体"需重新定位**: 用户希望"熔断器+智能调度作用于 group 功能块，支持组内设置"。该仓库 group 只做候选过滤 + 优先级覆盖，**熔断器是 per-provider 不是 per-group**。aidog 若要"group 级熔断"（整组屏蔽），属于该仓库**没有**的能力，需自行设计（可参考 vendor-type 级"批量屏蔽"的两态机思路：某 group 内所有平台都不可用时短期屏蔽该 group）。建议明确：调度配置(策略/权重/优先级)放 group 级，熔断状态仍按 provider/platform 维度记录但**可在 group 维度聚合展示/批量重置**。
3. **三层熔断（provider/endpoint/vendor-type）对 aidog 偏重**: aidog 一个 platform 通常单一 base_url、单一协议，**endpoint 级与 vendor-type 级多半冗余**，建议只移植 **provider/platform 级**一层，避免半套实现。
4. **与现有 auto_disabled + 指数退避的分工（避免冲突）**:
   - aidog 现有 `auto_disabled`（401/403 鉴权失败 → 永久禁用待人工处理，参考 memory `platform-retry-failover`）是**鉴权/配置错误的永久态**；熔断器是**临时性服务降级（5xx/超时）的自动恢复态**。二者**不重叠**：401/403 走 auto_disabled（不计熔断，类比该仓库 NON_RETRYABLE 不计），5xx/超时走熔断器（自动 half-open 探测恢复）。
   - **指数退避** = 单请求内重试节奏；**熔断 openDuration** = 跨请求屏蔽时长。建议：retry 用指数退避（请求内），耗尽后才 `record_failure`；熔断 open 期间该平台直接从候选剔除（不进入 retry）。两者层级不同、不冲突。
   - 选平台时的过滤顺序建议对齐该仓库：`auto_disabled` → `circuit_open` → 限额/配额 → 加权随机。
5. **costMultiplier 在该仓库只影响排序不影响概率**（实质决策仍是 weight 加权随机），语义偏弱；aidog 若要"成本优先"应明确是排序还是真正影响选择概率，避免照抄这个弱语义。
6. **sticky session 复用**依赖该仓库的 session 概念（按 session_id 绑定 provider）；aidog 若无等价 session 概念则该机制不直接适用，或按 group_name+某 key 维度自定义。

---

## 5. 关键文件路径 + 引用（owner/repo/path:line）

调度:
- `ding113/claude-code-hub/src/app/v1/_lib/proxy/provider-selector.ts`
  - `:153-170` sticky session 复用；`:752-790` group 预过滤 + 严格隔离；`:843-928` enabled/时间窗/模型/client 过滤；`:937,1047-1115` `filterByLimits`（含三层熔断 + 限额）；`:984-1001,1137-1150` 优先级分层 `selectTopPriority`；`:1003-1013,1155-1173` `selectOptimal`（成本排序）；`:1178-1197` `weightedRandom`；`:1121-1132` `resolveEffectivePriority`（per-group 覆盖）。
- `.../src/lib/utils/provider-schedule.ts:24-79` `isProviderActiveNow`（时间窗，跨天 + 时区）。
- `.../src/app/v1/_lib/proxy/provider-selector-settings-cache.ts`（选择器 settings 缓存，32 行）。
- 迁移 `.../drizzle/0041_sticky_jackal.sql`（sticky session 字段）。

熔断器:
- `.../src/lib/circuit-breaker.ts`（provider 级三态机主体）
  - `:33-42` `ProviderHealth`；`:207-257` 禁用/reset；`:447-479` `isCircuitOpen`（open→half-open 惰性转换）；`:484-558` `recordFailure`（计数+open）；`:612-671` `recordSuccess`（half-open→closed/清计数）；`:677-680` `getCircuitState`；`:543-607` 告警。
- `.../src/lib/redis/circuit-breaker-config.ts:16-27` config 类型 + `DEFAULT_CIRCUIT_BREAKER_CONFIG`(5 / 1800000 / 2)；`:43-97` 读 Redis→DB 回退。
- `.../src/lib/redis/circuit-breaker-state.ts`（状态 Redis 持久化，247 行）。
- `.../src/lib/endpoint-circuit-breaker.ts:19-22` `DEFAULT_ENDPOINT_CIRCUIT_BREAKER_CONFIG`(3 / 300000 / 1)。
- `.../src/lib/vendor-type-circuit-breaker.ts:22,116-188` 两态机 + 60s 自动 open + 手动 open。
- `.../src/lib/circuit-breaker-loader.ts`(27 行)、`.../src/lib/circuit-breaker-probe.ts`(281 行，half-open 探测调度)。

熔断↔重试集成:
- `.../src/app/v1/_lib/proxy/forwarder.ts`
  - `:11-13,23,39-40` 引入 record* API；`:462-466` `maxRetryAttempts` 解析（provider 覆盖 / env default）；`:1610` `recordSuccess`；`:1800-1851` NON_RETRYABLE 不计熔断；`:1928-1959` 网络错误默认不计；`:1962-1979` 404 不计；`:2096-2103` 重试耗尽 `recordFailure`+切换；`:2147-2150` vendor-type all-timeout。

配置/默认值:
- `.../src/drizzle/schema.ts:172-179`（provider_groups 仅 cost）、`:182-283`（providers 调度+熔断+限额列）、`:364`（priority 索引）、`:896-897`（熔断 webhook）、`:49,139`（用户/key providerGroup）。
- `.../src/lib/config/env.schema.ts:140-157`（`ENABLE_CIRCUIT_BREAKER_ON_NETWORK_ERRORS`/`ENABLE_ENDPOINT_CIRCUIT_BREAKER`/`MAX_RETRY_ATTEMPTS_DEFAULT`/`TZ`/超时）。

管理端 / 测试（如需 UI 与行为参考）:
- `.../src/app/[locale]/settings/providers/_components/scheduling-rules-dialog.tsx`（调度规则配置弹窗）、`messages/*/settings/providers/schedulingDialog.json`（i18n）。
- `.../tests/unit/lib/circuit-breaker.test.ts`、`tests/unit/lib/vendor-type-circuit-breaker.test.ts`、`tests/unit/lib/endpoint-circuit-breaker.test.ts`、`tests/unit/proxy/provider-selector-group-priority.test.ts`、`tests/unit/lib/utils/provider-schedule.test.ts`（行为契约可作移植测试参考）。

---

## Caveats / 未覆盖

- 行号基于 2026-06-13 抓取的 main HEAD，仓库活跃（迁移已到 0105+），后续行号可能漂移；引用以「文件+符号名」为准更稳。
- `forwarder.ts`(4000+ 行)、`provider-selector.ts`(1345 行) 仅做针对性 grep + 局部精读，未全文通读；熔断↔重试集成已核对主路径（success / NON_RETRYABLE / 网络错误 / 5xx 耗尽 / 524 vendor-type），但未逐一核对所有 raw passthrough / probe / MCP 分支的计熔断细节。
- `circuit-breaker.ts` 仅精读 1-694 行（含全部状态机核心 API）；未读 `:695-992`（`getAllHealthStatus` 后续 / 批量 reload / 监控辅助函数），但状态机主逻辑已完整覆盖。
- `circuit-breaker-probe.ts`(half-open 主动探测调度，281 行) 与 `endpoint-selector.ts`/`probe-scheduler.ts` 仅确认存在与职责，未逐行精读算法。
- **负向结论**（已搜证）: 全树 grep `round.?robin` 无 provider 间轮询调度；无 least-connection 选择策略；`provider_groups` 表确认仅含 cost_multiplier（无调度/熔断列）；无 group 级（整组）熔断器——熔断器最粗粒度是 vendor-type 级而非 group 级。
- 该仓库 group 概念 = `providerGroup` 字符串标签匹配 `provider.groupTag`，**不是**独立可配策略对象；这是与 aidog 用户"group 功能块带组内调度/熔断设置"诉求的最大语义差，移植时须重新定位（见 4.2 第 2 点）。
