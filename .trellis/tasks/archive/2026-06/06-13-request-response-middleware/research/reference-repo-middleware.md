# Research: ding113/claude-code-hub 请求/响应中间件能力

- **Query**: 调研 ding113/claude-code-hub 的请求/响应中间件设计与实现，为 aidog 移植同类功能提供参考
- **Scope**: external（远程 GitHub 仓库）
- **Date**: 2026-06-13
- **访问方式**: octocode MCP 工具不可用（`No such tool available`），改用 GitHub REST API（`api.github.com/repos/.../git/trees`、`/search/code`）+ `raw.githubusercontent.com` 拉取源码。仓库可访问。
- **引用基准 ref**: `ding113/claude-code-hub@main`（行号对应抓取时点的 main HEAD，2026-06-13）

---

## 结论先行

claude-code-hub **不是**单一"中间件"模块，而是把用户要的 8 类能力拆成两套机制：

1. **GuardPipeline（守卫管线）** — 一条有序的、配置驱动的请求前置链，每个 step 可以「改写 session」或「提前返回 Response（拦截）」。请求过滤器、敏感词、限流、provider 选择都是其中的 step。挂载于代理入口。详见 `src/app/v1/_lib/proxy/guard-pipeline.ts`。
2. **Rectifier + ResponseFixer + ErrorRule（整流/覆写）** — 分散在转发层（forwarder）和错误处理层（error-handler），由 `SystemSettings` 的全局布尔开关控制，针对上游请求体/响应体做"整流"和"错误覆写"。

8 类能力的实现情况：

| 用户需求 | 仓库是否实现 | 实现载体 |
|---|---|---|
| 请求过滤器 | ✅ | `request_filters` 表 + `RequestFilterEngine` + GuardStep `requestFilter`/`providerRequestFilter` |
| 响应覆写 | ✅（仅错误响应覆写 + 响应整流） | `error_rules.override_response/override_status_code` + `ResponseFixer` |
| 数据脱敏 | ⚠️ 部分（仅用于日志/审计存储，非转发链路改写） | `message-redaction.ts`、`audit/redact.ts` |
| 动态注入 | ✅ | `request_filters` advanced 模式 `operations`（set/merge/insert DSL）+ rectifier（如 metadata.user_id 注入） |
| 请求响应整流器 | ✅ | 多个 `*-rectifier.ts`（thinking/billing-header/response-input）+ `response-fixer/`（encoding/SSE/JSON） |
| 内容过滤（密钥/邮箱） | ❌ 无专门的密钥/邮箱正则内容过滤器 | 最接近的是 `audit/redact.ts` 按 key 名脱敏（非正则内容扫描）；可用 `request_filters` text_replace + regex 自建 |
| 错误规则检测 | ✅ | `error_rules` 表 + `ErrorRuleDetector` + forwarder/error-handler 接入 |
| 敏感词检测 | ✅ | `sensitive_words` 表 + `SensitiveWordDetector` + GuardStep `sensitive` |

**开关模型有两类**：规则类（request_filters / sensitive_words / error_rules）**无全局开关**，靠 per-rule `is_enabled` + "缓存空即跳过"实现；整流器类（rectifier / responseFixer）**有 SystemSettings 全局布尔开关**，且**默认开启**（与用户"默认开启"需求一致）。

---

## 1. 技术栈与整体架构

- **语言/框架**: TypeScript，Next.js（App Router，`src/app/`），Bun 运行时（`.bun-version`、`bunfig.toml`），Hono + zod-openapi 做 v1 REST API（`@hono/zod-openapi`，见 `src/lib/api/v1/schemas/*`）。
  - 引用: `ding113/claude-code-hub` 仓库元数据 `language=TypeScript`；`ding113/claude-code-hub/bunfig.toml`、`biome.json`。
- **ORM/DB**: Drizzle ORM + PostgreSQL（`pgTable`），迁移在 `drizzle/0000..0105_*.sql`，schema 在 `src/drizzle/schema.ts`。其中 `drizzle/0024_request-filters.sql` 即请求过滤器表的迁移。
- **缓存/多实例同步**: 内存单例缓存（`globalThis` 挂载）+ 本地 `eventEmitter` + Redis pub/sub 跨进程失效通知（见各 detector 的 `setupEventListener`）。
- **代理核心目录**: `src/app/v1/_lib/proxy/`（守卫、转发、响应处理、整流、错误处理）。
- **规则引擎/检测器**: `src/lib/`（`request-filter-engine.ts`、`sensitive-word-detector.ts`、`error-rule-detector.ts`）。
- **管理端**: `src/app/[locale]/settings/{request-filters,sensitive-words,error-rules}/`（React 配置页）+ `src/app/api/v1/resources/{request-filters,sensitive-words,error-rules}/`（REST handlers）+ `src/actions/*.ts`（Server Actions）。

请求生命周期（关键挂载点）:
```
入站请求
  → GuardPipeline.run(session)        // guard-pipeline.ts，有序 step，可提前返回 Response
      auth → sensitive → client → model → version → probe → session
      → warmup → requestFilter(global) → rateLimit
      → provider(选上游) → providerRequestFilter(per-provider) → messageContext
  → ProxyForwarder.forward(session)   // forwarder.ts
      → requestFilterEngine.applyFinal(...)  // final 阶段：body/headers 最终改写，发送前
      → 发往上游
      → 上游错误 → getErrorDetectionResultAsync → error-handler 应用 override
      → 上游响应 → ResponseFixer（encoding/SSE/JSON 整流）
  → 回客户端
```
引用: `ding113/claude-code-hub/src/app/v1/_lib/proxy/guard-pipeline.ts:200-227`（CHAT_PIPELINE 顺序）。

---

## 2. 八类能力的实现方式

### 2.1 请求过滤器（Request Filter）— 核心，最值得移植

**数据流**: 管理端写 `request_filters` 表 → `RequestFilterEngine` 单例缓存（按 phase/binding 分桶 + 预编译 regex）→ GuardStep 调用。

**两个挂载点**（`guard-pipeline.ts`）:
- `requestFilter`（全局，provider 选择**之前**）→ `ProxyRequestFilter.ensure` → `requestFilterEngine.applyGlobal(session)`
- `providerRequestFilter`（per-provider/group，provider 选择**之后**）→ `requestFilterEngine.applyForProvider(session)`
- **第三阶段 final**：在 forwarder 序列化 body 前 `requestFilterEngine.applyFinal(session, body, headers)`，对最终请求体/头做最后改写（含 transport header 黑名单清理）。
  - 引用: `src/app/v1/_lib/proxy/request-filter.ts:13-26`、`provider-request-filter.ts:10-33`、`forwarder.ts:2424,2464,2748,2812,2858`、`request-filter-engine.ts:560-591`（`applyFinal`）。

**两种规则模式**（`rule_mode`）:
- `simple`: 单条 `scope(header|body) + action(remove|set|json_path|text_replace) + target + matchType + replacement`。
- `advanced`: `operations[]` 数组，DSL 操作（见 2.4 动态注入）。

**容错策略**: fail-open——过滤异常只记日志不阻塞主流程（`request-filter.ts:21-24`）；regex 用 `safe-regex` 做 ReDoS 校验，不安全直接跳过（`request-filter-engine.ts:243,391-406`）；路径解析拒绝 `__proto__/constructor/prototype` 防原型污染（`request-filter-engine.ts:30,42`）。

**body 路径操作**: 自实现 `parsePath`/`setValueByPath`/`getValueByPath`/`deleteByPath`，支持点号 + `[index]` 数组下标（`request-filter-engine.ts:36-121`）。

### 2.2 响应覆写（Response Override）

仓库**没有**通用的"任意成功响应改写"，响应侧覆写分两块：

1. **错误响应覆写**（error_rules 的一部分）: 命中 error rule 时用 `override_response`（JSONB，整段错误体）+ `override_status_code` 替换上游错误。在 `error-handler.ts:281-430` 应用：支持仅覆写状态码 / 覆写整段 body / 二者皆有；运行时再校验 status 400-599 与 body 格式合法性（双重保护防脏数据）。
   - 引用: `src/app/v1/_lib/proxy/error-handler.ts:283-296,318-410`。
2. **响应整流**（ResponseFixer，见 2.5）。

### 2.3 数据脱敏（Redaction）

仓库的脱敏**只服务于日志/审计落库展示，不改写转发给上游/客户端的内容**：

- `src/lib/utils/message-redaction.ts`: 把 `messages[].content`、`system`、Gemini `parts`、tool_use/tool_result 等替换为 `[REDACTED]`，保留结构。用于日志存储时隐藏消息正文。
- `src/lib/audit/redact.ts`: `redactSensitive()` 深拷贝并把 key 名命中敏感集合（`key/apikey/api_key/password/secret/token/authorization/webhook_secret` 等，小写比较）的字段值替换为 `[REDACTED]`，用于审计快照。**按 key 名**脱敏，不是按内容正则。
  - 引用: `src/lib/utils/message-redaction.ts:8,20-80`、`src/lib/audit/redact.ts:4-15,42-67`。

### 2.4 动态注入（Dynamic Injection）

两条路径:

1. **request_filters advanced operations DSL**（`src/lib/request-filter-types.ts`）——可声明式向 body 注入/合并/插入字段:
   - `SetOp`: `path + value + writeMode(overwrite|if_missing)`
   - `MergeOp`（body）: 深合并，`value` 中 `null` 表示删除该 key
   - `InsertOp`（body 数组）: `position(start|end|before|after)` + `anchor`（FilterMatcher）+ `onAnchorMissing` + `dedupe.byFields`
   - `RemoveOp`: 按 path 或 array `matcher` 删除
   - 执行: `request-filter-engine.ts` `executeAdvancedOps` / `deepMerge`（`request-filter-engine.ts:158-175`）/ `matchElement`（178-218）。
   - 引用: `src/lib/request-filter-types.ts:13-99`。
2. **专用 rectifier 注入**: 如 `enableClaudeMetadataUserIdInjection`（为 Claude 请求补 `metadata.user_id` 提升缓存命中）、`enableCodexSessionIdCompletion`（补 session_id/prompt_cache_key）。
   - 引用: `src/types/system-config.ts:116-122`。

### 2.5 请求/响应整流器（Rectifier / ResponseFixer）

**请求侧 rectifier**（发往上游前主动改写请求体，每个独立文件、各自 SystemSettings 开关）:
- `thinking-signature-rectifier.ts` / `thinking-budget-rectifier.ts` / `thinking-effort-conflict-rectifier.ts`: thinking 相关 400 自动整流并**重试一次**（reactive，错误触发）。
- `billing-header-rectifier.ts`: proactive，发送前剥离 Claude Code 注入的 `x-anthropic-billing-header` system 文本块（正则 `/^\s*x-anthropic-billing-header\s*:/i`，原地 mutate message）。
- `response-input-rectifier.ts`: `/v1/responses` 非数组 input 规范化为数组。
  - 引用: `src/app/v1/_lib/proxy/billing-header-rectifier.ts:1-40`、`src/types/system-config.ts:86-107`。

**响应侧整流 ResponseFixer**（`src/app/v1/_lib/proxy/response-fixer/`）:
- `EncodingFixer`（修复编码）、`SseFixer`（修复 SSE 格式）、`JsonFixer`（修复截断 JSON），配置 `ResponseFixerConfig { fixTruncatedJson, fixSseFormat, fixEncoding, maxJsonDepth, maxFixSize }`。
- 默认全开（`response-fixer/index.ts:19-25` 的 `DEFAULT_CONFIG`），整体由 `enableResponseFixer` 全局开关 + 子项布尔控制。
  - 引用: `src/app/v1/_lib/proxy/response-fixer/index.ts:9-25`。

### 2.6 内容过滤（密钥/邮箱）— ❌ 无专门实现

搜索证据: 仓库 `src/lib/` 下匹配 `filter|sensitive|redact|mask|sanitiz` 的文件已枚举（见"关键文件"），**没有**针对请求内容做"密钥/邮箱正则识别并拦截/替换"的中间件。最接近的是:
- `audit/redact.ts` 按 **key 名**脱敏（非正则内容）；
- `request_filters` 的 `text_replace + matchType=regex` 可由管理员**手动配置**正则替换 body 文本（如自行写邮箱/密钥正则做替换），但仓库**未内置**这类规则。
→ aidog 若要"内置密钥/邮箱内容过滤"，需自建，可借鉴 sensitive-word-detector 的分桶检测结构。

### 2.7 错误规则检测（Error Rule Detection）

**数据流**: `error_rules` 表 → `ErrorRuleDetector` 单例（regex/contains/exact 分桶 + safe-regex ReDoS 校验 + 默认规则同步）→ forwarder 在收到上游错误时调用 `getErrorDetectionResultAsync(error)` 分类。

- **检测顺序**（性能优先）: contains → exact → regex（`error-rule-detector.ts:394-447`）。
- **作用**: ① 把上游错误归类（`category`: prompt_limit / content_filter / pdf_limit / thinking_error / parameter_error / invalid_request / cache_limit）；命中 NON_RETRYABLE_CLIENT_ERROR 的白名单错误**立即返回不重试、不计熔断**（forwarder.ts:1798-1851）。② 提供 override_response / override_status_code（见 2.2）。
- **默认规则**: `is_default=true` 的内置规则可通过 cache-refresh 同步（insert/update/skip custom/delete stale，见 `error-rules.ts` schema 的 `ErrorRulesCacheRefreshResponseSchema.syncResult`）。
  - 引用: `src/lib/error-rule-detector.ts:14,196-351,379-450`、`src/app/v1/_lib/proxy/forwarder.ts:74,1801,4287`、`src/lib/api/v1/schemas/error-rules.ts:4-14,84-94`。

### 2.8 敏感词检测（Sensitive Word Detection）

**数据流**: `sensitive_words` 表 → `SensitiveWordDetector` 单例（contains 数组 / exact Set / regex 列表）→ GuardStep `sensitive`（在 auth 之后、最早期执行，**计费前拦截**）。

- 命中即返回 400 错误 Response 拦截，并异步写 `message_request`（`provider_id=0`、`cost_usd=0`、`blocked_by=sensitive_word`、`blocked_reason`），**不计费**。
- fail-open: 检测异常时放行（`sensitive-word-guard.ts:71-74`）；缓存空快速放行（`isEmpty()`）。
- 检测顺序 contains→exact→regex，统一 lowercase。
  - 引用: `src/app/v1/_lib/proxy/sensitive-word-guard.ts:30-148`、`src/lib/sensitive-word-detector.ts:106-196,234-244`、`guard-pipeline.ts:110-115,201-203`（`sensitive` 排在 `auth` 之后）。

---

## 3. 规则配置数据结构（字段 / 类型 / 正则·通配支持）

### 3.1 `request_filters` 表（`src/drizzle/schema.ts`）

| 字段 | 类型 | 说明 / 默认 |
|---|---|---|
| id | serial PK | |
| name | varchar(100) NOT NULL | |
| description | text | |
| scope | varchar(20) `'header'\|'body'` NOT NULL | |
| action | varchar(30) `'remove'\|'set'\|'json_path'\|'text_replace'` NOT NULL | |
| matchType | varchar(20) `'regex'\|'contains'\|'exact'`（可空） | 仅 text_replace 用 |
| target | text NOT NULL | header 名 / body JSON path / 文本匹配目标 |
| replacement | jsonb（可空） | 替换值 |
| priority | integer NOT NULL default 0 | 越小越先 |
| **isEnabled** | boolean NOT NULL **default true** | per-rule 开关 |
| bindingType | varchar(20) `'global'\|'providers'\|'groups'` NOT NULL default `'global'` | 作用范围 |
| providerIds | jsonb `number[]` | bindingType=providers 时 |
| groupTags | jsonb `string[]` | bindingType=groups 时 |
| ruleMode | varchar(20) `'simple'\|'advanced'` NOT NULL default `'simple'` | |
| executionPhase | varchar(20) `'guard'\|'final'` NOT NULL default `'guard'` | 挂载阶段 |
| operations | jsonb `FilterOperation[]` | advanced 模式 DSL |
| createdAt/updatedAt | timestamptz defaultNow | |

索引: `(is_enabled, priority)`、`(scope)`、`(action)`、`(is_enabled, binding_type)`、`(is_enabled, execution_phase)`。
**正则/通配**: 支持 `matchType=regex`（text_replace 全局替换，预编译 + safe-regex 校验）；advanced `FilterMatcher.matchType` 支持 exact/contains/regex；JSON path 支持点号+数组下标通配定位。
引用: `src/drizzle/schema.ts` `request_filters` 块；zod schema `src/lib/api/v1/schemas/request-filters.ts:4-47`。

### 3.2 `sensitive_words` 表

| 字段 | 类型 | 默认 |
|---|---|---|
| id | serial PK | |
| word | varchar(255) NOT NULL | 词或正则 |
| matchType | varchar(20) NOT NULL **default 'contains'** | `'contains'\|'exact'\|'regex'` |
| description | text | |
| **isEnabled** | boolean NOT NULL **default true** | |
| createdAt/updatedAt | timestamptz | |

索引 `(is_enabled, match_type)`。正则: regex 类型用 `new RegExp(word, "i")`。
引用: `src/drizzle/schema.ts` `sensitive_words` 块；`src/lib/api/v1/schemas/sensitive-words.ts:4-16`。

### 3.3 `error_rules` 表

| 字段 | 类型 | 默认 |
|---|---|---|
| id | serial PK | |
| pattern | text NOT NULL | 错误消息匹配模式 |
| matchType | varchar(20) NOT NULL **default 'regex'** | `'regex'\|'contains'\|'exact'` |
| category | varchar(50) NOT NULL | prompt_limit/content_filter/pdf_limit/thinking_error/parameter_error/invalid_request/cache_limit |
| description | text | |
| overrideResponse | jsonb（可空） | 整段错误响应覆写（Claude/Gemini/OpenAI 格式自动识别） |
| overrideStatusCode | integer（可空） | null=透传上游状态码（写时校验 400-599） |
| **isEnabled** | boolean NOT NULL **default true** | |
| isDefault | boolean NOT NULL default false | 内置规则标记 |
| priority | integer NOT NULL default 0 | |
| createdAt/updatedAt | timestamptz | |

索引 `(is_enabled, priority)`、`unique(pattern)`、`(category)`、`(match_type)`。regex 经 `safe-regex` ReDoS 过滤。
引用: `src/drizzle/schema.ts` `error_rules` 块；`src/lib/api/v1/schemas/error-rules.ts:4-43`。

---

## 4. 开关机制（全局 / per-rule / 默认值）

**两套模型**:

1. **规则类（request_filters / sensitive_words / error_rules）= per-rule `is_enabled`（默认 true），无独立全局开关**。
   - "全局关停"靠：删除/禁用所有规则，或缓存为空时检测器 `isEmpty()` 快速放行（`sensitive-word-detector.ts:234`、`request-filter-engine.ts:479,503`）。
   - request_filters 额外有 `bypassRequestFilters`（endpoint policy 级旁路，特定端点如 raw passthrough 跳过，`request-filter.ts:15`）。
   - 热更新：写库后通过 eventEmitter + Redis pub/sub 触发 detector reload，无需重启（各 `setupEventListener`）。

2. **整流器类 = SystemSettings 全局布尔开关，写在 `system_settings` 表，多数默认开启**:
   - `enableThinkingSignatureRectifier` default **true**
   - `enableThinkingBudgetRectifier` default **true**
   - `enableThinkingEffortConflictRectifier` default **true**
   - `enableBillingHeaderRectifier` default **true**（schema 注释）
   - `enableResponseInputRectifier` default **true**
   - `enableCodexSessionIdCompletion` default **true**
   - `enableClaudeMetadataUserIdInjection` default **true**
   - `enableResponseFixer` default **true** + `responseFixerConfig` 子开关（fixTruncatedJson/fixSseFormat/fixEncoding 默认全开）
   - 引用: `src/types/system-config.ts:86-126`、`src/drizzle/schema.ts` `system_settings` 块 `:122-127`（节选）+ tool-result `bnrhsmfjh.txt`。

→ **与 aidog 用户需求"开关在系统设置中可启用/关闭，默认开启"最贴近的是整流器类的 SystemSettings 布尔开关模式**；而规则增删本身走 per-rule is_enabled。

---

## 5. 对 aidog（Rust 网关）的可借鉴点与不适用点

### 可借鉴点
1. **GuardPipeline 有序 step + 提前返回**: 用 Rust trait（`trait GuardStep { async fn execute(&self, session) -> Option<Response> }`）+ `Vec<Box<dyn GuardStep>>` 复刻，预设管线（full / passthrough）。这是最干净的"中间件"抽象，对应 aidog `gateway/proxy.rs` 的请求链路。
2. **规则三类匹配 contains→exact→regex 分桶缓存**: 性能优先顺序 + 单例热缓存，对应 aidog 可在 `gateway/db.rs` 读规则、内存 `RwLock<Cache>` 缓存、settings 变更时 reload。
3. **per-rule is_enabled + 缓存空快速放行 + fail-open**: 规则功能不需要额外全局开关，"无启用规则即零开销"。aidog 可同样在 SQLite 存规则、空表即跳过。
4. **整流器全局布尔开关 + 默认 true**: 直接对应 aidog 用户"系统设置开关、默认开启"。aidog 在 settings 表加 bool 列即可（参考 `ProxyLogSettings` 已有的 settings 模式）。
5. **错误规则 override_response/override_status_code**: aidog 已有平台失败重试 + 三态 status（见 memory `platform-retry-failover`），可在此基础上加"错误模式匹配→覆写响应/状态码"，复用 NON_RETRYABLE 立即返回的思路。
6. **request_filters advanced operations DSL（set/merge/insert/remove + JSON path）**: 是"动态注入"的通用方案，比 aidog 现有的零散 converter 改写更声明式。可作为 aidog `converter.rs` 旁路的可配置改写层。
7. **三阶段挂载（global guard / per-provider / final-before-send）**: aidog 已有 router 选平台、converter 转换，可在选平台前/后/序列化前对应插入三个改写点。
8. **safe-regex ReDoS 防护 + 原型污染防护**: Rust 用 `regex` crate（线性时间，天然抗 ReDoS，无需 safe-regex 等价物），原型污染问题在 Rust/serde_json 下不存在——这是 aidog 的优势。

### 不适用 / 需改造点
1. **语言/运行时差异**: 该仓库是 TS+Next.js+Postgres+Redis 多实例；aidog 是单进程 Rust+SQLite+Tauri 桌面应用，**不需要 Redis pub/sub 跨进程失效**，直接内存缓存 + 写后 reload 即可。
2. **数据脱敏不是转发链路改写**: 仓库的 redaction 只用于日志展示，aidog 若要"转发前脱敏"需新设计（仓库无现成参考）。aidog 已有 `aidog-request-inspect` headers 自动脱敏属同类"展示侧脱敏"。
3. **内容过滤（密钥/邮箱）该仓库未内置**: 无现成代码可抄，需 aidog 自建（建议复用 sensitive-word 分桶检测 + 内置常见密钥/邮箱正则）。
4. **JSON path 手写解析**: TS 因无类型 JSON 操作需手写 `parsePath`；Rust 下用 `serde_json::Value` + `pointer()`（RFC 6901）或自写点号解析，行为需对齐数组下标语义。
5. **NON_RETRYABLE 白名单 + 熔断器**: 仓库有完整熔断器（circuit breaker），aidog 当前是平台重试/auto_disabled，移植错误规则时要决定是否引入熔断概念，避免半套实现。
6. **fake-streaming / WebSocket / HTTP2** 等是该仓库特有的上游兼容能力，与"中间件"主题无关，移植时排除。

---

## 6. 关键文件路径 + 引用（owner/repo/path:line）

管线与守卫:
- `ding113/claude-code-hub/src/app/v1/_lib/proxy/guard-pipeline.ts:23-50`（GuardStep/GuardConfig 接口）、`:53-143`（step 适配器）、`:200-227`（CHAT/RAW 预设）
- `.../src/app/v1/_lib/proxy/request-filter.ts:13-26`（global guard step，fail-open）
- `.../src/app/v1/_lib/proxy/provider-request-filter.ts:10-33`（per-provider step）
- `.../src/app/v1/_lib/proxy/sensitive-word-guard.ts:30-67`（拦截）、`:80-116`（blocked 落库不计费）

规则引擎/检测器:
- `.../src/lib/request-filter-engine.ts:36-121`（path 工具）、`:158-218`（deepMerge/matchElement）、`:386-454`（分桶加载）、`:478-545`（applyGlobal/applyForProvider）、`:560-625`（applyFinal + 收集）
- `.../src/lib/request-filter-types.ts:13-99`（advanced operations DSL）
- `.../src/lib/sensitive-word-detector.ts:106-196`（reload+detect 分桶）、`:234-244`（isEmpty/单例）
- `.../src/lib/error-rule-detector.ts:196-351`（reload+ReDoS+原子替换）、`:379-450`（detect）

整流 / 错误覆写:
- `.../src/app/v1/_lib/proxy/forwarder.ts:1801,4287`（getErrorDetectionResultAsync 接入）、`:2424,2464,2748,2812,2858`（applyFinal 接入点）
- `.../src/app/v1/_lib/proxy/error-handler.ts:283-296,318-410`（override_response/override_status_code 应用 + 校验）
- `.../src/app/v1/_lib/proxy/response-fixer/index.ts:9-25`（ResponseFixer 组合 + DEFAULT_CONFIG）
- `.../src/app/v1/_lib/proxy/billing-header-rectifier.ts:1-40`（proactive rectifier 范例）

脱敏:
- `.../src/lib/utils/message-redaction.ts:8,20-80`（消息正文 [REDACTED]）
- `.../src/lib/audit/redact.ts:4-15,42-67`（按 key 名脱敏）

数据结构:
- `.../src/drizzle/schema.ts`（`request_filters` / `sensitive_words` / `error_rules` / `system_settings` 表定义）
- `.../drizzle/0024_request-filters.sql`（请求过滤器初始迁移）
- `.../src/lib/api/v1/schemas/{request-filters,sensitive-words,error-rules}.ts`（zod 校验 + 默认值）
- `.../src/types/system-config.ts:8-146`（SystemSettings + 整流器全局开关 + 默认值）

管理端（如需参考 UI/交互）:
- `.../src/app/[locale]/settings/{request-filters,sensitive-words,error-rules}/`（配置页，含 regex-tester / error-rule-tester / override-section）
- `.../src/app/api/v1/resources/{request-filters,sensitive-words,error-rules}/handlers.ts`
- `.../src/actions/{request-filters,sensitive-words,error-rules,proxy-status}.ts`

---

## Caveats / 未覆盖

- 行号基于 2026-06-13 抓取的 `main` HEAD，仓库活跃（0105 号迁移），后续行号可能漂移；引用以文件+符号名为准更稳妥。
- `forwarder.ts`(5340 行) / `response-handler.ts`(4553 行) 仅做了针对性 grep + 局部阅读，未全文通读；`applyFinal` 在 Gemini/OpenAI/Anthropic 各分支重复挂载，未逐分支核对每个挂载点的 body clone 细节。
- `error-rules` 的"默认内置规则集"具体内容（哪些 pattern→哪个 category）未抓取（在 repository 层 seed，未读 `src/repository/error-rules.ts`）。如需移植内置规则清单需补查。
- `response-fixer` 三个子 fixer（encoding/sse/json）的具体修复算法未逐一通读，仅确认其职责与配置开关。
- 未读管理端 React 组件实现细节（仅确认存在 regex-tester / error-rule-tester / override-section 等交互组件）。
- "内容过滤（密钥/邮箱）"经 `src/lib` 全量文件名搜索（filter/sensitive/redact/mask/sanitiz）+ 阅读 redaction 文件确认**无内置正则内容过滤器**；属负向结论，已给出搜索证据。
