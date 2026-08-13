# mock 压测能力补全 — 详细设计

## 为什么这个 task 排在最前

本批性能 task 中，`proxy-hotpath-buffers` 的验证链路明写「50 路并发 mock 流」，`perf-final-verification` 的全部结论建立在同一压测台上。用户硬约束：**压测只允许用 mock 平台与 mock 分组，禁用真实平台**。

故 mock 的实现瑕疵不是「测试工具的小问题」，而是**所有内存/CPU 数字的系统误差源**。先补齐它，后面的量测才可信。

## 现状（已勘察，file:line）

mock **不起本地 server**，在 forward 层短路：

| 环节 | 位置 |
|---|---|
| 拦截点 | `gateway/proxy/handler.rs:408-430`（`Protocol::Mock` 命中即 return，不建 reqwest 连接、不进重试） |
| 处理函数 | `gateway/proxy/mock.rs:5-158` |
| 非流式响应 | `gateway/adapter/mock/response.rs:11,18-99` |
| 流式 SSE | `gateway/adapter/mock/stream.rs:10,12-27,31-42` |
| 配置三层覆盖 | `gateway/adapter/mock/config.rs:46`（body 顶层 `mock` > message role 映射 > `platform.extra.mock`） |
| 字段定义 / 默认值 | `config.rs:11-25` / `:27-42` |
| 前端编辑器 | `src/domains/platforms/MockConfigEditor.tsx:14-114` |
| 挂载点 | `src/pages/platforms/PlatformEditForm.tsx:258-261`（`isMock` 时替代 endpoints/Key/模型区块） |
| 序列化 | `src/services/api/platforms.ts:124-151`，默认值 `:11` 起 |

现有可控字段：`response_text` / `chunk_count` / `delay_ms` / `input,output,cache_tokens` / `status_code` / `error_mode` / `stream_override` / `finish_reason`。

### 三个阻断项

**① 每请求一次 platform 写连接（唯一真正的数据污染源）**

`proxy/mock.rs:96-104` 无条件调用 `apply_manual_budgets` → `gateway/manual_budget.rs:194-215` 走 `platform_write_conn()`。tokio-rusqlite 是**单后台线程串行执行**。默认 `input+output = 150 > 0` 恒触发，即使用户没配任何限额也要先 `SELECT manual_budgets`（`manual_budget.rs:196-203`），且 `db.invalidate_group_details_cache()` 在函数末尾**无条件**执行（`manual_budget.rs:216`）。

50 路并发下这是唯一真正排队的 DB 写路径 —— 压测量到的「内存/CPU」里混着 tokio-rusqlite 的排队。

**这不是 mock 专属问题**：真实转发路径走同一个 `apply_manual_budgets`。修它是真实热路径优化。

**② `delay_ms` 语义重载**

`proxy/mock.rs:22-24` 响应前整体 sleep 一次；`proxy/mock.rs:113-118` 流式时每个 chunk 前**再 sleep 同一个值**。没有独立 TTFT / inter-chunk 旋钮，造不出「首包 800ms、后续 30ms/chunk」这类真实流形。

**③ `error_mode` 确定性单值**

`proxy/mock.rs:32` 单值判定，只能整平台全成功或全失败，做不到「5% 请求 429」。

## 方案（当前方案 = 精简守现状）

三处独立改动 + 一处前端同步。

### 1. manual_budget 空短路

判空点前移到**进写连接之前**。现状判空在闭包内部、已经在写连接上了（`manual_budget.rs:194-215`）。

修法：先走只读路径判「本 platform 是否有任何 budget 配置」，无则直接 return，不取写连接、不失效缓存。

**硬约束**（红线 2，计费路径）：有配额时的扣减逻辑与失效时机**逐字不变**。短路只在「零配额」这一条分支上生效。

判「是否有配额」的来源需勘察 —— 若已有 platform 级缓存可复用则复用（对照 `db/group_platform.rs:339-359` 的缓存 idiom），否则走只读池一次 SELECT（只读池不串行，8 条并发）。

### 2. `ttft_ms` / `inter_chunk_ms` 拆旋钮

`config.rs:11-25` 加两个 `Option<u64>` 字段。取值逻辑：

- `ttft_ms.unwrap_or(delay_ms)` → 用在 `mock.rs:22-24`
- `inter_chunk_ms.unwrap_or(delay_ms)` → 用在 `mock.rs:113-118`

`delay_ms` 保留为兼容入口，现有配置零行为变化。两行取值 + 两个字段，不新造机制。

### 3. `error_rate` 概率注入

`config.rs` 加 `error_rate: Option<f64>`（0.0-1.0）。`mock.rs:32` 的判定前置一层：`error_rate` 存在且本次命中 → 走既有 `error_mode` 分支；否则正常。

**不新增 `error_mode` 枚举值** —— 复用现有四值（`none`/`http_error`/`rate_limit_429`/`timeout`），`error_rate` 只决定「这次是否触发」。

随机源优先用**确定性伪随机**（原子请求计数器 + 取模），非 `rand`：压测场景需要可复现，且省一个依赖判断。`ponytail:` 注释标明取舍与升级路径。

### 4. 前端同步（用户明令）

`MockConfigEditor.tsx:67-74` 的数值网格加三格：`ttft_ms` / `inter_chunk_ms` / `error_rate`。

- `usePlatformForm.ts:202-204` 补默认值
- `services/api/platforms.ts:124-151` 的 `serializeMockConfig` / parse 补字段，`:11` 起默认值同步
- 8 语言 locale key（`src/locales/*.json` 顶层扁平 dotted key —— memory `locale-flat-key-convention`），`scripts/check-i18n.mjs` 必须绿

## 为什么不选别的

| 备选 | 否决理由 |
|---|---|
| 不改 mock，压测时手工把 token 置 0 | 用户已否决 —— 每个 verify subtask 都要记得这个前置，忘一次数据就脏且难发现 |
| 给 mock 单独绕过 `apply_manual_budgets` | 治标。真实转发路径同样每请求进写连接，堵源头更便宜且是真优化 |
| 加并发级观测埋点 / chunk 字节控制 / timeout 可配 | 内存与 CPU 压测用不上，YAGNI（边界章已排除） |
| 独立的 loadgen 二进制 / 外部压测工具 | 用户约束是「用 mock 平台与分组」；且外部工具测不到应用内部内存分解 |
| mock 起独立本地 server | 现状短路设计更省资源，改架构无收益 |

## 数据流（验证链路）

```
配零配额 mock 平台 → 单请求 → trace/计数器确认未触及 platform 写连接
配有配额 mock 平台 → 逐条比对扣减结果与改动前一致（红线 2）
ttft_ms=800 / inter_chunk_ms=30 → 客户端记录首包与 chunk 间隔时间戳，±20% 内
只设 delay_ms → 行为与改动前逐条一致（向后兼容）
error_rate=0.05 × 200 请求 → 统计 429 比例落 5%±3%
50 路并发 mock 流 5min → 无 panic、无非注入失败
cargo clippy --workspace + cargo test --workspace + yarn build + check-i18n
```

## 可能性分支（不进当前方案，仅留痕）

- **per-request 观测埋点（chunk 时间线）** — 触发条件：若压测中发现流形与配置不符但查不出原因。
- **`timeout` 模式时长可配** — 触发条件：若要压「大量连接长期占用」的场景。当前硬编码 600s（`mock.rs:77`）在内存压测中反而是特性（能造长驻连接）。
- **chunk 字节大小直接控制** — 触发条件：若要精确复现「单 chunk 超 16MB cap」的边界用例。当前可用 `response_text` 长度 ÷ `chunk_count` 间接凑。
- **`apply_manual_budgets` 全路径改只读预检 + 批量写** — 触发条件：若空短路后配额路径仍是并发瓶颈。改动面大，超本 task 边界。
