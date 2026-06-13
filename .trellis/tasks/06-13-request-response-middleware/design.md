# Design — 请求/响应中间件规则引擎

> 架构契约文档。dispatch 的 trellis-implement 据此落地。PRD 见 `prd.md`。

## 模块表

| 模块 | 文件 | 执行层 | 职责 | 资源边界 |
| --- | --- | --- | --- | --- |
| 数据模型 | `models.rs` | main/worktree | MiddlewareRule / MiddlewareSettings / 枚举 / BreakerState | 与现有 struct 同文件，追加不改 |
| 引擎核心 | `middleware.rs`(新) | main/worktree | MiddlewareEngine（缓存/作用域解析/regex 预编译/检测器/熔断器） | 新文件，无冲突 |
| 持久化 | `db.rs` | main/worktree | `middleware_rule` 表 DDL + CRUD + seed + settings CRUD | 与现有 CRUD 同文件 |
| 入站挂载 | `proxy.rs` | main/worktree | parse 后 apply 入站规则；拦截即返回写审计 | 入站段（~638-704） |
| 出站挂载 | `proxy.rs` | main/worktree | forward 后 apply 出站规则；错误分类喂重试（熔断归 group 树）；流式逐块 | 出站段 + SSE 转发 |
| commands | `lib.rs` | main/worktree | 规则 CRUD + settings get/set commands 注册 | invoke_handler 列表 |
| 前端 API | `services/api.ts` | sub-agent(并行) | TS 类型 + invoke 封装（**契约由 S1 冻结**） | 前端独立 |
| 前端 UI | `AppSettings.tsx` + `components/settings/**` | sub-agent(并行) | 中间件 tab + group/platform 嵌入 | 前端独立 |
| i18n | i18n 资源 | sub-agent(并行) | 7 语言 key | 前端独立 |

## 数据模型（契约 — Rust ↔ TS 字段名必须一致，见 guides/cross-layer-rules）

### `middleware_rule` 表

```sql
CREATE TABLE IF NOT EXISTS middleware_rule (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  name         TEXT NOT NULL,
  description  TEXT NOT NULL DEFAULT '',
  rule_type    TEXT NOT NULL,        -- request_filter|sensitive_word|redaction|content_filter|dynamic_injection|response_override|rectifier|error_rule
  scope        TEXT NOT NULL DEFAULT 'global',  -- global|group|platform
  scope_ref    TEXT NOT NULL DEFAULT '',        -- group_name | platform_id(字符串) | ''(global)
  match_type   TEXT NOT NULL DEFAULT 'contains',-- regex|contains|exact
  pattern      TEXT NOT NULL DEFAULT '',        -- 匹配模式/目标 path/header 名
  action       TEXT NOT NULL DEFAULT 'warn',    -- mask|block|warn|inject|override|classify
  config       TEXT NOT NULL DEFAULT '{}',      -- JSON，type-specific（见下）
  priority     INTEGER NOT NULL DEFAULT 0,      -- 越小越先
  enabled      INTEGER NOT NULL DEFAULT 1,
  is_builtin   INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_mw_rule_lookup ON middleware_rule(enabled, rule_type, scope);
```

### config JSON（按 rule_type）
- `redaction`/`content_filter`/`response_override`(成功体)：`{ "replacement": "****", "fields": ["messages","system"] }`
- `dynamic_injection`：`{ "inject_mode":"system_append|header_set|body_set", "target":"...", "value":"..." }`
- `error_rule`：`{ "category":"prompt_limit|content_filter|...", "override_status": 400, "override_body": {...}, "retryable": false }`
- `rectifier`：`{ "fix":"sse|json|encoding|field_default", "target":"...", "default":... }`
- `request_filter`：`{ "field":"model|...", "op":"reject|allow|set", "value":... }`
- `sensitive_word`：空（pattern 即词；action 默认 block）

### Rust 枚举（models.rs）
`RuleType` / `RuleScope` / `MatchType` / `RuleAction` 各 `#[serde(rename_all="snake_case")]`，与 TS 字面量联合类型对齐。

### MiddlewareSettings（settings KV，scope=`middleware` key=`settings`）
```rust
pub struct MiddlewareSettings {
  pub enabled: bool,                 // 总开关，default true
  pub type_toggles: HashMap<String,bool>, // 按 rule_type 子开关，缺省视为 true
}
// Default: enabled=true, type_toggles 全 true
```

> **熔断器已移出本树** → 归入新 parent 树 `06-13-group-scheduling-breaker`（group 功能块，组内配置 + 默认值）。本中间件树不含 BreakerConfig/BreakerState。error_rule 仅产出 retryable/non-retryable 标记喂现有重试编排，熔断由 group 树消费这些标记 + auto_disabled 状态解耦实现。

## 作用域解析（就近覆盖）

```
resolve_rules(rule_type, group_name, platform_id):
  platform 层规则(scope=platform, scope_ref=platform_id) 非空 → 用之
  否则 group 层(scope=group, scope_ref=group_name) 非空 → 用之
  否则 global 层
  → 同 rule_type 内只生效最细粒度存在的那一层（CSS 级联语义）
```
> 注意"非空"判定按"该层该类型是否存在 enabled 规则"。脱敏类同样就近覆盖（用户已选"就近覆盖"，非累加）。

## 执行流程

### 入站（proxy.rs，chat_req 就绪后 ~638 行）
```
若 settings.enabled == false → 跳过
解析 group_name；platform 作用域规则在路由选定候选后按 platform_id 解析
顺序：request_filter → sensitive_word → redaction → content_filter → dynamic_injection
  - block 命中：写 proxy_log(blocked_by/blocked_reason)，不计费，立即返回 4xx
  - mask 命中：原地改写 chat_req.messages/system
  - inject：按 inject_mode 注入
  - warn：仅 tracing 告警
fail-open：单条规则 panic/regex err → catch + 记日志 + 继续
```
全局/group 规则在路由前应用；platform 规则在候选选定后、convert_request 前应用。

### 出站（proxy.rs，forward 返回后）
```
非流式：拿到完整 body
  - error_rule：状态码非 2xx 时分类 → 标记 retryable/non-retryable（喂给现有重试循环）+ override_status/body
  - response_override/redaction/content_filter(出站)：改写 body
流式 SSE：转发每个 chunk 时
  - 对 chunk 文本应用 mask/override/sensitive（逐块正则替换）
  - error_rule 按首 chunk / HTTP 状态码判定
  - 跨块边界匹配可能漏（已知限制，design 备注；可选滑窗后续）
```
> 熔断器移出本树（归 group 树 `06-13-group-scheduling-breaker`）。error_rule 仅产出 retryable/non-retryable 标记 + 现有 auto_disabled 状态，group 树的熔断/调度消费这些信号决定候选过滤。

## 契约冻结（S1 产出，S5 消费）
S1 完成时在 `services/api.ts` 写好：`MiddlewareRule`/`MiddlewareSettings` TS 类型 + `middlewareApi.{listRules,createRule,updateRule,deleteRule,getSettings,setSettings}` invoke 封装。S5 仅消费，不改契约。命名 snake_case ↔ camelCase 转换遵循项目现有 api.ts 惯例（核对现有 settings 封装）。

## 资源边界 / 并行决策
- 后端 S1→S2→S3→S4 **串行**（共享 proxy.rs/db.rs/models.rs/middleware.rs）。
- S5 前端仅碰 src/ 前端文件，与后端零交集 → S1 冻结契约后**并行 sub-agent**。
- 全程单 worktree `.trellis/worktrees/06-13-request-response-middleware`。

## Rollback
总开关 OFF = 全旁路。worktree 未合并前 master 不受影响。内置规则可逐条禁用。
