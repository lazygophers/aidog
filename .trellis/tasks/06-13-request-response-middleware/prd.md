# 请求/响应中间件规则引擎

## Goal

为 aidog 网关在请求入站与上游响应两处挂载一套**配置驱动的中间件规则引擎**，落地 8 类能力（请求过滤器、敏感词检测、数据脱敏、内容过滤(密钥/邮箱)、动态注入、响应覆写、请求响应整流器、错误规则检测），规则支持全局/group/platform 三级作用域（就近覆盖）+ 内置预设与用户自定义，命中后可脱敏改写/拦截拒绝/仅告警，开关在系统设置中可启停且**默认开启**。完成后：系统设置出现"中间件"tab；密钥/邮箱默认被脱敏；敏感词请求被拦截写审计日志；上游错误按规则分类并喂给现有重试编排（熔断器另见 group 树 `06-13-group-scheduling-breaker`）。

## What I already know

### 现状
- aidog = Tauri 2.0 + React 19 + Rust 单进程网关 + SQLite（`~/.aidog/aidog.db`）。
- proxy 链路（proxy.rs）：`parse_incoming_request`(629) → router 选平台 → 透传判定(746) / `convert_request`(839) → forward 上游 → 响应回客户端。
- **入站挂载点** = parse 后 / convert 前，操作 `ChatRequest`（含 messages/system/headers）。
- **出站挂载点** = forward 返回后 / 回客户端前；流式有 StreamAggregator 旁路（memory `streaming-sse-log-aggregation`）。
- 设置 = KV store（`SettingEntry` scope/key/value:json，models.rs:626）；已有 `ProxyLogSettings`/`ProxyTimeoutSettings`/`ProxyClientSettings` 模式（models.rs + db.rs CRUD + lib.rs command + services/api.ts + 设置 UI）。
- 重试体系：多平台失败重试 + 401/403 auto_disabled + 指数退避（memory `platform-retry-failover`）。
- 系统设置 UI = AppSettings.tsx（tab 式）+ components/settings/ 子组件；i18n 7 语言（zh-CN/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP，含 RTL）。
- proxy_log 已有 blocked 类字段惯例可借鉴（需确认列）；Protocol 枚举 62 变体。

### 调研结论
- [`research/reference-repo-middleware.md`](research/reference-repo-middleware.md) — ding113/claude-code-hub（TS/Next.js/Bun/Drizzle/Postgres）把 8 类能力拆两套机制：**GuardPipeline**（有序 step + 可提前返回拦截）+ **Rectifier/ResponseFixer/ErrorRule**（整流覆写）。关键移植点：
  - 规则三表 `request_filters`/`sensitive_words`/`error_rules`，per-rule `is_enabled` default true；整流器类走 SystemSettings 全局布尔默认 true（aidog "默认开启"的最佳对照）。
  - 检测顺序 contains→exact→regex（性能优先）；fail-open；safe-regex ReDoS 校验。
  - **内容过滤(密钥/邮箱)** 该仓库未内置 → aidog 需自建（复用敏感词分桶检测结构 + 内置正则）。
  - aidog 单进程：**不需要** Redis 跨进程失效，内存缓存 + 写后 reload 即可；Rust `regex` crate 天然抗 ReDoS。

## Assumptions (temporary)
- 8 类能力统一抽象为单表 `middleware_rule`（rule_type 区分），而非参考仓库的三张分表 —— Rust+SQLite 下统一表 + JSON config 更省样板、便于三级作用域统一查询。
- 总开关 + 按 rule_type 子开关存于 settings KV（scope=`middleware`）；规则数据存 `middleware_rule` 表。
- 熔断器不在本树（移至 group 树）；error_rule 仅产出 retryable/non-retryable 标记。

## Open Questions
无（范围已明确，全部经 AskUserQuestion 锁定，见 Decision）。

## Deliverable 矩阵

| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| D1 | 规则引擎基座（models + 表 + CRUD + 内存缓存 + 热更新 + 三级作用域解析 + ReDoS 防护 + commands） | diff | `cargo test` 引擎单测过；commands 在 lib.rs 注册；规则增改后缓存 reload | P0 |
| D2 | 入站规则执行（请求过滤器/敏感词/脱敏/内容过滤/动态注入 挂载 proxy 入站） | diff | 集成测试：含密钥/邮箱请求被脱敏；敏感词请求被拦截返回 + 写审计；fail-open 验证 | P0 |
| D3 | 出站规则执行（响应覆写/整流/错误规则 + 流式逐块改写；熔断器移至 group 树） | diff | 集成测试：响应密钥被脱敏；错误按规则分类喂重试 | P0 |
| D4 | 内置预设规则集（密钥/邮箱/手机正则 + 默认 error_rules，is_builtin=1，默认开启） | diff | 首次启动 seed；`cargo test` 验证内置规则命中样例文本 | P0 |
| D5 | 前端 UI（AppSettings 新"中间件"tab 全局规则 + group/platform 页内嵌 + api.ts 类型 + 7 语言 i18n） | UI | `yarn build` 通过；tab 可见可增删改规则；总开关默认 ON；7 语言无缺键 | P0 |

## Requirements

- **R1**(D1) 单表 `middleware_rule`：id/name/description/rule_type/scope/scope_ref/match_type/pattern/action/config(JSON)/priority/enabled/is_builtin/timestamps。
- **R2**(D1) rule_type 枚举 8 类；action 枚举 mask/block/warn/inject/override/classify；scope 枚举 global/group/platform。
- **R3**(D1) 内存缓存按 (scope, rule_type) 分桶 + 预编译 regex；写库后 reload，免重启。
- **R4**(D1) 三级作用域**就近覆盖**：同 rule_type 下 platform 规则存在则盖 group、group 盖 global（解析出生效规则集）。
- **R5**(D1) ReDoS 防护：用户自定义正则编译失败/超限 → 跳过 + 记日志（fail-open）。
- **R6**(D2) 入站执行顺序：请求过滤器 → 敏感词 → 脱敏/内容过滤 → 动态注入；拦截类命中立即返回，不发上游、不计费、写审计日志。
- **R7**(D2/D3) fail-open：任一规则执行异常放行主链路，仅记日志。
- **R8**(D3) 出站执行：响应覆写/整流 + 错误规则分类；错误规则产出 retryable/non-retryable 标记喂给现有重试编排（熔断由 group 树消费，本树不做）。
- **R10**(D3) 流式 SSE 逐块改写：对每个 chunk 应用脱敏/覆写/敏感词；错误检测按首块/状态码判定。
- **R11**(D4) 内置预设：密钥（常见 API key 模式）、邮箱、手机号正则 + 默认 error_rules，is_builtin=1 默认 enabled，可被用户禁用但不可删（或软删）。
- **R12**(D5) 系统设置"中间件"tab：总开关（默认 ON）+ 按 rule_type 子开关 + 全局规则增删改查 UI。
- **R13**(D5) group/platform 编辑页内嵌该作用域规则管理。
- **R14**(D5) 7 语言 i18n 全覆盖，新增 key 无缺失，阿拉伯语 RTL 正常。

## Child Task Map

本 task 为 **parent**，按分层拆 5 个独立 child（各自 plan/impl/check/archive）。Deliverable → child 一一对应。共享架构契约见本目录 `design.md`（child 据此落地）。

| Child | Slug | Deliverable | 交付物 | 独立验收 | 依赖 | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | `06-13-mw-rule-engine-core` | D1 | 规则引擎基座 + 冻结 api.ts 契约 | `cargo test` 引擎/作用域/缓存单测过；CRUD commands 注册 | 无（先行） | planning |
| C2 | `06-13-mw-inbound-execution` | D2 | 入站 5 类规则执行 + 拦截审计 | 集成测试：密钥/邮箱脱敏；敏感词拦截写审计；fail-open | C1 | planning |
| C3 | `06-13-mw-outbound-breaker` | D3 | 出站规则 + 流式逐块（熔断器移至 group 树） | 集成测试：响应脱敏(含流式)；错误分类喂重试 | C1 | planning |
| C4 | `06-13-mw-builtin-presets` | D4 | 内置预设规则集 seed | 首启 seed；内置正则命中样例单测 | C1 | planning |
| C5 | `06-13-mw-frontend-ui` | D5 | 前端 UI + i18n | `yarn build` 过；tab 增删改；7 语言无缺键 RTL 正常 | C1(仅契约) | planning |

### Child 调度图

```mermaid
flowchart LR
    C1[C1 · 规则引擎基座/冻结契约] --> C2[C2 · 入站执行]
    C1 --> C3[C3 · 出站规则+流式]
    C1 --> C4[C4 · 内置预设]
    C1 -.冻结 api.ts 契约.-> C5[C5 · 前端 UI+i18n]
    C2 --> G1{{G1 · 后端集成验收}}
    C3 --> G1
    C4 --> G1
    C5 --> G2{{G2 · 端到端验收}}
    G1 --> G2
    classDef serial fill:#fff3e0,stroke:#e65100;
    classDef parallel fill:#e0f7fa,stroke:#006064;
    class C1 serial
    class C2,C3,C4 serial
    class C5 parallel
```

> 资源互斥（child 间依赖写在各自 child PRD，不靠目录隐含）：C2/C3 同改 `proxy.rs`、C1/C4 同改 `db.rs`、C1/C2/C3 同改 `middleware.rs` → 后端 child **串行执行**（C1→C2→C3→C4）。C5 仅碰前端文件，与后端零交集，C1 冻结 `api.ts` 契约后**可并行**。

## 跨 Child 验收

- [ ] C1 的 `middleware_rule` 表 + 缓存被 C2/C3/C4 正确消费（同一 MiddlewareEngine 实例）。
- [ ] C1 冻结的 `api.ts` 契约被 C5 正确消费（字段名 snake↔camel 一致，无契约漂移）。
- [ ] C2 入站 + C3 出站在同一请求链路上不互踩（入站脱敏后出站再脱敏幂等）。
- [ ] C4 内置规则在 C2/C3 执行链路中生效（密钥/邮箱默认被脱敏）。
- [ ] 总开关 OFF 时 C2/C3 全旁路；fail-open 在任一 child 规则异常时不阻断主链路。
- [ ] C3 error_rule 产出的 retryable/non-retryable 标记被现有重试编排正确消费（熔断在 group 树验收）。

## 集成 Review

所有 child 完成后，parent 跑一次端到端 review：
```bash
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
cd .. && yarn build
```
- 端到端手测：起 `yarn tauri dev`，中间件 tab 总开关默认 ON；构造含密钥/邮箱请求验证脱敏；构造敏感词验证拦截 + 审计；构造上游错误验证 error_rule 分类与重试。

## Acceptance Criteria

- [ ] D1：`cargo test` 引擎/作用域/缓存单测通过；规则 CRUD commands 注册可调。
- [ ] D2：含密钥/邮箱的请求发出后上游收到的是脱敏版；含敏感词请求被拦截返回错误 + proxy_log 写 blocked 记录、不计费。
- [ ] D3：上游响应含密钥时回客户端为脱敏版（含流式）；上游错误按 error_rule 正确分类并喂给重试编排（non-retryable 立即返回）。
- [ ] D4：全新 db 首启 seed 内置规则；内置密钥/邮箱正则命中样例文本（单测）。
- [ ] D5：`yarn build`（tsc && vite build）通过；中间件 tab 总开关默认 ON 可增删改规则；group/platform 页可管理本作用域规则；7 语言无缺键、ar-SA RTL 正常。
- [ ] 一致性：总开关 OFF 时入站/出站均跳过；fail-open 在规则异常时不阻断主链路（单测/手测）。

## Definition of Done
- 全部 Requirements 实现 + Acceptance Criteria 勾选。
- `cargo build` + `cargo clippy`（warning 清零）+ `cargo test` + `yarn build` 全绿。
- 变更自动暂存并按 conventional commits 提交（项目授权自动 commit）。
- task worktree 合并回 master + 移除（环境干净）。
- 非平凡发现落 cortex（中间件架构/三级作用域解析/error_rule↔重试接线）。
- bump 版本（用户可见功能变更）。

## Out of Scope
- 不引入 Redis / 跨进程缓存失效（单进程内存缓存足够）。
- 不做规则的导入/导出、版本历史、A/B。
- 不做请求级限流（rate limit，参考仓库有但非本次需求）。
- 不改造现有协议转换 converter 的语义，仅在其前后挂载。
- 不做规则命中的实时统计面板（仅审计日志，统计面板后续）。

## Decision (ADR-lite)

**Context**: 8 类中间件能力，需定数据模型/作用域叠加/流式处理/错误规则与现有重试的关系。

**Decision**（全部经 AskUserQuestion 用户拍板）:
1. 范围：8 类一次性做全。
2. 作用域：全局 + group + platform 三级，**就近覆盖**（同类型最细粒度层生效）。
3. 规则来源：内置预设 + 用户自定义，内置默认开启。
4. 命中动作：脱敏/改写 + 拦截/拒绝 + 仅告警，三者皆支持，per-rule 可覆盖。
5. 流式响应：**逐块改写**保留实时性。
6. UI：AppSettings 新 tab（全局）+ group/platform 页内嵌。
7. 错误规则：分类 + 覆写响应 + 产出 retryable/non-retryable 标记喂现有重试。**熔断器经后续用户决策移出本树**，归 group 功能块（新 parent 树 `06-13-group-scheduling-breaker`，组内配置 + 默认值）。
8. 健壮性：fail-open、拦截写审计日志、规则热更新免重启、正则 ReDoS 防护。

**Consequences**:
- 单表 `middleware_rule` 统一抽象（偏离参考仓库三表，更适配 Rust+SQLite）。
- 熔断器与智能调度独立成 group 树：本树 error_rule 仅产信号（retryable/non-retryable），group 树消费信号 + auto_disabled 决定候选过滤，二者解耦避免半套实现。
- 8 类全做 + 三级作用域 + 流式 + 7 语言 UI = 大改，后端 child 串行（共享 proxy.rs/db.rs/middleware.rs），前端可并行。

## Technical Notes

### 文件位置
- 新增 `src-tauri/src/gateway/middleware.rs`（引擎 + 检测器 + 缓存 + 作用域解析；熔断器不在此）。
- 改：`models.rs`（MiddlewareRule/MiddlewareSettings/枚举）、`db.rs`（表 DDL + CRUD + seed）、`proxy.rs`（入站/出站挂载）、`lib.rs`（commands 注册）。
- 前端：`services/api.ts`（类型 + invoke）、`AppSettings.tsx`（tab）、`components/settings/**`、group/platform 编辑页、i18n 文件。

### 灰度 / 回滚
- 总开关 OFF = 全链路旁路（等价未启用），即时回滚。
- 内置规则可逐条禁用。
- worktree 隔离，未合并前 master 不受影响。

### 验证命令
```bash
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
cd .. && yarn build
```
