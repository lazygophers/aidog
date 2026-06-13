# 中间件 C1 — 规则引擎基座

Parent: `06-13-request-response-middleware` — 8 类请求/响应中间件规则引擎。共享架构契约见 `../06-13-request-response-middleware/design.md`。

## Goal

落地中间件规则引擎的公共底座：`middleware_rule` 表 + Rust 数据模型/枚举 + CRUD + 内存缓存(分桶/regex 预编译) + 三级作用域就近覆盖解析 + ReDoS 防护 + Tauri commands；并**冻结 `services/api.ts` 契约**供 C5 消费。完成后：`cargo test` 引擎单测过，规则增改后缓存自动 reload，前端可通过 commands 增删改查规则与 settings。

## What I already know
- 共享架构/数据模型/作用域解析/契约定义全在 parent `design.md`（权威）。
- settings KV 模式见 models.rs `ProxyLogSettings` + db.rs settings CRUD + lib.rs command + api.ts。
- 本 child **先行**，C2/C3/C4/C5 全依赖其产物。

## Deliverable 矩阵
| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| D1.1 | models.rs 模型/枚举 + MiddlewareSettings + BreakerConfig | diff | 编译过 + serde 往返单测 | P0 |
| D1.2 | db.rs 表 DDL + CRUD + MiddlewareSettings get/set | diff | CRUD 单测 | P0 |
| D1.3 | middleware.rs 引擎(缓存/作用域解析/regex 预编译/ReDoS) | diff | 作用域就近覆盖 + 缓存 reload 单测 | P0 |
| D1.4 | lib.rs commands 注册 + api.ts 契约冻结 | diff/契约 | commands 在 invoke_handler；api.ts 类型+封装存在 | P0 |

## Requirements
- R1 单表 schema 严格按 design.md（字段名/类型/索引）。
- R2 枚举 RuleType/RuleScope/MatchType/RuleAction，serde snake_case，与 TS 字面量对齐。
- R3 MiddlewareEngine 单例(随 ProxyState 或 Db 持有)，按 (rule_type,scope) 分桶缓存 + 预编译 regex；CRUD 写后 reload。
- R4 `resolve_rules(rule_type, group_name, platform_id)` 就近覆盖。
- R5 ReDoS：正则编译失败/超限跳过 + 记日志，不 panic。
- R6 commands：listRules/createRule/updateRule/deleteRule/getSettings/setSettings。
- R7 api.ts：MiddlewareRule/MiddlewareSettings TS 类型 + middlewareApi 封装（**契约冻结点**）。

## Acceptance Criteria
- [ ] `cd src-tauri && cargo test` 引擎/作用域/缓存/serde 单测通过。
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告。
- [ ] commands 在 lib.rs invoke_handler 注册。
- [ ] api.ts 含 MiddlewareRule/MiddlewareSettings 类型 + middlewareApi.*；`yarn build` 通过（类型层）。

## Definition of Done
- Requirements 全实现 + AC 勾选；变更自动暂存提交；非平凡发现落 cortex；契约冻结通知 parent。

## Out of Scope
- 不做规则的实际执行（入站/出站属 C2/C3）；不做内置 seed（C4）；不做 UI（C5）。

## Technical Notes
- 新文件 `src-tauri/src/gateway/middleware.rs`；改 models.rs/db.rs/lib.rs/api.ts。
- 验证：`cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cd .. && yarn build`。
