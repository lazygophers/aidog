# SKEIN recall 规则索引 (章节粒度: 一行一条规则)

类目: arch(97), build(58), db(5), domain(74), frontend(80), git(7), i18n(15), ops(23), optimization(43), proxy(26), reuse(5), shadcn(48), skein(24), style(9), test(14), testing(15), ts-rust-boundary(12) · 关联见 [backlinks.md](backlinks.md)

| rule (topic.md#标题) | category | title | keywords | inclusion | anchors | status/出链 | summary |
|---|---|---|---|---|---|---|---|
| arch/adapter-deadcode-whitelist-authority.md#关联 | arch | 关联 | adapter, 死代码, 白名单, wire_protocol, converter | auto | - | active / →wire-protocol-gate-is-failfast | [[wire-protocol-gate-is-failfast]]  --- |
| arch/adapter-deadcode-whitelist-authority.md#案例 | arch | 案例 | adapter, 死代码, 白名单, wire_protocol, converter | auto | - | active | - arch-deepen-2 commit `78e32df4`：删的 5 个 vendor adapter（glm_… |
| arch/adapter-deadcode-whitelist-authority.md#正解 | arch | 正解 | adapter, 死代码, 白名单, wire_protocol, converter | auto | - | active | **唯一权威 = `gateway/proxy/forward.rs:85-86` 的 `is_valid_wire_p… |
| arch/adapter-deadcode-whitelist-authority.md#触发场景 | arch | 触发场景 | adapter, 死代码, 白名单, wire_protocol, converter | auto | - | active | 删除 vendor adapter 文件或判定某 adapter 是否属于死代码时。 |
| arch/adapter-deadcode-whitelist-authority.md#适用 | arch | 适用 | adapter, 死代码, 白名单, wire_protocol, converter | auto | - | active | - adapter 文件管理时 - protocol 数量变更 - 编码规范卡关：为什么要删这个文件 |
| arch/adapter-deadcode-whitelist-authority.md#陷阱 | arch | 陷阱 | adapter, 死代码, 白名单, wire_protocol, converter | auto | - | active | 用文件名判定（如「vendor 名 = 协议名」），误删活代码；或遗漏实际有白名单的 adapter。 |
| arch/agent-platform-handler-branch.md#关联 | arch | 关联 | agent,handler,branch,platform,wire,sse | auto | - | active / →protocol-variant-extension | dashmap-sharding (session 映射) [[protocol-variant-extension]]… |
| arch/agent-platform-handler-branch.md#判定：分支 vs wire | arch | 判定：分支 vs wire | agent,handler,branch,platform,wire,sse | auto | - | active | / 特征 / wire 层 / handler 分支 / /------/---------/-------------… |
| arch/agent-platform-handler-branch.md#反例 | arch | 反例 | agent,handler,branch,platform,wire,sse | auto | - | active | ❌ 新 agent 平台塞 wire 层 → adapter 改到吐血 ❌ 分支内做多候选 retry → agent … |
| arch/agent-platform-handler-branch.md#触发场景 | arch | 触发场景 | agent,handler,branch,platform,wire,sse | auto | - | active | 新增「agent-as-LLM」类平台（无标准 chat completions wire，API 形态是 sessio… |
| arch/agent-platform-handler-branch.md#适用 | arch | 适用 | agent,handler,branch,platform,wire,sse | auto | - | active | agent-as-LLM 平台接入（Mock/ClaudeCode/Devin/Factory） |
| arch/agent-platform-handler-branch.md#陷阱-正解 | arch | 陷阱-正解 | agent,handler,branch,platform,wire,sse | auto | - | active | - **陷阱**: 新平台硬塞 wire 层 → adapter/converter 反复打补丁、协议转换丢字段、候选切… |
| arch/coding-plan-base-url-from-endpoint.md#coding-plan-utilization-calib-fix-25 | arch | coding-plan-utilization-calib-fix-25 | coding-plan,base_url,quota,calibration,finish,est_coding_plan | auto | - | active | --- coding plan 平台 preset 平台级 base_url 恒为 None (真 base_url 在… |
| arch/component-extraction-grep-callsites.md#关联 | arch | 关联 | refactor,component,extraction,grep,dead-code | auto | - | active / →grep-before-write | [[grep-before-write]] |
| arch/component-extraction-grep-callsites.md#案例 | arch | 案例 | refactor,component,extraction,grep,dead-code | auto | - | active | - arch-deepen-2 commit `1eee3975`：删 ImportDialog 内联 91 行副本前先… |
| arch/component-extraction-grep-callsites.md#检查清单 | arch | 检查清单 | refactor,component,extraction,grep,dead-code | auto | - | active | ```bash # 抽前 & 抽后各一次 grep -r "ProviderRow" --include="*.tsx"… |
| arch/component-extraction-grep-callsites.md#正解 | arch | 正解 | refactor,component,extraction,grep,dead-code | auto | - | active | 1. grep 搜索原位置组件名，确认所有调用点 2. 逐个改为新 import 路径 3. 最后删旧副本前再 grep… |
| arch/component-extraction-grep-callsites.md#触发场景 | arch | 触发场景 | refactor,component,extraction,grep,dead-code | auto | - | active | 从大文件抽出独立组件或把函数迁移到新位置时。 |
| arch/component-extraction-grep-callsites.md#适用 | arch | 适用 | refactor,component,extraction,grep,dead-code | auto | - | active | - UI 组件抽取重构 - 函数迁 crate 时 - 任何多处定义的重复 |
| arch/component-extraction-grep-callsites.md#陷阱 | arch | 陷阱 | refactor,component,extraction,grep,dead-code | auto | - | active | 只 import 不渲染 = 死代码副本。原文件可能仍有内联副本，抽取后遗漏切换会导致两份代码。 |
| arch/cross-db-subquery-handle-selection.md#Cross-ref | arch | Cross-ref | subquery, cross-db, SelectionStrategy, 查询, db | auto | - | active / →db-split-access-point-audit | - sqlite-cross-db-no-join（跨库禁 JOIN，强制拆闭包 + Rust 合并） - [[db-s… |
| arch/cross-db-subquery-handle-selection.md#MUST 规则 | arch | MUST 规则 | subquery, cross-db, SelectionStrategy, 查询, db | auto | - | active | 跨库补查闭包的 handle **必须按补查表的库归属选**，禁顺手复用主表 handle。 |
| arch/cross-db-subquery-handle-selection.md#正确写法（✅） | arch | 正确写法（✅） | subquery, cross-db, SelectionStrategy, 查询, db | auto | - | active | ```rust // 主查走 log.db handle let logs = proxy_log_handle.cal… |
| arch/cross-db-subquery-handle-selection.md#错误样本（❌） | arch | 错误样本（❌） | subquery, cross-db, SelectionStrategy, 查询, db | auto | - | active | ```rust // proxy_log 在 log.db，补查 cpp.name 在 platform.db prox… |
| arch/cross-db-subquery-handle-selection.md#验收 | arch | 验收 | subquery, cross-db, SelectionStrategy, 查询, db | auto | - | active | ```bash # 找跨库补查点（同函数 / 同闭包内出现多库表名） grep -rn 'FROM "proxy_log… |
| arch/db-split-access-point-audit.md#关联 | arch | 关联 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active / →cross-db-subquery-handle-selection,db-split-access-point-audit | [[cross-db-subquery-handle-selection]] (跨库读两阶段) [[db-split-a… |
| arch/db-split-access-point-audit.md#反例 | arch | 反例 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | ❌ 只 grep `call_traced` → 6 处 `write_conn` 漏网（s3 错误模式） ❌ 只 gr… |
| arch/db-split-access-point-audit.md#触发场景 | arch | 触发场景 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | 表从一个 SQLite 库拆到另一个库（主库→log.db / platform.db），需把该表所有访问点切到新 ha… |
| arch/db-split-access-point-audit.md#适用 | arch | 适用 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | DB 拆库迁移、表访问点归属审计 |
| arch/db-split-access-point-audit.md#陷阱-正解 | arch | 陷阱-正解 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | - **陷阱**: 只查 `call_*_traced` chokepoint → 漏掉 `.write_conn()`… |
| arch/db-split-access-point-audit.md#验收命令 | arch | 验收命令 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | ```bash # 1. wrapper 形式 grep -rn "call_platform_traced\/call… |
| arch/dedup-key-must-be-nonempty.md#关联 | arch | 关联 | dedup,空字段,key,数据丢失,合并 | auto | - | active / →locale-deadkey-cleanup-ownership | [[locale-deadkey-cleanup-ownership]] (数据清理) |
| arch/dedup-key-must-be-nonempty.md#反例 | arch | 反例 | dedup,空字段,key,数据丢失,合并 | auto | - | active | ❌ (provider.source_segment, provider.base_url) 其中 base_url 全… |
| arch/dedup-key-must-be-nonempty.md#正解 | arch | 正解 | dedup,空字段,key,数据丢失,合并 | auto | - | active | dedup key 选择优先级： 1. **业务唯一键**(user_id / email / name) — 最稳 2… |
| arch/dedup-key-must-be-nonempty.md#测试 | arch | 测试 | dedup,空字段,key,数据丢失,合并 | auto | - | active | 构造 N 个对象(该字段全空但其余不同)，dedup 后必须保留 N 个(非合并为 1)。 |
| arch/dedup-key-must-be-nonempty.md#触发场景 | arch | 触发场景 | dedup,空字段,key,数据丢失,合并 | auto | - | active | 写任何 dedup / 去重 / 合并逻辑(HashSet key / HashMap key / groupBy ke… |
| arch/dedup-key-must-be-nonempty.md#适用 | arch | 适用 | dedup,空字段,key,数据丢失,合并 | auto | - | active | dedup / 去重 / 合并逻辑、数据导入解析 |
| arch/dedup-key-must-be-nonempty.md#陷阱 | arch | 陷阱 | dedup,空字段,key,数据丢失,合并 | auto | - | active | 字段设计为空(待后续回填 / 占位)但被用作 dedup key → N 个对象共享同一空值 → HashSet 全撞 … |
| arch/enum-variant-delete-needs-migration.md#MUST 流程 | arch | MUST 流程 | enum,serde,db,migration,rust,panic | auto | - | active | 1. 写 migration: DELETE FROM table WHERE enum_column = 'delet… |
| arch/enum-variant-delete-needs-migration.md#关联 | arch | 关联 | enum,serde,db,migration,rust,panic | auto | - | active / →locale-deadkey-cleanup-ownership,protocol-variant-extension | [[locale-deadkey-cleanup-ownership]] (locale 清理) [[protocol-… |
| arch/enum-variant-delete-needs-migration.md#反例 | arch | 反例 | enum,serde,db,migration,rust,panic | auto | - | active | ❌ 先删代码再 migration → migration 期间所有访问 panic ❌ 只改 TS 未改 Rust e… |
| arch/enum-variant-delete-needs-migration.md#硬约束 | arch | 硬约束 | enum,serde,db,migration,rust,panic | auto | - | active | **删 serde 落库的 enum 变体前必须先 migration DELETE DB 旧值**，否则代码中 `fr… |
| arch/enum-variant-delete-needs-migration.md#触发场景 | arch | 触发场景 | enum,serde,db,migration,rust,panic | auto | - | active | 删 serde 落库的 enum 变体时。 |
| arch/enum-variant-delete-needs-migration.md#适用 | arch | 适用 | enum,serde,db,migration,rust,panic | auto | - | active | serde enum 变体删除、DB schema enum 迁移、前后端 enum 同步 |
| arch/frontend-constants-derived-from-json.md#AppContext 预热缓存 (best-effort) | arch | AppContext 预热缓存 (best-effort) | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | AppContext 顶层调一次 `buildXFromPresets().catch(console.error)` … |
| arch/frontend-constants-derived-from-json.md#Cross-reference | arch | Cross-reference | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | - 真值源: `src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆… |
| arch/frontend-constants-derived-from-json.md#单真值源派生 (MUST) | arch | 单真值源派生 (MUST) | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | 前端平台 / 协议类大枚举常量（`PROTOCOLS` / `PROTOCOL_LABELS` / `PROTOCOL_… |
| arch/frontend-constants-derived-from-json.md#实例 | arch | 实例 | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | task 07-10-protocols-frontend-derive（C3）： - 删 `PROTOCOLS`（81… |
| arch/frontend-constants-derived-from-json.md#小常量例外（保留硬编码） | arch | 小常量例外（保留硬编码） | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | 非后端真值源映射的小常量（请求格式协议 5 条 `ENDPOINT_PROTOCOLS` / 路由判定 / UI 固定枚… |
| arch/frontend-constants-derived-from-json.md#调用点 async 化范式 (MUST) | arch | 调用点 async 化范式 (MUST) | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | 派生函数 async 后，所有 caller **MUST** 改 `useEffect + useState` 模式，… |
| arch/frontend-constants-derived-from-json.md#验收断言（可复用） | arch | 验收断言（可复用） | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | ```bash # 派生层单 RPC 缓存（docPromise module-level 单次 invoke，非函数内… |
| arch/gemini-sse-alt-param.md#关联 | arch | 关联 | gemini,sse,streaming,adapter,parameter | auto | - | active / →adapter-deadcode-whitelist-authority,protocol-wire-str | [[protocol-wire-str]] [[adapter-deadcode-whitelist-authority… |
| arch/gemini-sse-alt-param.md#案例 | arch | 案例 | gemini,sse,streaming,adapter,parameter | auto | - | active | - arch-deepen-2 commit `39a6614c`：gateway/proxy/forward.rs:2… |
| arch/gemini-sse-alt-param.md#正解 | arch | 正解 | gemini,sse,streaming,adapter,parameter | auto | - | active | 向 Gemini 端点拼入 `?alt=sse` 参数，确保响应格式为 Server-Sent Events。 |
| arch/gemini-sse-alt-param.md#触发场景 | arch | 触发场景 | gemini,sse,streaming,adapter,parameter | auto | - | active | 改 gemini adapter 或调试 Gemini streaming 响应时。 |
| arch/gemini-sse-alt-param.md#适用 | arch | 适用 | gemini,sse,streaming,adapter,parameter | auto | - | active | - Gemini 协议 SSE 响应处理 - 其他 SSE 适配器的对称性检查（防止他协议有类似参数需求遗漏） |
| arch/gemini-sse-alt-param.md#陷阱 | arch | 陷阱 | gemini,sse,streaming,adapter,parameter | auto | - | active | 不带 `?alt=sse` 参数时，Gemini API 响应体不是 SSE 格式（返回普通 JSON 数组），`str… |
| arch/i18n-key-set-diff-check.md#关联 | arch | 关联 | i18n,migration,locale,key,coverage,comm | auto | - | active | - |
| arch/i18n-key-set-diff-check.md#案例 | arch | 案例 | i18n,migration,locale,key,coverage,comm | auto | - | active | - arch-deepen-2 c3-commands batch 3：搬迁时检查 system/ai_tools/cl… |
| arch/i18n-key-set-diff-check.md#正解 | arch | 正解 | i18n,migration,locale,key,coverage,comm | auto | - | active | 搬迁前后比对 locale key 集合（grep 源代码找 namespace 模式），用 comm -23 差集查漏… |
| arch/i18n-key-set-diff-check.md#触发场景 | arch | 触发场景 | i18n,migration,locale,key,coverage,comm | auto | - | active | command/组件迁 crate 或改名时，若涉及 i18n key（如 UI 文案）。 |
| arch/i18n-key-set-diff-check.md#适用 | arch | 适用 | i18n,migration,locale,key,coverage,comm | auto | - | active | - 跨 crate 搬迁涉及 i18n - rename command 时 - 删减功能前验证 |
| arch/i18n-key-set-diff-check.md#陷阱 | arch | 陷阱 | i18n,migration,locale,key,coverage,comm | auto | - | active | 不动 locale 文件时 `yarn check-i18n` 查不出搬迁丢 key（新位置 key 可能取名不同）。 |
| arch/invoke-name-source-of-truth.md#关联 | arch | 关联 | command,tauri,handler,migration,invoke,symmetry | auto | - | active | - |
| arch/invoke-name-source-of-truth.md#案例 | arch | 案例 | command,tauri,handler,migration,invoke,symmetry | auto | - | active | - arch-deepen-2 batch 3：commands 迁 aidog_core 时，verify 用 com… |
| arch/invoke-name-source-of-truth.md#正解 | arch | 正解 | command,tauri,handler,migration,invoke,symmetry | auto | - | active | **invoke 名的真值源 = `src-tauri/src/startup.rs:41` 的 `tauri::gen… |
| arch/invoke-name-source-of-truth.md#触发场景 | arch | 触发场景 | command,tauri,handler,migration,invoke,symmetry | auto | - | active | command 跨 crate 搬迁后（新增、删除、拆分 command）。 |
| arch/invoke-name-source-of-truth.md#适用 | arch | 适用 | command,tauri,handler,migration,invoke,symmetry | auto | - | active | - command 跨 crate 搬迁 - 新增/删除 command - 重构后 sanity check |
| arch/invoke-name-source-of-truth.md#陷阱 | arch | 陷阱 | command,tauri,handler,migration,invoke,symmetry | auto | - | active | 改了 Rust 函数签名或迁移位置，却漏改了前端 invoke 名或 startup.rs 注册，导致静默失败。 |
| arch/locale-deadkey-cleanup-ownership.md#关联 | arch | 关联 | locale,dead-key,cleanup,responsibility,theme | auto | - | active / →enum-variant-delete-needs-migration | [[enum-variant-delete-needs-migration]] (同任务 enum 删约定) |
| arch/locale-deadkey-cleanup-ownership.md#反例 | arch | 反例 | locale,dead-key,cleanup,responsibility,theme | auto | - | active | ❌ 删 palette 只改代码不清理 locale → 死键残留 ❌ 甩给「下次整理 locale 时」→ 永远不清理… |
| arch/locale-deadkey-cleanup-ownership.md#案例 | arch | 案例 | locale,dead-key,cleanup,responsibility,theme | auto | - | active | - shadcn-infra task: 删 palette 时应同步清理 theme.color.* locale 键 |
| arch/locale-deadkey-cleanup-ownership.md#正解 | arch | 正解 | locale,dead-key,cleanup,responsibility,theme | auto | - | active | 1. **删 palette 主题**: 清理所有 `theme.color.{palette}` 相关 locale … |
| arch/locale-deadkey-cleanup-ownership.md#流程约定 | arch | 流程约定 | locale,dead-key,cleanup,responsibility,theme | auto | - | active | **删除主题/功能导致的 locale 死键，由删该主题/功能的 task 同源清理**，不甩给下游消费 task。 |
| arch/locale-deadkey-cleanup-ownership.md#适用 | arch | 适用 | locale,dead-key,cleanup,responsibility,theme | auto | - | active | locale 清理、主题删除、功能下架、enum 变体删除 |
| arch/locale-deadkey-cleanup-ownership.md#陷阱 | arch | 陷阱 | locale,dead-key,cleanup,responsibility,theme | auto | - | active | - **陷阱**: 删代码只删 TS 类型，locale 死键留给后续清理 → 下次改 locale 人困惑 - **陷… |
| arch/protocol-variant-extension.md#Cross-reference | arch | Cross-reference | protocol,enum,变体,grep,serde,match,union | auto | - | active | - research 结论：`.trellis/tasks/archive/2026-07/07-10-protocol… |
| arch/protocol-variant-extension.md#serde round-trip + JSON key 对齐 (MUST) | arch | serde round-trip + JSON key 对齐 (MUST) | protocol,enum,变体,grep,serde,match,union | auto | - | active | - `#[serde(rename = "<key>")]` 与 `platform-presets.json` pro… |
| arch/protocol-variant-extension.md#命中点 3 类分类（据实判定改动面） | arch | 命中点 3 类分类（据实判定改动面） | protocol,enum,变体,grep,serde,match,union | auto | - | active | grep 同构变体命中点，按下列 3 类分类，**仅第 1 类必须改**：  1. **enum 定义 + serde … |
| arch/protocol-variant-extension.md#实例 | arch | 实例 | protocol,enum,变体,grep,serde,match,union | auto | - | active | task 07-10-protocols-rust-enum：+3 cp 变体（KimiCoding/QianfanCo… |
| arch/protocol-variant-extension.md#新增变体 MUST 先 grep 同构变体命中点 (MUST) | arch | 新增变体 MUST 先 grep 同构变体命中点 (MUST) | protocol,enum,变体,grep,serde,match,union | auto | - | active | 新增 `Protocol` 变体前，**MUST** grep 现有同构变体全链命中点，据实际命中分类决定改动面，禁预设… |
| arch/protocol-variant-extension.md#零专属 match 臂 → 加枚举即覆盖 (MUST) | arch | 零专属 match 臂 → 加枚举即覆盖 (MUST) | protocol,enum,变体,grep,serde,match,union | auto | - | active | **反直觉发现**：`router.rs` / `adapter/converter.rs` / `quota.rs` … |
| arch/protocol-variant-extension.md#验收断言（可复用） | arch | 验收断言（可复用） | protocol,enum,变体,grep,serde,match,union | auto | - | active | ```bash # 新变体字面量全链命中点清单（据分类决定改动面） grep -rn '<NewVariant>\/<n… |
| arch/tauri-command-macro-no-mut.md#关联 | arch | 关联 | tauri,command,macro,parameter,mut | auto | - | active | - |
| arch/tauri-command-macro-no-mut.md#案例 | arch | 案例 | tauri,command,macro,parameter,mut | auto | - | active | - arch-deepen-2：迁 command 时遇此限制 |
| arch/tauri-command-macro-no-mut.md#正解 | arch | 正解 | tauri,command,macro,parameter,mut | auto | - | active | 去掉函数签名中的 `mut`，在函数体首行用 `let mut x = x;` 重绑定： ```rust // 错误 #… |
| arch/tauri-command-macro-no-mut.md#触发场景 | arch | 触发场景 | tauri,command,macro,parameter,mut | auto | - | active | Tauri command 函数形参中使用 `mut` 修饰时。 |
| arch/tauri-command-macro-no-mut.md#适用 | arch | 适用 | tauri,command,macro,parameter,mut | auto | - | active | - Tauri command 签名设计 - 其他 proc macro 类似限制排查 |
| arch/tauri-command-macro-no-mut.md#陷阱 | arch | 陷阱 | tauri,command,macro,parameter,mut | auto | - | active | `tauri_command!` 宏模式 `$($arg:ident : $ty:ty),*` 不匹配 `mut x: … |
| arch/tauri-popover-window-reuse.md#关联 | arch | 关联 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active / →frontend-conventions,planning-scope-pregrep | [[planning-scope-pregrep]] (popover 域划分) / [[frontend-conven… |
| arch/tauri-popover-window-reuse.md#反例 | arch | 反例 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | ```rust // ❌ 陷阱实现（每次销毁） if let Some(w) = app.get_webview_win… |
| arch/tauri-popover-window-reuse.md#实现清单 | arch | 实现清单 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | - [ ] `app_setup.rs::setup` 阶段 `prebuild_popover()`：`.visibl… |
| arch/tauri-popover-window-reuse.md#性能收益 | arch | 性能收益 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | - 消除冷启 webview (setup 预建一次)。 - 去掉 tray click 时的 4 路 IPC 瀑布（背… |
| arch/tauri-popover-window-reuse.md#案例 | arch | 案例 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | - popover-perf task (commit 14ec141d)：预建隐藏窗 + toggle hide/sh… |
| arch/tauri-popover-window-reuse.md#触发场景 | arch | 触发场景 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | 实现 Tauri 桌面应用的浮窗（如托盘 popover）时，需要避免每次点击都冷启 webview，导致的延迟与卡顿。 |
| arch/tauri-popover-window-reuse.md#适用 | arch | 适用 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | - Tauri 桌面应用浮窗（托盘 popover、context menu、floating panel） - 需要快… |
| arch/tauri-popover-window-reuse.md#陷阱-正解 | arch | 陷阱-正解 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | ❌ **陷阱**：tray 点击每次 destroy + 新建窗口 → 冷启 webview + 瀑布 IPC 4 路 … |
| build/build-rs-env-is-crate-scoped.md#关联 | build | 关联 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | - |
| build/build-rs-env-is-crate-scoped.md#案例 | build | 案例 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | - arch-deepen-2 c3-commands batch 3：commands_tray/commands_s… |
| build/build-rs-env-is-crate-scoped.md#检查 | build | 检查 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | ```bash # 检查迁移后是否仍能编译通过 cargo build -p aidog_core  # 应无 env!… |
| build/build-rs-env-is-crate-scoped.md#正解 | build | 正解 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | 迁移代码到新 crate 后，给**新 crate 补等价的 build.rs**，重新定义环境变量。 |
| build/build-rs-env-is-crate-scoped.md#触发场景 | build | 触发场景 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | 用 `env!("XXX")` 的代码从一个 crate 迁移到另一个 crate 时。 |
| build/build-rs-env-is-crate-scoped.md#适用 | build | 适用 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | - 任何用 env!() 的代码跨 crate 迁移 - workspace 多 crate 场景 - build.rs… |
| build/build-rs-env-is-crate-scoped.md#陷阱 | build | 陷阱 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | `cargo:rustc-env=` 在 build.rs 中定义的环境变量**只对定义它的 crate 生效**，跨 … |
| build/cargo-workspace-hygiene.md#Cross-reference | build | Cross-reference | cargo, workspace, workspaces, lint, members | auto | - | active | - parent design：`.trellis/tasks/07-10-commands-restructure/d… |
| build/cargo-workspace-hygiene.md#GUI 冒烟降级（worktree 无 display 时） | build | GUI 冒烟降级（worktree 无 display 时） | cargo, workspace, workspaces, lint, members | auto | - | active | worktree 无 `node_modules` / 无 display 无法跑 `yarn tauri dev` 全… |
| build/cargo-workspace-hygiene.md#PoC 空骨架门禁 (MUST) | build | PoC 空骨架门禁 (MUST) | cargo, workspace, workspaces, lint, members | auto | - | active | 单 crate → workspace 多 crate 重构 **MUST 先建空骨架 PoC 门禁**，过才放行全量迁… |
| build/cargo-workspace-hygiene.md#PoC 门禁验收 (MUST，全量迁移前必过) | build | PoC 门禁验收 (MUST，全量迁移前必过) | cargo, workspace, workspaces, lint, members | auto | - | active | 1. `cargo build --workspace`：0 errors（含现 root crate + N 空壳 +… |
| build/cargo-workspace-hygiene.md#recall/cross-layer | build | recall/cross-layer | cargo, workspace, workspaces, lint, members | auto | - | active | - |
| build/cargo-workspace-hygiene.md#root 过渡路径迁移 (MUST) | build | root 过渡路径迁移 (MUST) | cargo, workspace, workspaces, lint, members | auto | - | active | core 提取后 root package **过渡保留**（binary crate C10 才建），加 `aidog… |
| build/cargo-workspace-hygiene.md#workspace.dependencies 版本对齐 (MUST) | build | workspace.dependencies 版本对齐 (MUST) | cargo, workspace, workspaces, lint, members | auto | - | active | - `[workspace.dependencies]` 版本号 + features **MUST 逐项照抄**现 r… |
| build/cargo-workspace-hygiene.md#子 crate 规范 (MUST) | build | 子 crate 规范 (MUST) | cargo, workspace, workspaces, lint, members | auto | - | active | - `name` 用下划线（`commands_platform` 等，非 hyphen；目录名连字符是 Cargo 惯… |
| build/cargo-workspace-hygiene.md#实例 | build | 实例 | cargo, workspace, workspaces, lint, members | auto | - | active | task 07-10-ws-skeleton（commands-restructure C1）：src-tauri 单 … |
| build/cargo-workspace-hygiene.md#核心提取下沉防循环范式 (MUST) | build | 核心提取下沉防循环范式 (MUST) | cargo, workspace, workspaces, lint, members | auto | - | active | PoC 空骨架过门后，业务代码入 `aidog_core` 时**MUST** 据依赖关系分类下沉，防 core→com… |
| build/cargo-workspace-hygiene.md#验收断言（可复用） | build | 验收断言（可复用） | cargo, workspace, workspaces, lint, members | auto | - | active | ```bash # baseline 不回归 cargo test --workspace --lib / grep -… |
| build/cargo-workspace-hygiene.md#验收断言（核心提取，可复用） | build | 验收断言（核心提取，可复用） | cargo, workspace, workspaces, lint, members | auto | - | active | ```bash # 路径迁移彻底（root 残留核心域路径 = 漏改） grep -rn 'crate::gateway… |
| build/clippy-touch-before-recheck.md#关联 | build | 关联 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | - |
| build/clippy-touch-before-recheck.md#案例 | build | 案例 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | - arch-deepen-2：迁移函数后 clippy 无新输出，touch 才触发重编检查 |
| build/clippy-touch-before-recheck.md#正解 | build | 正解 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | 修改源文件后跑 clippy 前，先 `touch` 该文件强制重编： ```bash touch src-tauri/… |
| build/clippy-touch-before-recheck.md#触发场景 | build | 触发场景 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | 修改后再跑 `cargo clippy` 判断 warning 数时。 |
| build/clippy-touch-before-recheck.md#适用 | build | 适用 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | - 验证 clippy 改动效果 - 高频编译场景 - 持续集成前检查 |
| build/clippy-touch-before-recheck.md#陷阱 | build | 陷阱 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | 同命令第二次跑输出为空（命中编译缓存），易误判「0 warning」实际仍有。 |
| build/converter-endpoint-decoupled.md#MUST 硬约束 | build | MUST 硬约束 | - | auto | - | active | converter 双向转（source→wire 请求 + wire→source 响应）与 endpoint 选择解… |
| build/converter-endpoint-decoupled.md#关联 | build | 关联 | - | auto | - | active | - |
| build/converter-endpoint-decoupled.md#反例 | build | 反例 | - | auto | - | active | - ❌ 误判：endpoint 层限制只许选同协议 → converter 能力已就绪，endpoint 无需自我限制 … |
| build/converter-endpoint-decoupled.md#案例 | build | 案例 | - | auto | - | active / →wire-protocol-gate-is-failfast | - endpoint-cross-protocol-fallback task：converter 5×5 已就绪，en… |
| build/converter-endpoint-decoupled.md#适用 | build | 适用 | - | auto | - | active | - 所有新增 wire protocol 的变更 - endpoint 跨协议回退扩展 - converter 双向转换… |
| build/shadcn-add-verify-deps.md#关联 | build | 关联 | shadcn-ui, add, verify, dependencies, package.json | auto | - | active / →theme-token-runtime-switch | [[theme-token-runtime-switch]] (同任务产出的前端规则)  --- |
| build/shadcn-add-verify-deps.md#反例 | build | 反例 | shadcn-ui, add, verify, dependencies, package.json | auto | - | active | ❌ 只加 UI 组件不验证 cva → 运行时崩 ❌ 改 package.json 后不 yarn install → … |
| build/shadcn-add-verify-deps.md#案例 | build | 案例 | shadcn-ui, add, verify, dependencies, package.json | auto | - | active | - shadcn-infra task: 首次 `shadcn add` 后运行时崩，发现 cva 缺失 - 根因: y… |
| build/shadcn-add-verify-deps.md#触发场景 | build | 触发场景 | shadcn-ui, add, verify, dependencies, package.json | auto | - | active | 运行 `npx shadcn add` 批量添加组件后，依赖树中仅含 `@radix-ui/react-slot` 等 … |
| build/shadcn-add-verify-deps.md#适用 | build | 适用 | shadcn-ui, add, verify, dependencies, package.json | auto | - | active | yarn 4+ / pnp 环境，shadcn 批量 add 场景 |
| build/shadcn-add-verify-deps.md#陷阱-正解 | build | 陷阱-正解 | shadcn-ui, add, verify, dependencies, package.json | auto | - | active | - **陷阱**: shadcn CLI 在 yarn 4+ / pnp 环境下可能未正确解析 cva 传递依赖，只装直… |
| build/tailwind-v4-import-form.md#MUST 迁移方式 | build | MUST 迁移方式 | tailwind,v4,preflight,migration,css | auto | - | active | 1. 仅 import theme/utilities（跳过 preflight/base） 2. 或单行总导入：@im… |
| build/tailwind-v4-import-form.md#关联 | build | 关联 | tailwind,v4,preflight,migration,css | auto | - | active / →css-var-alias-layer,shadcn-add-verify-deps | [[css-var-alias-layer]] [[shadcn-add-verify-deps]] |
| build/tailwind-v4-import-form.md#硬约束 | build | 硬约束 | tailwind,v4,preflight,migration,css | auto | - | active | Tailwind v4 迁移过程中**禁使用旧 v3 的三行导入方式**，必须用 v4 的 @import 方式。 |
| build/tailwind-v4-import-form.md#禁用的旧方式 | build | 禁用的旧方式 | tailwind,v4,preflight,migration,css | auto | - | active | ❌ @tailwind base;  /* v3 方式，v4 崩盘 */ ❌ @tailwind components;… |
| build/tailwind-v4-import-form.md#适用 | build | 适用 | tailwind,v4,preflight,migration,css | auto | - | active | Tailwind v3 → v4 迁移、新项目用 v4 |
| build/tauri-build-bundle.md#yarn tauri build --no-bundle 不产 .app | build | yarn tauri build --no-bundle 不产 .app | tauri, bundle, dmg, 构建产物, resources, app | auto | - | active | - |
| build/tauri-build-bundle.md#反例（错误模式） | build | 反例（错误模式） | tauri, bundle, dmg, 构建产物, resources, app | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / `yarn tauri build --no-bundle` / … |
| build/tauri-build-bundle.md#案例 | build | 案例 | tauri, bundle, dmg, 构建产物, resources, app | auto | - | active | 性能测试中需要获取原始二进制做行为测试。尝试 `yarn tauri build --no-bundle` 后发现 `b… |
| build/tauri-build-bundle.md#触发场景 | build | 触发场景 | tauri, bundle, dmg, 构建产物, resources, app | auto | - | active | Tauri macOS 构建时使用 `yarn tauri build --no-bundle` 时，只产生裸二进制 `… |
| build/tauri-build-bundle.md#适用 | build | 适用 | tauri, bundle, dmg, 构建产物, resources, app | auto | - | active | - Tauri macOS 应用打包 - CI/CD 中需确保 .app 生成 - 区分二进制构建 vs app bun… |
| build/tauri-build-bundle.md#陷阱 & 正解 | build | 陷阱 & 正解 | tauri, bundle, dmg, 构建产物, resources, app | auto | - | active | ❌ **陷阱**：假设 `--no-bundle` 仅跳过签名/通证，仍产 `.app`  ```bash yarn t… |
| build/vite-at-alias-manual.md#关联 | build | 关联 | vite, alias, @, resolveAlias, paths, tsconfig | auto | - | active / →shadcn-add-verify-deps | [[shadcn-add-verify-deps]] (同任务 cva 依赖)  --- |
| build/vite-at-alias-manual.md#反例 | build | 反例 | vite, alias, @, resolveAlias, paths, tsconfig | auto | - | active | ❌ 只配 vite alias 不配 tsconfig → 类型检查报错 ❌ 用相对路径 `../../componen… |
| build/vite-at-alias-manual.md#案例 | build | 案例 | vite, alias, @, resolveAlias, paths, tsconfig | auto | - | active | - shadcn-infra task: shadcn 生成的组件含 `import @/components/xxx`… |
| build/vite-at-alias-manual.md#触发场景 | build | 触发场景 | vite, alias, @, resolveAlias, paths, tsconfig | auto | - | active | 使用 shadcn/ui 或其他假设存在 `@` 别名的库时，项目原无 `@` → `src` 的路径别名配置，导致 `… |
| build/vite-at-alias-manual.md#适用 | build | 适用 | vite, alias, @, resolveAlias, paths, tsconfig | auto | - | active | shadcn/ui 迁移、Vite 从零配置、路径别名标准化 |
| build/vite-at-alias-manual.md#陷阱-正解 | build | 陷阱-正解 | vite, alias, @, resolveAlias, paths, tsconfig | auto | - | active | - **陷阱**: shadcn 假设 vite 已有 `@` 别名（标准 scaffolding 如 Vite 默认模… |
| build/wire-protocol-gate-is-failfast.md#MUST 硬约束 | build | MUST 硬约束 | - | auto | - | active | is_valid_wire_protocol gate 触发（502）说明 endpoint 选择失败（matched_… |
| build/wire-protocol-gate-is-failfast.md#关联 | build | 关联 | - | auto | - | active | - |
| build/wire-protocol-gate-is-failfast.md#反例 | build | 反例 | - | auto | - | active | - 只修白名单而未修 select → 新协议仍 502（根因未除） - 误判为 endpoint 配置缺 protoc… |
| build/wire-protocol-gate-is-failfast.md#案例 | build | 案例 | - | auto | - | active / →protocol-wire-str | - converter-reasoning-content bug1：preset 未加载致 matched_ep=No… |
| build/wire-protocol-gate-is-failfast.md#适用 | build | 适用 | - | auto | - | active | - 所有 502 route fail 场景 - is_valid_wire_protocol gate 触发 - en… |
| db/filter-semantics.md#排斥列默认过滤需明确确认为产品设计意图 | db | 排斥列默认过滤需明确确认为产品设计意图 | filter, semantics, sql, WHERE, query, db | auto | - | active | 当 task 涉及「默认排斥某类请求」的过滤逻辑时（如 Logs 主页默认隐藏 test/quota 请求），确认这是*… |
| db/pagination-offset.md#LIMIT+1 探测分页无精确总数 | db | LIMIT+1 探测分页无精确总数 | pagination, offset, limit, sqlite, 分页 | auto | - | active | 当分页 UI 仅需「有无下一页」而不需精确总数时，改用 LIMIT offset+pageSize+1 探测有下一页，而… |
| db/sqlite-cache-residency-probe-method.md#SQLite 页缓存常驻量的直接探针方法 | db | SQLite 页缓存常驻量的直接探针方法 | sqlite,page-cache,measurement,heap,malloc,probe | auto | - | active | - |
| db/sqlite-cache-residency-probe-method.md#页缓存常驻量探针 | db | 页缓存常驻量探针 | sqlite,page-cache,measurement,heap,malloc,probe | auto | - | active / →measure-window-exclusive-env,sqlite-cache-measurement-traps,sqlite-read-cache-config | ### 方法  用 `heap --addresses 'malloc[5k]'` 的 5KB 块数作为 SQLite … |
| db/sqlite-partial-index.md#参数化查询无法触发 partial index（字面量盲区） | db | 参数化查询无法触发 partial index（字面量盲区） | sqlite, partial, index, WHERE, 约束 | auto | - | active | SQLite 查询规划器对 partial index 的匹配仅识别 SQL 文本中的**字面量常量**谓词，不识别**… |
| domain/bundled-models-fallback.md#关联 | domain | 关联 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active / →resolve-price-now-ms,time-tiers-apply-idiom | [[time-tiers-apply-idiom]] [[resolve-price-now-ms]] |
| domain/bundled-models-fallback.md#反例 | domain | 反例 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | ```rust // ❌ 启动 seed （版本冲突、IO 阻塞） #[init] async fn on_startu… |
| domain/bundled-models-fallback.md#触发场景 | domain | 触发场景 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | 只读配置数据（models.json 价格表、platform-presets.json）需在 DB 为空或未同步时兜底… |
| domain/bundled-models-fallback.md#路径计算 | domain | 路径计算 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | `include_str!` 相对路径**从当前 .rs 文件出发**（不是 Cargo.toml 所在目录）： - `… |
| domain/bundled-models-fallback.md#适用 | domain | 适用 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | - 只读配置（定价表、平台预设、常量列表） - 冷启动不依赖 RPC / 版本同步 - DB 可能暂时为空、滞后同步的场… |
| domain/bundled-models-fallback.md#陷阱 ❌ vs 正解 ✅ | domain | 陷阱 ❌ vs 正解 ✅ | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | **陷阱1**：启动时 seed DB - ❌ `fn seed_models()` 启动期间 INSERT bundl… |
| domain/claude-code-passthrough-platform.md#Frontend (MUST) | domain | Frontend (MUST) | claude, passthrough, platform, 代理, Protocol | auto | - | active | - `api.ts` Protocol union 含 `/ "claude_code"` - `Platforms.t… |
| domain/claude-code-passthrough-platform.md#Intercept Point (MUST) | domain | Intercept Point (MUST) | claude, passthrough, platform, 代理, Protocol | auto | - | active | - 拦截点：`select_platform` 之后、`convert_request` 之前（与 mock 拦截点同区… |
| domain/claude-code-passthrough-platform.md#No Transform / No Inject (MUST) | domain | No Transform / No Inject (MUST) | claude, passthrough, platform, 代理, Protocol | auto | - | active | - 禁 `convert_request` / 禁 `build_upstream_headers` / 禁 `appl… |
| domain/claude-code-passthrough-platform.md#Original Request Capture (MUST) | domain | Original Request Capture (MUST) | claude, passthrough, platform, 代理, Protocol | auto | - | active | - `proxy.rs` handle_proxy 在 `req.into_parts()` **之前**捕获原始量（对… |
| domain/claude-code-passthrough-platform.md#Verification | domain | Verification | claude, passthrough, platform, 代理, Protocol | auto | - | active | ```bash cd src-tauri && cargo test passthrough   # URL 拼接 / … |
| domain/claude-code-passthrough-platform.md#What & When (MUST) | domain | What & When (MUST) | claude, passthrough, platform, 代理, Protocol | auto | - | active | - `Protocol::ClaudeCode`（`models.rs`，serde rename `"claude_c… |
| domain/claude-code-passthrough-platform.md#handle_passthrough Semantics (MUST) | domain | handle_passthrough Semantics (MUST) | claude, passthrough, platform, 代理, Protocol | auto | - | active | 1. **目标 URL** = `base_url` + 客户端原始 path（+ query）。**约定 CC 平台 … |
| domain/claude-code-passthrough-platform.md#proxy_log (MUST) | domain | proxy_log (MUST) | claude, passthrough, platform, 代理, Protocol | auto | - | active | - 透传分支**正常记** `proxy_log`：   - `source_protocol` = `target_p… |
| domain/coding-plan-no-public-quota-api.md#coding-plan-no-public-quota-api | domain | coding-plan-no-public-quota-api | coding-plan, quota, public, 端点, glm_coding | auto | - | active | bailian/qianfan/xiaomi/compshare 等 coding plan 订阅制平台上游均无公开程序… |
| domain/converter-normalized-intermediate.md#关联 | domain | 关联 | converter NonStreamResponse parse render protocol | auto | - | active / →five-wire-protocols-anchor | [[five-wire-protocols-anchor]] |
| domain/converter-normalized-intermediate.md#反例 | domain | 反例 | converter NonStreamResponse parse render protocol | auto | - | active | - 点对点设计：新增协议时改 N 处 → O(N²) 维护成本 - 无中间归一：无法跨协议组合（如 openai→gem… |
| domain/converter-normalized-intermediate.md#案例 | domain | 案例 | converter NonStreamResponse parse render protocol | auto | - | active | - converter-reasoning-content：5×5 互转矩阵用 NonStreamResponse - … |
| domain/converter-normalized-intermediate.md#覆盖范围 | domain | 覆盖范围 | converter NonStreamResponse parse render protocol | auto | - | active | - 当前：openai → anthropic 真转换（convert_response） - 其余组合：回退透传（re… |
| domain/converter-normalized-intermediate.md#触发场景 | domain | 触发场景 | converter NonStreamResponse parse render protocol | auto | - | active | - N 协议互转设计选择：内部归一（路 A）vs 点对点（路 B） - O(N) parse + render vs O… |
| domain/converter-normalized-intermediate.md#设计决策 | domain | 设计决策 | converter NonStreamResponse parse render protocol | auto | - | active | 路 A（内部归一）： 1. 上游响应 → parse → NonStreamResponse（归一） 2. NonStr… |
| domain/converter-normalized-intermediate.md#适用 | domain | 适用 | converter NonStreamResponse parse render protocol | auto | - | active | - converter 模块扩展（新增协议/转换组合） - N×N 互转矩阵设计（converter-reasoning… |
| domain/converter-normalized-intermediate.md#陷阱-正解 | domain | 陷阱-正解 | converter NonStreamResponse parse render protocol | auto | - | active | - ❌ 路 B：点对点 N×N 函数 → 新增协议需加 N 个函数 - ✅ 路A：NonStreamResponse 作… |
| domain/cpa-oauth-credential-format.md#Cross-ref | domain | Cross-ref | cpa, oauth, credential, format, token | auto | - | active / →db-split-access-point-audit | - `src-tauri/crates/aidog_core/src/gateway/cpa_import/parser… |
| domain/cpa-oauth-credential-format.md#OAuth 类型枚举（CpaOAuthType） | domain | OAuth 类型枚举（CpaOAuthType） | cpa, oauth, credential, format, token | auto | - | active | codex / claude / kimi / xai / vertex / aistudio / antigravit… |
| domain/cpa-oauth-credential-format.md#多账号语义（CLIProxyAPI） | domain | 多账号语义（CLIProxyAPI） | cpa, oauth, credential, format, token | auto | - | active / →db-split-access-point-audit | - 同一 OAuth 类型(如 xai)可有多个凭据(各 email 不同)→ **各自独立平台**(负载均衡) - d… |
| domain/cpa-oauth-credential-format.md#格式结构 | domain | 格式结构 | cpa, oauth, credential, format, token | auto | - | active | CLIProxyAPI OAuth 凭据 JSON(auth-dir 文件 / 导出 zip 内): ```json {… |
| domain/cpa-oauth-credential-format.md#识别逻辑 | domain | 识别逻辑 | cpa, oauth, credential, format, token | auto | - | active | - `parse_oauth_json(content) -> Option<Vec<CpaProvider>>`(pa… |
| domain/endpoint-cross-protocol-fallback.md#关联 | domain | 关联 | - | auto | - | active | - |
| domain/endpoint-cross-protocol-fallback.md#分层不变量 | domain | 分层不变量 | - | auto | - | active | - 回退仅在普通平台生效：普通平台允许跨协议回退（降低 502 率） - coding 平台永不落非 coding：步骤… |
| domain/endpoint-cross-protocol-fallback.md#反例 | domain | 反例 | - | auto | - | active | - ❌ 误判：coding 平台也跨协议回退 → 破坏 401 防护 - ❌ 误修：只修普通平台回退，忘了 coding… |
| domain/endpoint-cross-protocol-fallback.md#案例 | domain | 案例 | - | auto | - | active / →wire-protocol-gate-is-failfast,wire-protocol-whitelist-sync | - endpoint-cross-protocol-fallback task：普通平台步骤 4 泛化（同协议 > op… |
| domain/endpoint-cross-protocol-fallback.md#触发场景 | domain | 触发场景 | - | auto | - | active | - 普通平台 endpoint 选择时协议不匹配（如 anthropic 入站 + 仅 openai endpoint）… |
| domain/endpoint-cross-protocol-fallback.md#适用 | domain | 适用 | - | auto | - | active | - endpoint.rs select_endpoint_for_protocol 修改 - 跨协议回退逻辑扩展 - … |
| domain/endpoint-cross-protocol-fallback.md#陷阱-正解 | domain | 陷阱-正解 | - | auto | - | active | **陷阱**: 误以为跨协议回退可应用于所有平台类型，或回退优先级混乱。  **正解**: 普通平台步骤 4 泛化为三级… |
| domain/five-wire-protocols-anchor.md#关联 | domain | 关联 | protocol endpoint converter platform_type | auto | - | active / →protocol-wire-str,reasoning-content-as-text-block | [[protocol-wire-str]] [[reasoning-content-as-text-block]] |
| domain/five-wire-protocols-anchor.md#关键不变量 | domain | 关键不变量 | protocol endpoint converter platform_type | auto | - | active | endpoint 协议 = converter 模块支持的格式（convert_request + parse_sse） |
| domain/five-wire-protocols-anchor.md#反例 | domain | 反例 | protocol endpoint converter platform_type | auto | - | active | - 把 glm/kimi/sensenova 当作 endpoint 协议 → 转换时 panic/未实现 - 误以为有… |
| domain/five-wire-protocols-anchor.md#案例 | domain | 案例 | protocol endpoint converter platform_type | auto | - | active | - converter-reasoning-content task：5 协议是 N×N 互转矩阵的锚点 - glm/k… |
| domain/five-wire-protocols-anchor.md#触发场景 | domain | 触发场景 | protocol endpoint converter platform_type | auto | - | active | - endpoint 协议层只 5 种（anthropic/openai/openai_responses/openai… |
| domain/five-wire-protocols-anchor.md#适用 | domain | 适用 | protocol endpoint converter platform_type | auto | - | active | - converter 模块扩展（新增 wire protocol） - N×N 协议互转设计（真值源） - 平台接入时… |
| domain/five-wire-protocols-anchor.md#陷阱-正解 | domain | 陷阱-正解 | protocol endpoint converter platform_type | auto | - | active | - ❌ 混淆：以为所有 Protocol 枚举值都是「协议」 - ✅ 区分：仅 5 个可作为 endpoint 协议参与… |
| domain/mock-platform-contract.md#Config Carrier — extra.mock (MUST) | domain | Config Carrier — extra.mock (MUST) | mock, platform, contract, 测试, placeholder | auto | - | active | - mock 配置载体必须为现有 `platform.extra`（TEXT JSON 列），禁新增专用 DB 列（零迁… |
| domain/mock-platform-contract.md#Response Builders (MUST) | domain | Response Builders (MUST) | mock, platform, contract, 测试, placeholder | auto | - | active | - 非流式: `build_response(cfg, source_protocol, model)` 按 5 协议返… |
| domain/mock-platform-contract.md#Three-Layer Config Override (MUST) | domain | Three-Layer Config Override (MUST) | mock, platform, contract, 测试, placeholder | auto | - | active | 最终生效值 = 逐字段按优先级取首个存在者（`resolve_mock_config(extra, chat_req, … |
| domain/mock-platform-contract.md#Verification | domain | Verification | mock, platform, contract, 测试, placeholder | auto | - | active | ```bash cd src-tauri && cargo test mock   # 全部通过（三层覆盖 / 5 协议… |
| domain/mock-platform-contract.md#What & When (MUST) | domain | What & When (MUST) | mock, platform, contract, 测试, placeholder | auto | - | active | - `Protocol::Mock`（`models.rs`，serde rename `"mock"`）是**平台主类… |
| domain/mock-platform-contract.md#error_mode Semantics (MUST) | domain | error_mode Semantics (MUST) | mock, platform, contract, 测试, placeholder | auto | - | active | `handle_mock`（proxy.rs）按 `error_mode` 分派，两类语义并存（delay 与 erro… |
| domain/mock-platform-contract.md#proxy_log (MUST) | domain | proxy_log (MUST) | mock, platform, contract, 测试, placeholder | auto | - | active | - mock 分支直接写最终生效值 `log.{input_tokens,output_tokens,cache_tok… |
| domain/prd-acceptance-consistency-check.md#PRD 验收标准与约束互容性检查 | domain | PRD 验收标准与约束互容性检查 | PRD,acceptance,constraint,compatibility,plan | auto | - | active | - |
| domain/prd-acceptance-consistency-check.md#PRD 验收标准与约束互容性检查 | domain | PRD 验收标准与约束互容性检查 | PRD,acceptance,constraint,compatibility,plan | auto | - | active / →mock-platform-bypasses-forward-pipeline | ### 触发场景  task plan 阶段定下验收标准（如「phys_footprint 下降」）和技术约束（如「仅用… |
| domain/protocol-logo-fallback-chain.md#HTTP client (MUST) | domain | HTTP client (MUST) | protocol, logo, fallback, icon, 协议 | auto | - | active | - **MUST 复用 `build_http_client_system`** (非 `build_http_clie… |
| domain/protocol-logo-fallback-chain.md#presets JSON 读取 (MUST) | domain | presets JSON 读取 (MUST) | protocol, logo, fallback, icon, 协议 | auto | - | active | - `read_local_presets_json` 优先级: `~/.aidog/platform-presets.… |
| domain/protocol-logo-fallback-chain.md#三路 fallback 顺序 (MUST, 首成功即止) | domain | 三路 fallback 顺序 (MUST, 首成功即止) | protocol, logo, fallback, icon, 协议 | auto | - | active | 固定顺序, **禁重排**, 见 `sync_one_into`:  1. **simpleicons CDN** — … |
| domain/protocol-logo-fallback-chain.md#入口 | domain | 入口 | protocol, logo, fallback, icon, 协议 | auto | - | active | - `sync_all_logos(db, app_data_dir)` — 后台批量同步 (app 启动 / 手动触发… |
| domain/protocol-logo-fallback-chain.md#关联 | domain | 关联 | protocol, logo, fallback, icon, 协议 | auto | - | active | - [http-client-forward.md](./http-client-forward.md) — build… |
| domain/protocol-logo-fallback-chain.md#缓存契约 (MUST) | domain | 缓存契约 (MUST) | protocol, logo, fallback, icon, 协议 | auto | - | active | - 缓存路径 `~/.aidog/logos/<protocol_id>.png` (`logo_cache_path`… |
| domain/protocol-logo-fallback-chain.md#验收基准 (可复用) | domain | 验收基准 (可复用) | protocol, logo, fallback, icon, 协议 | auto | - | active | - [ ] 清空 `~/.aidog/logos/` 后, 有 `logo_url` 的 protocol 命中路 1;… |
| domain/protocol-logo-fallback-chain.md#验证命令 | domain | 验证命令 | protocol, logo, fallback, icon, 协议 | auto | - | active | ```bash # 三路 URL 模板存在且顺序 grep -n "cdn.simpleicons.org\//favi… |
| domain/reasoning-content-as-text-block.md#关联 | domain | 关联 | reasoning thinking anthropic signature converter | auto | - | active / →five-wire-protocols-anchor,reasoning-content-as-text-block | [[reasoning-content-as-text-block]] [[five-wire-protocols-an… |
| domain/reasoning-content-as-text-block.md#决策背景 | domain | 决策背景 | reasoning thinking anthropic signature converter | auto | - | active | - TrueFoundry/LiteLLM #8927 调研佐证：第三方 reasoning 无 signature -… |
| domain/reasoning-content-as-text-block.md#反例 | domain | 反例 | reasoning thinking anthropic signature converter | auto | - | active | - 强行出 thinking 块 → CC 多轮交互时 400/empty or malformed - 空 reaso… |
| domain/reasoning-content-as-text-block.md#实现 | domain | 实现 | reasoning thinking anthropic signature converter | auto | - | active | - openai/response.rs:13：reasoning_content 被忽略，不影响 content/to… |
| domain/reasoning-content-as-text-block.md#触发场景 | domain | 触发场景 | reasoning thinking anthropic signature converter | auto | - | active | - 第三方（deepseek/sensenova/glm）reasoning_content 纯文本无 signatur… |
| domain/reasoning-content-as-text-block.md#适用 | domain | 适用 | reasoning thinking anthropic signature converter | auto | - | active | - 所有第三方 → anthropic 跨协议转换 - reasoning 扩展字段处理（未来第三方新增非标准字段） |
| domain/reasoning-content-as-text-block.md#陷阱-正解 | domain | 陷阱-正解 | reasoning thinking anthropic signature converter | auto | - | active | - ❌ 方案 A（标准协议）：出 thinking 块 → signature 风险 - ✅ 方案 B（务实方案）：re… |
| domain/task-decomposition-coverage-check.md#task 分解 → subtask DAG 覆盖检查 | domain | task 分解 → subtask DAG 覆盖检查 | subtask,PRD,coverage,decomposition,plan | auto | - | active | ### 触发场景  task 分解拆 subtask DAG 时。某次 task 有 7 个明确的目标（PRD），但原拆… |
| domain/task-decomposition-coverage-check.md#task 分解 → subtask DAG 覆盖检查 | domain | task 分解 → subtask DAG 覆盖检查 | subtask,PRD,coverage,decomposition,plan | auto | - | active | - |
| domain/time-tiers-apply-idiom.md#关联 | domain | 关联 | time_tiers, 定价分档, 嵌套价表, 时间维度 | auto | - | active / →bundled-models-fallback,peak-multiplier-symmetry,resolve-price-now-ms | [[resolve-price-now-ms]] [[peak-multiplier-symmetry]] [[bund… |
| domain/time-tiers-apply-idiom.md#反例 | domain | 反例 | time_tiers, 定价分档, 嵌套价表, 时间维度 | auto | - | active | ```rust // ❌ 顺序首命中 + 扁平相加 let tier = tiers.iter().find(/t/ t… |
| domain/time-tiers-apply-idiom.md#案例 | domain | 案例 | time_tiers, 定价分档, 嵌套价表, 时间维度 | auto | - | active | **glm-5-turbo 时段+长文档**： - base: 32k 档 = 2e-6 $/token（普通价） - … |
| domain/time-tiers-apply-idiom.md#触发场景 | domain | 触发场景 | time_tiers, 定价分档, 嵌套价表, 时间维度 | auto | - | active | 模型定价加入时间维度（同一个模型不同时段不同价格）。需要表达二维定价：时间 + 内容长度。 |
| domain/time-tiers-apply-idiom.md#适用 | domain | 适用 | time_tiers, 定价分档, 嵌套价表, 时间维度 | auto | - | active | - 模型单价时间分档（glm_coding 早高峰 ×3.0 + 0-24 ×2.0） - 平台级时段价（某云商服务某时… |
| domain/time-tiers-apply-idiom.md#陷阱 ❌ vs 正解 ✅ | domain | 陷阱 ❌ vs 正解 ✅ | time_tiers, 定价分档, 嵌套价表, 时间维度 | auto | - | active | **陷阱1**：time_tiers 数组用顺序首命中 - ❌ `tiers[0]` 如果 start_at 符合就用，… |
| frontend/css-var-alias-layer.md#CSS var live resolution 别名层 | frontend | CSS var live resolution 别名层 | CSS,变量,Tailwind,layer,cascade | auto | - | active | CSS 变量改名迁移时，用 :root 别名层实现 live resolution，替代批量 sed 替换。 |
| frontend/css-var-alias-layer.md#Tailwind cascade layer: 裸写规则反压 layer 内 utility | frontend | Tailwind cascade layer: 裸写规则反压 layer 内 utility | CSS,变量,Tailwind,layer,cascade | auto | - | active | Tailwind v4 项目里若分层导入 CSS，任何裸写（不在 `@layer` 块内）的规则优先级都高于 layer… |
| frontend/css-var-alias-layer.md#css-var-alias-layer | frontend | css-var-alias-layer | CSS,变量,Tailwind,layer,cascade | auto | - | active | - |
| frontend/css-var-alias-layer.md#关联 | frontend | 关联 | CSS,变量,Tailwind,layer,cascade | auto | - | active / →semantic-token-foreground-pairing | [[semantic-token-foreground-pairing]] |
| frontend/css-var-alias-layer.md#关联 | frontend | 关联 | CSS,变量,Tailwind,layer,cascade | auto | - | active / →theme-token-runtime-switch | [[theme-token-runtime-switch]] |
| frontend/css-var-alias-layer.md#对比 | frontend | 对比 | CSS,变量,Tailwind,layer,cascade | auto | - | active | / 方式 / 改动量 / 误伤风险 / 回滚 / /------/--------/---------/------/ … |
| frontend/css-var-alias-layer.md#案例 | frontend | 案例 | CSS,变量,Tailwind,layer,cascade | auto | - | active | frontend-compositing-purge task：commit c3f9515e 裸写 UA reset … |
| frontend/css-var-alias-layer.md#案例 | frontend | 案例 | CSS,变量,Tailwind,layer,cascade | auto | - | active | shadcn-infra task: 主题变量改名用别名层，globals.css 加 10 行 vs sed 700+… |
| frontend/css-var-alias-layer.md#检查 | frontend | 检查 | CSS,变量,Tailwind,layer,cascade | auto | - | active | globals.css 顶部若见 `@layer <names>;` 声明 + `@import ... layer(.… |
| frontend/css-var-alias-layer.md#正解 | frontend | 正解 | CSS,变量,Tailwind,layer,cascade | auto | - | active | 1. 在 :root 定义别名：`--legacy: var(--shadcn);` 2. 所有引用用旧名 `--leg… |
| frontend/css-var-alias-layer.md#正解 | frontend | 正解 | CSS,变量,Tailwind,layer,cascade | auto | - | active | 补 UA reset 规则必须包进 `@layer base {}` 块，与 globals.css 顶部声明的 lay… |
| frontend/css-var-alias-layer.md#适用 | frontend | 适用 | CSS,变量,Tailwind,layer,cascade | auto | - | active | CSS 变量迁移、主题重构、大型 CSS 重构中间状态 |
| frontend/css-var-alias-layer.md#适用 | frontend | 适用 | CSS,变量,Tailwind,layer,cascade | auto | - | active | Tailwind v4 + cascade layer 项目，补 preflight/UA reset 规则时。 |
| frontend/css-var-alias-layer.md#陷阱 | frontend | 陷阱 | CSS,变量,Tailwind,layer,cascade | auto | - | active | 补 preflight 缺失的 UA reset（如 button/input/select 色继承）时若裸写在 glo… |
| frontend/dirty-float-hour-normalization.md#dirty-float-hour-normalization | frontend | dirty-float-hour-normalization | Number.isInteger,浮点,hour,minute,splitFraction | auto | - | active | - |
| frontend/dirty-float-hour-normalization.md#关联 | frontend | 关联 | Number.isInteger,浮点,hour,minute,splitFraction | auto | - | active / →modal-state-architecture,time-zone-minute-arithmetic | [[time-zone-minute-arithmetic]]、[[modal-state-architecture]] |
| frontend/dirty-float-hour-normalization.md#前端读取路径归一（关键） | frontend | 前端读取路径归一（关键） | Number.isInteger,浮点,hour,minute,splitFraction | auto | - | active | ### MUST 单点归一（parse 层）  ```ts /** 存量非整数 start_hour/end_hour（… |
| frontend/dirty-float-hour-normalization.md#单测覆盖（脏数据拆分规则） | frontend | 单测覆盖（脏数据拆分规则） | Number.isInteger,浮点,hour,minute,splitFraction | auto | - | active | - 整数 hour 原样不变 - 8.0 视为整数不变 - 8.5 拆分为 8:30 - 已有 start_minute… |
| frontend/dirty-float-hour-normalization.md#反例 / 常见错误 | frontend | 反例 / 常见错误 | Number.isInteger,浮点,hour,minute,splitFraction | auto | - | active | / 错误                          / 为什么错                        … |
| frontend/dirty-float-hour-normalization.md#脏数据入库时归一 — 浮点 hour 拆分为整数 hour+minute | frontend | 脏数据入库时归一 — 浮点 hour 拆分为整数 hour+minute | Number.isInteger,浮点,hour,minute,splitFraction | auto | - | active | 系统升级或跨版本迁移中，存量数据可能包含脏数据。例如，旧版本按整小时换算时产生 `start_hour: 8.5`（半时… |
| frontend/dirty-float-hour-normalization.md#适用 | frontend | 适用 | Number.isInteger,浮点,hour,minute,splitFraction | auto | - | active | - 版本升级中的数据兼容性问题 - 存量脏数据前端吸收而非后端永久兼容 |
| frontend/dirty-float-hour-normalization.md#陷阱：后端 migration 改 serde 类型成本高，数据污染持久 | frontend | 陷阱：后端 migration 改 serde 类型成本高，数据污染持久 | Number.isInteger,浮点,hour,minute,splitFraction | auto | - | active | 旧版本：`peak_hours` 整小时换算，半时区用户产生 `start_hour: 8.5` 写入 JSON。后端声… |
| frontend/form-level-tz-state-sharing.md#MUST 单一真值源（表单级 state） | frontend | MUST 单一真值源（表单级 state） | 表单,时区,状态共用,peak_hours | auto | - | active | ✅ **表单级单一 state 透传**  ```ts // usePlatformForm.ts：表单级 hook e… |
| frontend/form-level-tz-state-sharing.md#form-level-tz-state-sharing | frontend | form-level-tz-state-sharing | 表单,时区,状态共用,peak_hours | auto | - | active | - |
| frontend/form-level-tz-state-sharing.md#关联 | frontend | 关联 | 表单,时区,状态共用,peak_hours | auto | - | active / →dirty-float-hour-normalization,time-zone-minute-arithmetic | [[time-zone-minute-arithmetic]]、[[dirty-float-hour-normaliza… |
| frontend/form-level-tz-state-sharing.md#表单级时区状态共用 — 单一 state 透传避免口径漂移 | frontend | 表单级时区状态共用 — 单一 state 透传避免口径漂移 | 表单,时区,状态共用,peak_hours | auto | - | active | 同一表单内多个组件展示同一类数据不同维度时，需要单一 state 透传避免口径漂移。 |
| frontend/form-level-tz-state-sharing.md#适用 | frontend | 适用 | 表单,时区,状态共用,peak_hours | auto | - | active | - 表单内多个子组件需同步状态的场景 - peak_hours + time_models 编辑器一致性 |
| frontend/form-level-tz-state-sharing.md#陷阱：各组件独立 state 导致口径漂移 | frontend | 陷阱：各组件独立 state 导致口径漂移 | 表单,时区,状态共用,peak_hours | auto | - | active | PlatformEditForm 编辑单个平台。peak_hours 与 time_models 都含「时段」结构，都需… |
| frontend/frontend-conventions.md#API Layer (MUST) | frontend | API Layer (MUST) | 前端,约定,conventions,强制规则 | auto | - | active | - invoke 契约需严格遵守（泛型标注 / 集中 api/ 目录 / 字段名 snake_case） - API n… |
| frontend/frontend-conventions.md#CRUD 刷新链契约 (MUST) | frontend | CRUD 刷新链契约 (MUST) | 前端,约定,conventions,强制规则 | auto | - | active | - **全入口扫描**: `platformApi.delete` 等后端真删的 CRUD 入口的全调用点 MUST g… |
| frontend/frontend-conventions.md#Component Patterns (MUST) | frontend | Component Patterns (MUST) | 前端,约定,conventions,强制规则 | auto | - | active | - 页面组件必须 `export function <PascalCase>()`，用 named export - 共… |
| frontend/frontend-conventions.md#Directory Structure (MUST) | frontend | Directory Structure (MUST) | 前端,约定,conventions,强制规则 | auto | - | active | - 新页面必须放 `src/pages/<PascalCase>.tsx` - 共享组件放 `src/component… |
| frontend/frontend-conventions.md#Large File Split — facade 模式 (MUST) | frontend | Large File Split — facade 模式 (MUST) | 前端,约定,conventions,强制规则 | auto | - | active | >800 行文件统一走 facade + 子目录模式：  - **facade 保留同名 export**: 拆后 `<… |
| frontend/frontend-conventions.md#State Management (MUST) | frontend | State Management (MUST) | 前端,约定,conventions,强制规则 | auto | - | active | - 全局设置（locale / theme）必须走 `AppContext` + `useApp()` hook - 禁… |
| frontend/frontend-conventions.md#frontend-conventions | frontend | frontend-conventions | 前端,约定,conventions,强制规则 | auto | - | active | - |
| frontend/frontend-conventions.md#i18n (MUST) | frontend | i18n (MUST) | 前端,约定,conventions,强制规则 | auto | - | active | - 所有用户可见文案必须用 `t("key")` - 新增任一 key 必须 8 locale 全补（zh-Hans/e… |
| frontend/frontend-conventions.md#关联 | frontend | 关联 | 前端,约定,conventions,强制规则 | auto | - | active / →i18n-key-eight-locales,modal-state-architecture,tauri-drag-drop-api | [[tauri-drag-drop-api]]、[[modal-state-architecture]]、[[i18n-… |
| frontend/frontend-conventions.md#前端 conventions 强制规则 | frontend | 前端 conventions 强制规则 | 前端,约定,conventions,强制规则 | auto | - | active | 前端代码变更必须遵循约定，确保与现有模式一致，减少增量成本。 |
| frontend/modal-state-architecture.md#PlatformEditForm Modal 架构模式 | frontend | PlatformEditForm Modal 架构模式 | Modal,架构,PlatformEditForm,SmartPasteModal | auto | - | active | PlatformEditForm Modal 架构需要区分两类：直接灌表单 Modal 与跨表单 Modal，state… |
| frontend/modal-state-architecture.md#modal-state-architecture | frontend | modal-state-architecture | Modal,架构,PlatformEditForm,SmartPasteModal | auto | - | active | - |
| frontend/modal-state-architecture.md#两类 Modal 区分 | frontend | 两类 Modal 区分 | Modal,架构,PlatformEditForm,SmartPasteModal | auto | - | active | ### 直接灌表单 Modal（SmartPasteModal 模式） - **State 位置**: `usePlat… |
| frontend/modal-state-architecture.md#关联 | frontend | 关联 | Modal,架构,PlatformEditForm,SmartPasteModal | auto | - | active / →form-level-tz-state-sharing,tauri-drag-drop-api | [[tauri-drag-drop-api]]、[[form-level-tz-state-sharing]] |
| frontend/modal-state-architecture.md#后续新 Modal 决策树 | frontend | 后续新 Modal 决策树 | Modal,架构,PlatformEditForm,SmartPasteModal | auto | - | active | 新 Modal (如 Sub2Api) ├─ onApply 直接填表单字段？ │  └─ 是 → SmartPaste… |
| frontend/modal-state-architecture.md#架构原则 | frontend | 架构原则 | Modal,架构,PlatformEditForm,SmartPasteModal | auto | - | active | 1. **Modal 直接操作表单字段 → state 放 hook，通过 PlatformPasteCtx 传 set… |
| frontend/modal-state-architecture.md#验收 | frontend | 验收 | Modal,架构,PlatformEditForm,SmartPasteModal | auto | - | active | - [ ] grep `showCpaImport` / `showPaste` 在 PlatformEditForm … |
| frontend/platform-creation-entry-consolidation.md#cli-proxy 平台创建入口唯一性 | frontend | cli-proxy 平台创建入口唯一性 | cli-proxy,平台创建,入口,CliProxy | auto | - | active | cli-proxy 平台的创建路径需要统一化，唯一入口是 CliProxy 页的「建平台行」按钮。 |
| frontend/platform-creation-entry-consolidation.md#platform-creation-entry-consolidation | frontend | platform-creation-entry-consolidation | cli-proxy,平台创建,入口,CliProxy | auto | - | active | - |
| frontend/platform-creation-entry-consolidation.md#关联 | frontend | 关联 | cli-proxy,平台创建,入口,CliProxy | auto | - | active / →i18n-key-deletion-safety,modal-state-architecture | [[i18n-key-deletion-safety]]、[[modal-state-architecture]] |
| frontend/platform-creation-entry-consolidation.md#反例 | frontend | 反例 | cli-proxy,平台创建,入口,CliProxy | auto | - | active | ❌ 在 PlatformEditForm 新建态混入「从 cli-proxy 导入」选项 → 创建路径分裂 ❌ 允许多个… |
| frontend/platform-creation-entry-consolidation.md#正解 | frontend | 正解 | cli-proxy,平台创建,入口,CliProxy | auto | - | active | - 添加平台表单（PlatformEditForm）只用于编辑现有平台 - 创建新 cli-proxy 平台必须走 Cl… |
| frontend/platform-creation-entry-consolidation.md#约束 | frontend | 约束 | cli-proxy,平台创建,入口,CliProxy | auto | - | active | cli-proxy 平台的唯一创建入口是 **CliProxy 页 src/pages/CliProxy/index.t… |
| frontend/platform-creation-entry-consolidation.md#适用 | frontend | 适用 | cli-proxy,平台创建,入口,CliProxy | auto | - | active | - CLI Proxy 平台管理流程设计 - 添加平台表单重构 |
| frontend/semantic-token-foreground-pairing.md#MUST 约束 | frontend | MUST 约束 | 语义色,token,foreground,对比度 | auto | - | active | 修对比度缺陷时**禁改 `--accent` 等语义色 token 的值本身**，只能改配对的 `-foreground… |
| frontend/semantic-token-foreground-pairing.md#semantic-token-foreground-pairing | frontend | semantic-token-foreground-pairing | 语义色,token,foreground,对比度 | auto | - | active | - |
| frontend/semantic-token-foreground-pairing.md#关联 | frontend | 关联 | 语义色,token,foreground,对比度 | auto | - | active / →tailwind-cascade-layer-base | [[tailwind-cascade-layer-base]] |
| frontend/semantic-token-foreground-pairing.md#案例 | frontend | 案例 | 语义色,token,foreground,对比度 | auto | - | active | frontend-compositing-purge task 对比度审计：dark `--accent-foregro… |
| frontend/semantic-token-foreground-pairing.md#正解 | frontend | 正解 | 语义色,token,foreground,对比度 | auto | - | active | 逐处核对 `bg-X`/`-foreground` 组合对比度，修改 foreground 侧色值不修改 accent/… |
| frontend/semantic-token-foreground-pairing.md#语义色 token 必须成对达标对比度 | frontend | 语义色 token 必须成对达标对比度 | 语义色,token,foreground,对比度 | auto | - | active | 任何语义色 `bg-X` token 都必须配达标对比度的 `-foreground` token。本项目 `--acc… |
| frontend/semantic-token-foreground-pairing.md#陷阱 | frontend | 陷阱 | 语义色,token,foreground,对比度 | auto | - | active | 补 preflight 缺失的 UA reset 时若改了语义色 token 本值（如 `--accent` 色），会连… |
| frontend/tauri-drag-drop-api.md#MUST 用 Tauri onDragDropEvent，禁 HTML5 onDrop | frontend | MUST 用 Tauri onDragDropEvent，禁 HTML5 onDrop | Tauri,拖拽,drag,drop,WKWebView | auto | - | active | macOS WKWebView 的 HTML5 `drop` 事件不触发。Tauri `getCurrentWebvie… |
| frontend/tauri-drag-drop-api.md#Tauri 拖拽事件 API（macOS WKWebView 限制） | frontend | Tauri 拖拽事件 API（macOS WKWebView 限制） | Tauri,拖拽,drag,drop,WKWebView | auto | - | active | Tauri 前端实现文件拖拽导入时，必须使用 Tauri `onDragDropEvent`，禁用 HTML5 onDr… |
| frontend/tauri-drag-drop-api.md#event.payload.type | frontend | event.payload.type | Tauri,拖拽,drag,drop,WKWebView | auto | - | active | - enter/over: paths[] → 高亮判断 - drop: paths[] → 取目标文件 - leave… |
| frontend/tauri-drag-drop-api.md#关联 | frontend | 关联 | Tauri,拖拽,drag,drop,WKWebView | auto | - | active / →modal-state-architecture | [[modal-state-architecture]] |
| frontend/tauri-drag-drop-api.md#约束 | frontend | 约束 | Tauri,拖拽,drag,drop,WKWebView | auto | - | active | - 禁混 HTML5 onDrop（macOS WKWebView 不触发） - MUST unlisten（clean… |
| frontend/tauri-drag-drop-api.md#范本 | frontend | 范本 | Tauri,拖拽,drag,drop,WKWebView | auto | - | active | ```typescript useEffect(() => {   let unlisten: (() => void)… |
| frontend/tauri-drag-drop-api.md#适用 | frontend | 适用 | Tauri,拖拽,drag,drop,WKWebView | auto | - | active | Tauri 文件拖拽导入、跨平台拖拽 |
| frontend/theme-token-runtime-switch.md#shadcn token 运行时切换 | frontend | shadcn token 运行时切换 | shadcn,主题,token,运行时 | auto | - | active | shadcn 主题 token 在运行时动态切换时，用 `setProperty` inline 方式，无需 !impo… |
| frontend/theme-token-runtime-switch.md#theme-token-runtime-switch | frontend | theme-token-runtime-switch | shadcn,主题,token,运行时 | auto | - | active | - |
| frontend/theme-token-runtime-switch.md#关联 | frontend | 关联 | shadcn,主题,token,运行时 | auto | - | active / →css-var-alias-layer | [[css-var-alias-layer]] |
| frontend/theme-token-runtime-switch.md#反例 | frontend | 反例 | shadcn,主题,token,运行时 | auto | - | active | ❌ 用 !important 覆盖所有 token → 优先级混乱 ❌ 依赖静态 @import → 运行时无法切换 |
| frontend/theme-token-runtime-switch.md#案例 | frontend | 案例 | shadcn,主题,token,运行时 | auto | - | active | shadcn-infra task: 运行时主题切换用 setProperty inline，避免 !important |
| frontend/theme-token-runtime-switch.md#正解 | frontend | 正解 | shadcn,主题,token,运行时 | auto | - | active | 1. applyTheme 函数直接设置 CSS var：    ```ts    document.documentE… |
| frontend/theme-token-runtime-switch.md#适用 | frontend | 适用 | shadcn,主题,token,运行时 | auto | - | active | shadcn 主题运行时切换、动态主题系统、CSS var 运行时更新 |
| frontend/theme-token-runtime-switch.md#陷阱 | frontend | 陷阱 | shadcn,主题,token,运行时 | auto | - | active | - **陷阱**: 用 !important 强制覆盖 → 级联爆炸、难以维护 - **陷阱**: 依赖 @import… |
| frontend/time-zone-minute-arithmetic.md#MUST 换算公式（单位：分钟） | frontend | MUST 换算公式（单位：分钟） | 时区,换算,分钟,精度 | auto | - | active | ```ts export function shiftClock(   hour: number,    minute:… |
| frontend/time-zone-minute-arithmetic.md#time-zone-minute-arithmetic | frontend | time-zone-minute-arithmetic | 时区,换算,分钟,精度 | auto | - | active | - |
| frontend/time-zone-minute-arithmetic.md#关联 | frontend | 关联 | 时区,换算,分钟,精度 | auto | - | active / →dirty-float-hour-normalization,form-level-tz-state-sharing | [[dirty-float-hour-normalization]]、[[form-level-tz-state-sha… |
| frontend/time-zone-minute-arithmetic.md#时区换算硬约束 — 绝对分钟精度 | frontend | 时区换算硬约束 — 绝对分钟精度 | 时区,换算,分钟,精度 | auto | - | active | 前端时区显示/输入交互需与服务端一致，半时区用户（印度 UTC+5:30 等）填写时段时必须绝对分钟精度。 |
| frontend/time-zone-minute-arithmetic.md#适用 | frontend | 适用 | 时区,换算,分钟,精度 | auto | - | active | - 前端时区显示/输入交互（peak_hours / time_models 编辑器） - 任何跨时区时刻换算 |
| frontend/time-zone-minute-arithmetic.md#陷阱：按整小时换算产生非整数 | frontend | 陷阱：按整小时换算产生非整数 | 时区,换算,分钟,精度 | auto | - | active | 半时区下 UTC `8:00` 换到本地是 `8 + 5.5 = 13.5 小时`，被写进 JSON 后后端解析失败 →… |
| git/parallel-commit-scope-check.md#关联 | git | 关联 | commit,scope,validation,parallel,git-check | auto | - | active | git-worktree-parallel-isolation |
| git/parallel-commit-scope-check.md#处理流程 | git | 处理流程 | commit,scope,validation,parallel,git-check | auto | - | active | ```bash # commit 前检查 staged 文件 git diff --cached --name-only… |
| git/parallel-commit-scope-check.md#并行提交 Scope 检查 | git | 并行提交 Scope 检查 | commit,scope,validation,parallel,git-check | auto | - | active | - |
| git/parallel-commit-scope-check.md#案例 | git | 案例 | commit,scope,validation,parallel,git-check | auto | - | active | - shadcn-pages task 并行 m-groups/m-logs/m-stats 等子任务，需 commit… |
| git/parallel-commit-scope-check.md#触发场景 | git | 触发场景 | commit,scope,validation,parallel,git-check | auto | - | active | 同一 worktree 并行跑多个 subtask 时，不同 agent 可能对同一文件产生变更，导致 git inde… |
| git/parallel-commit-scope-check.md#适用 | git | 适用 | commit,scope,validation,parallel,git-check | auto | - | active | - 同 worktree 并行 subtask（skein parallel 模式） - 多 agent 同时改同一文件… |
| git/parallel-commit-scope-check.md#陷阱-正解 | git | 陷阱-正解 | commit,scope,validation,parallel,git-check | auto | - | active | ❌ **陷阱**：多个并行 subtask 各自 commit，兄弟 staged 文件可能被误入彼此的 commit（… |
| i18n/i18n-key-deletion-safety.md#i18n key 删除的安全规矩 | i18n | i18n key 删除的安全规矩 | i18n,key删除,grep,引用清零 | auto | - | active | 删除项目中的 i18n key 时，需要确保引用点已清零，避免遗留的 key 引用导致运行时错误。 |
| i18n/i18n-key-deletion-safety.md#i18n-key-deletion-safety | i18n | i18n-key-deletion-safety | i18n,key删除,grep,引用清零 | auto | - | active | - |
| i18n/i18n-key-deletion-safety.md#关联 | i18n | 关联 | i18n,key删除,grep,引用清零 | auto | - | active / →platform-creation-entry-consolidation | [[platform-creation-entry-consolidation]]（同批 task remove-cli… |
| i18n/i18n-key-deletion-safety.md#分类注意 | i18n | 分类注意 | i18n,key删除,grep,引用清零 | auto | - | active | 关键词 `platform.cliProxy.inherited*` 系列（如 `inheritedEndpoint`,… |
| i18n/i18n-key-deletion-safety.md#反例 | i18n | 反例 | i18n,key删除,grep,引用清零 | auto | - | active | ❌ 在 i18n JSON 直接删键，不检查代码里还有没有调用 → 运行时缺键报错 ❌ 只 grep 常见调用模式（如直… |
| i18n/i18n-key-deletion-safety.md#正解 | i18n | 正解 | i18n,key删除,grep,引用清零 | auto | - | active | 1. 确认该 key 的所有调用点    ```bash    grep -r "platform.cliProxy.i… |
| i18n/i18n-key-deletion-safety.md#约束 | i18n | 约束 | i18n,key删除,grep,引用清零 | auto | - | active | 删 i18n key 时必须**逐 key grep 确认引用点完全归零**。直接删文件内容是常见陷阱。 |
| i18n/i18n-key-deletion-safety.md#适用 | i18n | 适用 | i18n,key删除,grep,引用清零 | auto | - | active | - i18n 文件清理 - 界面流程重构后的 key 梳理 - 删除冗余翻译项 |
| i18n/i18n-key-eight-locales.md#8 个语言 i18n key 同步硬约束 | i18n | 8 个语言 i18n key 同步硬约束 | i18n,8语言,key同步,locale | auto | - | active | 新增 i18n key 时必须同步到所有 8 个语言文件，避免某些语言用户看到 key 原文或空白。 |
| i18n/i18n-key-eight-locales.md#MUST 硬约束 | i18n | MUST 硬约束 | i18n,8语言,key同步,locale | auto | - | active | 新增 i18n key 必须同时补齐 8 个语言文件（zh-Hans/en-US/ar-SA/fr-FR/de-DE/r… |
| i18n/i18n-key-eight-locales.md#i18n-key-eight-locales | i18n | i18n-key-eight-locales | i18n,8语言,key同步,locale | auto | - | active | - |
| i18n/i18n-key-eight-locales.md#关联 | i18n | 关联 | i18n,8语言,key同步,locale | auto | - | active / →i18n-key-deletion-safety | [[i18n-key-deletion-safety]] |
| i18n/i18n-key-eight-locales.md#处理流程 | i18n | 处理流程 | i18n,8语言,key同步,locale | auto | - | active | ```bash # 新增 key 后检查 yarn check-i18n  # 自动补齐（示例：从 zh-Hans 复制… |
| i18n/i18n-key-eight-locales.md#检查机制 | i18n | 检查机制 | i18n,8语言,key同步,locale | auto | - | active | - `check-i18n` 守门：跑 `yarn check-i18n` 检测 key 同步 - 缺失语言会导致对应语… |
| i18n/i18n-key-eight-locales.md#适用 | i18n | 适用 | i18n,8语言,key同步,locale | auto | - | active | - 所有 i18n key 新增/修改 - alert() → toast() 迁移（如 shadcn-pages ta… |
| ops/buf-residue-observability.md#关联 | ops | 关联 | observability,buffer-residue,logging,stderr,stream,SSE,debuggability | auto | - | active / →sse-chunk-stateless-defect,stream-buf-unified-cap | [[sse-chunk-stateless-defect]] 阐述缓冲架构，[[stream-buf-unified-c… |
| ops/buf-residue-observability.md#原则：不静默丢 | ops | 原则：不静默丢 | observability,buffer-residue,logging,stderr,stream,SSE,debuggability | auto | - | active | ### MUST  - **在 Drop trait 或流末清理处记 WARN log** —— 任何缓冲残留 drop… |
| ops/buf-residue-observability.md#日志等级选择 | ops | 日志等级选择 | observability,buffer-residue,logging,stderr,stream,SSE,debuggability | auto | - | active | / 场景 / 等级 / 理由 / /---/---/---/ / 正常流末有残行（客户端断连/超时） / WARN / … |
| ops/buf-residue-observability.md#缓冲残留处置·禁静默丢原则 | ops | 缓冲残留处置·禁静默丢原则 | observability,buffer-residue,logging,stderr,stream,SSE,debuggability | auto | - | active | - |
| ops/buf-residue-observability.md#缺陷根因分析 | ops | 缺陷根因分析 | observability,buffer-residue,logging,stderr,stream,SSE,debuggability | auto | - | active | SSE 流处理中，缓冲残留（流末有不完整帧）本身不是 bug —— 半帧因定义就不合法，丢弃是对的。**但静默丢弃正是这… |
| ops/buf-residue-observability.md#适用 | ops | 适用 | observability,buffer-residue,logging,stderr,stream,SSE,debuggability | auto | - | active | - 任何有状态缓冲的流式处理 - 特别是帧边界缓冲（SSE / WebSocket 等） - 异步处理中 drop 可能… |
| ops/buf-residue-observability.md#验收 | ops | 验收 | observability,buffer-residue,logging,stderr,stream,SSE,debuggability | auto | - | active | - [ ] Drop impl 存在（或流末清理函数有相关 log） - [ ] warn 日志含残留长度和必要上下文 … |
| ops/idle-wakeup-sources-inventory.md#空闲期唤醒源 6 分类清单 | ops | 空闲期唤醒源 6 分类清单 | wakeup,timers,scheduler,sources,profiling,static-analysis,cpu | auto | - | active / →idle-cpu-baseline-xctrace,measure-window-exclusive-env | 空闲期 CPU 唤醒源分 6 类，静态 rg 检索无遗漏（src-tauri + src）。  / 分类 / 频率 / … |
| ops/logging-queue-capacity-tuning.md#日志队列 capacity 定值方法：从采样均值反推 | ops | 日志队列 capacity 定值方法：从采样均值反推 | logging,queue,capacity,tuning,p99 | auto | - | active | - |
| ops/logging-queue-capacity-tuning.md#日志队列 capacity 定值方法：从采样均值反推 | ops | 日志队列 capacity 定值方法：从采样均值反推 | logging,queue,capacity,tuning,p99 | auto | - | active / →hot-path-buffers | ### 触发场景  应用日志流量稳定后需定值日志队列 capacity（mpsc channel），既要不丢日志（缓冲充… |
| ops/remote-defaults-sync-chain.md#Cross-reference | ops | Cross-reference | platform-presets,defaults,sync-chain,source-of-truth,bundled,gateway | auto | - | active | - 先例代码: `crates/aidog_core/src/gateway/defaults_sync.rs`（pla… |
| ops/remote-defaults-sync-chain.md#实例 | ops | 实例 | platform-presets,defaults,sync-chain,source-of-truth,bundled,gateway | auto | - | active | - task 07-09-*（platform-presets 同步首次落地，`defaults_sync.rs` 先例… |
| ops/remote-defaults-sync-chain.md#平台预设同步链路 | ops | 平台预设同步链路 | platform-presets,defaults,sync-chain,source-of-truth,bundled,gateway | auto | - | active | # 远端 defaults JSON 同步链范式  何时被读: 新增 `src-tauri/defaults/*.jso… |
| ops/remote-defaults-sync-chain.md#数据流架构 (MUST，禁前端直读 github) | ops | 数据流架构 (MUST，禁前端直读 github) | platform-presets,defaults,sync-chain,source-of-truth,bundled,gateway | auto | - | active | ``` github (master) ──rust sync (<x>_sync.rs)──▶ ~/.aidog/<f… |
| ops/remote-defaults-sync-chain.md#范式 (MUST，照抄先例 `gateway/defaults_sync.rs`) | ops | 范式 (MUST，照抄先例 `gateway/defaults_sync.rs`) | platform-presets,defaults,sync-chain,source-of-truth,bundled,gateway | auto | - | active | `defaults/*.json` 远端同步**MUST** 实现完整 7 件套，缺一致命。先例 `crates/aid… |
| ops/remote-defaults-sync-chain.md#验收断言（可复用） | ops | 验收断言（可复用） | platform-presets,defaults,sync-chain,source-of-truth,bundled,gateway | auto | - | active | ```bash # 7 件套齐全（双源 / last_updated / 24h / 三路触发 / schema gat… |
| ops/stack-attribution-profiling-methodology.md#栈归因用法 | ops | 栈归因用法 | profiling,stack-trace,attribution,instruments,xctrace,methodology,cpu | auto | - | active / →idle-cpu-baseline-xctrace,measure-window-exclusive-env,webkit-jit-warmup-trap | **定理**：静态检索定时器只能估出量级（因周期、触发条件、执行成本都是猜），无法判断是否真在稳态 CPU 占比中命中。… |
| ops/tauri-logging-guard-lifecycle.md#Tauri `tracing_appender::non_blocking` WorkerGuard 生命周期陷阱 | ops | Tauri `tracing_appender::non_blocking` WorkerGuard 生命周期陷阱 | Tauri,tracing,WorkerGuard,logging,lifecycle,guard-management | auto | - | active | ### 触发场景  在 Tauri 应用中使用 `tracing_appender::non_blocking` 创建后… |
| ops/tauri-logging-guard-lifecycle.md#Tauri tracing_appender::non_blocking WorkerGuard 生命周期陷阱 | ops | Tauri tracing_appender::non_blocking WorkerGuard 生命周期陷阱 | Tauri,tracing,WorkerGuard,logging,lifecycle,guard-management | auto | - | active | - |
| ops/test-data-isolation-constraint.md#性能测试数据隔离约束 | ops | 性能测试数据隔离约束 | testing,data,isolation,database,measurement,real-data,HOME,environment,loadgen,pollution,tmp | auto | - | active | - |
| ops/test-data-isolation-constraint.md#测试数据隔离硬约束 | ops | 测试数据隔离硬约束 | testing,data,isolation,database,measurement,real-data,HOME,environment,loadgen,pollution,tmp | auto | - | active | 性能量测或功能验证时需要用特定数据库（如缩小库、污染库等）。  ### 硬约束  - **禁移动/重命名用户的真实库文件… |
| ops/test-data-isolation-constraint.md#量测脚本 HOME 环境隔离硬约束 | ops | 量测脚本 HOME 环境隔离硬约束 | testing,data,isolation,database,measurement,real-data,HOME,environment,loadgen,pollution,tmp | auto | - | active | - |
| ops/test-data-isolation-constraint.md#量测脚本 HOME 环境隔离硬约束 | ops | 量测脚本 HOME 环境隔离硬约束 | testing,data,isolation,database,measurement,real-data,HOME,environment,loadgen,pollution,tmp | auto | - | active / →"$HOME" == "$HOME_REAL",tmp | ### 扩展约束：禁污染用户真实数据目录  前置约束禁止移动用户真实库文件，但仍需隔离 **整个数据目录**（不仅是单个… |
| optimization/api-payload-optimization.md#API 负载最小化 | optimization | API 负载最小化 | payload,optimization,request-size,bandwidth,compression | auto | - | active | - |
| optimization/api-payload-optimization.md#后端 DISTINCT 替代前端集合去重降低 IPC payload | optimization | 后端 DISTINCT 替代前端集合去重降低 IPC payload | payload,optimization,request-size,bandwidth,compression | auto | - | active | 后端改为返回去重后的单列（如 DISTINCT model），而非拉全字段摘要行数组到前端，再用集合去重。  **收益*… |
| optimization/idle-cpu-baseline-xctrace.md#空闲 CPU 基线数据 | optimization | 空闲 CPU 基线数据 | baseline,measurement,xctrace,process,webkit,profiling,cpu | auto | - | active / →idle-wakeup-sources-inventory,measure-window-exclusive-env,webkit-jit-warmup-trap | 基于 xctrace Time Profiler 实测（2026-07-31，30s 采样窗口）。四进程占比： - **… |
| optimization/idle-cpu-stack-sampling.md#反例（错误模式） | optimization | 反例（错误模式） | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / 仅 grep 定时器列表 / grep 列表 + `sample`… |
| optimization/idle-cpu-stack-sampling.md#案例 | optimization | 案例 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | grep 找到 5 个定时器，工作量推算应占 CPU 1-1.5%。但实测 3.0% 稳态，缺口 1.5% 无法追溯。用… |
| optimization/idle-cpu-stack-sampling.md#空闲 CPU 归因必须靠栈采样 | optimization | 空闲 CPU 归因必须靠栈采样 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | - |
| optimization/idle-cpu-stack-sampling.md#触发场景 | optimization | 触发场景 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | 性能分析中发现应用稳态 CPU 占用 3.0%，但静态代码检索只能找到 60s×1 + 300s×1 + 24h×3 共… |
| optimization/idle-cpu-stack-sampling.md#适用 | optimization | 适用 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | - 稳态 CPU 3% 以上但代码检索无法解释的场景 - 长时间后台进程 CPU 诊断 - 定时任务链效应分析（A 定时… |
| optimization/idle-cpu-stack-sampling.md#陷阱 & 正解 | optimization | 陷阱 & 正解 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | ❌ **陷阱**：仅用静态代码检索（grep）列举定时器  ```bash # 搜索所有定时器 grep -r "set… |
| optimization/manual-budget-empty-shortcircuit.md#manual_budget 零配额短路：进写连接前预检 | optimization | manual_budget 零配额短路：进写连接前预检 | budget,empty-check,shortcircuit,performance,cost-estimation | auto | - | active | - |
| optimization/manual-budget-empty-shortcircuit.md#关键点 | optimization | 关键点 | budget,empty-check,shortcircuit,performance,cost-estimation | auto | - | active | - **硬约束**：配额存在时行为不变，短路仅对「零配额」分支生效 - **非 mock 专属**：真实转发路径共用同一… |
| optimization/manual-budget-empty-shortcircuit.md#手动预算空集短路 | optimization | 手动预算空集短路 | budget,empty-check,shortcircuit,performance,cost-estimation | auto | - | active | - |
| optimization/manual-budget-empty-shortcircuit.md#方案 | optimization | 方案 | budget,empty-check,shortcircuit,performance,cost-estimation | auto | - | active | **分两阶段：**  1. **只读池预检**（`has_any_budget`，line:189-203）：用只读池（… |
| optimization/manual-budget-empty-shortcircuit.md#用途 | optimization | 用途 | budget,empty-check,shortcircuit,performance,cost-estimation | auto | - | active | 高频转发路径的每请求冷路径优化，减少单线程 DB 写锁争。适用于： - mock/真实平台混用的压测 - 用户未配额时的… |
| optimization/manual-budget-empty-shortcircuit.md#问题 | optimization | 问题 | budget,empty-check,shortcircuit,performance,cost-estimation | auto | - | active | `apply_manual_budgets`（`manual_budget.rs:211-246`）处理用户手动配额时，… |
| optimization/measure-footprint-pid-matching.md#measure.sh 同 label 跨 run 文件混淆 | optimization | measure.sh 同 label 跨 run 文件混淆 | measure,footprint,pid,glob,data-corruption,baseline | auto | - | active | - |
| optimization/measure-footprint-pid-matching.md#反例（错误模式） | optimization | 反例（错误模式） | measure,footprint,pid,glob,data-corruption,baseline | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / `glob footprint-${label}-*-*.txt`… |
| optimization/measure-footprint-pid-matching.md#案例 | optimization | 案例 | measure,footprint,pid,glob,data-corruption,baseline | auto | - | active | 实测显示某指标（graphics 等）跳到物理上限 2 倍，对比 size-curve-raw.txt 确认该档 TOT… |
| optimization/measure-footprint-pid-matching.md#触发场景 | optimization | 触发场景 | measure,footprint,pid,glob,data-corruption,baseline | auto | - | active | 性能量测脚本 `measure.sh` 按 label 重复运行（如多轮对比测试）时，旧 run 的 footprint… |
| optimization/measure-footprint-pid-matching.md#适用 | optimization | 适用 | measure,footprint,pid,glob,data-corruption,baseline | auto | - | active | - `measure.sh` 同 label 重复运行（对比 baseline 常见） - 任何大块临时数据依赖文件名去… |
| optimization/measure-footprint-pid-matching.md#陷阱 & 正解 | optimization | 陷阱 & 正解 | measure,footprint,pid,glob,data-corruption,baseline | auto | - | active | ❌ **陷阱**：glob 匹配所有同 label 的 footprint 文件，不区分 run  ```bash # … |
| optimization/measure-window-exclusive-env.md#环境互斥约束 | optimization | 环境互斥约束 | profiling,performance,measurement,environment,cargo,yarn,exclusive | auto | - | active / →idle-cpu-baseline-xctrace,webkit-jit-warmup-trap | Profiling（采样、trace 录制）与后台编译（cargo/yarn build）占用机器资源竞争。同步触发导致… |
| optimization/measure-window-multi-probe.md#判据 | optimization | 判据 | 量测,采样,cpu,前台,探针,regime,steady-state,foreground | auto | - | active | CPU/内存稳态采样，只在采样前打一次前台确证（如 `lsappinfo front`）不够——采样窗口内应用可能中途失… |
| optimization/measure-window-multi-probe.md#案例 | optimization | 案例 | 量测,采样,cpu,前台,探针,regime,steady-state,foreground | auto | - | active | `.scratch/perf-200mb/assets/results/cpu-s7-after-run3.txt`（8… |
| optimization/measure-window-multi-probe.md#正解 | optimization | 正解 | 量测,采样,cpu,前台,探针,regime,steady-state,foreground | auto | - | active | 稳态采样窗口内必须**多点探针**（如每 15s 一次），全程确证前台/目标态未漂移，而非仅窗口前一次性确证。另需注意 … |
| optimization/measure-window-multi-probe.md#适用 | optimization | 适用 | 量测,采样,cpu,前台,探针,regime,steady-state,foreground | auto | - | active | CPU/内存稳态性能采样，尤其涉及应用前台/背景态切换、GUI 应用量测场景。 |
| optimization/measure-window-multi-probe.md#量测 regime 自证必须窗口内多点探针 | optimization | 量测 regime 自证必须窗口内多点探针 | 量测,采样,cpu,前台,探针,regime,steady-state,foreground | auto | - | active | - |
| optimization/measure-window-multi-probe.md#陷阱 | optimization | 陷阱 | 量测,采样,cpu,前台,探针,regime,steady-state,foreground | auto | - | active | 实测：run3 采样前确证前台，但 60s 窗口末端已漂回终端，读数被稀释成 8.2%（前台+背景混合值）。同实例钉死前… |
| optimization/memory-measure-background.md#内存量测走纯背景态口径 | optimization | 内存量测走纯背景态口径 | memory,measure,background,activate,settle,foreground | auto | - | active | - |
| optimization/memory-measure-background.md#反例（错误模式） | optimization | 反例（错误模式） | memory,measure,background,activate,settle,foreground | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / 内存+CPU 都用 activate + settle / 内存用… |
| optimization/memory-measure-background.md#案例 | optimization | 案例 | memory,measure,background,activate,settle,foreground | auto | - | active | run1/run2 内存量测全 4 档失效，对比日志发现 activate 后应用被 Finder 抢走。改为背景态启动… |
| optimization/memory-measure-background.md#触发场景 | optimization | 触发场景 | memory,measure,background,activate,settle,foreground | auto | - | active | 内存占用量测时，采用 CPU 量测的 `activate + settle` 两段试图通过前台激活 + 等待稳定来排除用… |
| optimization/memory-measure-background.md#适用 | optimization | 适用 | memory,measure,background,activate,settle,foreground | auto | - | active | - Tauri / Electron 应用内存占用基准量测 - 长时间后台内存监控（避免前台抢占） - 交叉对比前台/后… |
| optimization/memory-measure-background.md#陷阱 & 正解 | optimization | 陷阱 & 正解 | memory,measure,background,activate,settle,foreground | auto | - | active | ❌ **陷阱**：内存量测复用 CPU 量测的 activate + settle 口径  ```bash # CPU … |
| optimization/sqlite-cache-measurement-traps.md#SQLite 页缓存量测三大陷阱 | optimization | SQLite 页缓存量测三大陷阱 | sqlite,measurement,profiling,memory,phys_footprint,noise | auto | - | active | 实测 SQLite 默认 cache_size 与各档位定值方案时踩过的坑。  ### 陷阱一：内存计量工具选错  **… |
| optimization/sqlite-cache-measurement-traps.md#SQLite 页缓存量测陷阱 | optimization | SQLite 页缓存量测陷阱 | sqlite,measurement,profiling,memory,phys_footprint,noise | auto | - | active | - |
| optimization/webkit-jit-warmup-trap.md#WebContent JSC JIT 热身陷阱 | optimization | WebContent JSC JIT 热身陷阱 | webkit,jsc,jit,warmup,profiling,sampling,trap,cpu | auto | - | active / →idle-cpu-baseline-xctrace | WebContent 进程中 JSC JIT 热身阶段（启动后数分钟）vs 稳定态（运行 45+ 分钟）的 CPU 占比… |
| optimization/webkit-xpc-helper-process-bounds.md#反例（错误模式） | optimization | 反例（错误模式） | webkit,xpc,helper,process-tree,ppid,measurement-isolation | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / 用 ppid 反查归属 / 编制硬闸：期望 WebContent×… |
| optimization/webkit-xpc-helper-process-bounds.md#案例 | optimization | 案例 | webkit,xpc,helper,process-tree,ppid,measurement-isolation | auto | - | active | 多轮量测发现某档进程数突增（期望 4，实际 6-8），发现混入了飞书/Safari 的 WebKit helper。改用… |
| optimization/webkit-xpc-helper-process-bounds.md#触发场景 | optimization | 触发场景 | webkit,xpc,helper,process-tree,ppid,measurement-isolation | auto | - | active | WebKit 内嵌浏览器在 Tauri 应用中运行时，`ppid`（父进程 ID）恒为 1（launchd），`ps -… |
| optimization/webkit-xpc-helper-process-bounds.md#进程编制核验硬闸替代动态反查 | optimization | 进程编制核验硬闸替代动态反查 | webkit,xpc,helper,process-tree,ppid,measurement-isolation | auto | - | active | - |
| optimization/webkit-xpc-helper-process-bounds.md#适用 | optimization | 适用 | webkit,xpc,helper,process-tree,ppid,measurement-isolation | auto | - | active | - Tauri / Electron 等嵌入 WebKit 的桌面应用性能量测 - 多窗口场景排查进程组织 - 交叉应用… |
| optimization/webkit-xpc-helper-process-bounds.md#陷阱 & 正解 | optimization | 陷阱 & 正解 | webkit,xpc,helper,process-tree,ppid,measurement-isolation | auto | - | active | ❌ **陷阱**：用 ppid / ps args / procinfo 反查进程归属  ```bash # ppid … |
| proxy/async-log-queue-backpressure.md#关联 | proxy | 关联 | async,logging,queue,backpressure,throughput,buffer | auto | - | active / →connect-tunnel-contract,db-table-conventions | [[connect-tunnel-contract]] （proxy 统计不污染） · [[db-table-conve… |
| proxy/async-log-queue-backpressure.md#反例 / 常见错误 | proxy | 反例 / 常见错误 | async,logging,queue,backpressure,throughput,buffer | auto | - | active | / 错误                          / 为什么错                        … |
| proxy/async-log-queue-backpressure.md#异步日志队列反压 | proxy | 异步日志队列反压 | async,logging,queue,backpressure,throughput,buffer | auto | - | active | - |
| proxy/async-log-queue-backpressure.md#案例 | proxy | 案例 | async,logging,queue,backpressure,throughput,buffer | auto | - | active | - log-async-write task (commit 529e571b) — proxy_log 改为单 wri… |
| proxy/async-log-queue-backpressure.md#正解：方案 B（单 writer + 有界 queue + 分级背压 + 串行快照） | proxy | 正解：方案 B（单 writer + 有界 queue + 分级背压 + 串行快照） | async,logging,queue,backpressure,throughput,buffer | auto | - | active | ### 架构骨架 ``` 热路径 (request handler)       后台 writer task ────… |
| proxy/async-log-queue-backpressure.md#落库路径升级 checklist | proxy | 落库路径升级 checklist | async,logging,queue,backpressure,throughput,buffer | auto | - | active | ```rust // 新增高频异步操作时参考此模式： // 1. 定义枚举消息类型 pub(crate) enum Yo… |
| proxy/async-log-queue-backpressure.md#触发场景 | proxy | 触发场景 | async,logging,queue,backpressure,throughput,buffer | auto | - | active | 高频热路径中需要异步写入数据库（如 proxy_log upsert），不能阻塞请求处理；需要保证最终结果不丢且落库顺序… |
| proxy/async-log-queue-backpressure.md#适用 | proxy | 适用 | async,logging,queue,backpressure,throughput,buffer | auto | - | active | - proxy_log 异步写入（已实现 s1） - 其他高频日志 / 统计 / 聚合表的异步更新（future 可参考… |
| proxy/async-log-queue-backpressure.md#陷阱：同步写会阻塞热路径 + 异步不保证持久性 | proxy | 陷阱：同步写会阻塞热路径 + 异步不保证持久性 | async,logging,queue,backpressure,throughput,buffer | auto | - | active | > proxy_log 原先热路径内同步调 `upsert_log(db).await` → 所有请求必须等 DB 写入… |
| proxy/async-log-queue-backpressure.md#验证 | proxy | 验证 | async,logging,queue,backpressure,throughput,buffer | auto | - | active | ```bash # 背压分级（中间态 try_send vs 终态 send） cd src-tauri && grep… |
| proxy/diagnostic-header-helper.md#Helper 复用契约 (MUST) | proxy | Helper 复用契约 (MUST) | diagnostic-headers,request-tracing,debugging,headers | auto | - | active | > 违反代价: 各响应构造点重复实现 `cfg!(debug_assertions)` gate, 新加诊断 heade… |
| proxy/diagnostic-header-helper.md#blind_relay 物理豁免 (MUST NOT) | proxy | blind_relay 物理豁免 (MUST NOT) | diagnostic-headers,request-tracing,debugging,headers | auto | - | active | > 违反代价: blind_relay 是 CONNECT 隧道建好后 TCP 字节透传, AirDog 看见的是加密 … |
| proxy/diagnostic-header-helper.md#header 名规范 (MUST) | proxy | header 名规范 (MUST) | diagnostic-headers,request-tracing,debugging,headers | auto | - | active | - **header 名 MUST 小写** (`x-aidog-trace` 等), 用 `HeaderName::f… |
| proxy/diagnostic-header-helper.md#id 取值链 (MUST) | proxy | id 取值链 (MUST) | diagnostic-headers,request-tracing,debugging,headers | auto | - | active | > 违反代价: 各处自造 id 失去与 proxy_log / span 的关联, 诊断时无法客户端报错 → AirDo… |
| proxy/diagnostic-header-helper.md#release build 行为 (MUST) | proxy | release build 行为 (MUST) | diagnostic-headers,request-tracing,debugging,headers | auto | - | active | - **release build MUST 不注入** —— helper 内 `if cfg!(debug_asse… |
| proxy/diagnostic-header-helper.md#诊断头部辅助函数 | proxy | 诊断头部辅助函数 | diagnostic-headers,request-tracing,debugging,headers | auto | - | active | # Proxy 诊断响应 Header (debug build)  何时被读: 改 `src-tauri/crates… |
| proxy/diagnostic-header-helper.md#跨协议注入选址参考 | proxy | 跨协议注入选址参考 | diagnostic-headers,request-tracing,debugging,headers | auto | - | active | `07-05-proxy-trace-id-header` 实施时枚举的 47 调用点分布: - `handler.rs… |
| proxy/diagnostic-header-helper.md#验收基准 (可复用) | proxy | 验收基准 (可复用) | diagnostic-headers,request-tracing,debugging,headers | auto | - | active | - [ ] debug build: 所有 AirDog **直构**响应含诊断 header (grep `injec… |
| proxy/diagnostic-header-helper.md#验证命令 | proxy | 验证命令 | diagnostic-headers,request-tracing,debugging,headers | auto | - | active | ```bash # helper 调用计数 (debug 注入点) grep -rn "inject_trace_hea… |
| proxy/sse-chunk-stateless-defect.md#SSE 流式处理无状态缺陷 | proxy | SSE 流式处理无状态缺陷 | SSE,streaming,stateless,chunk,frame-boundary,protocol | auto | - | active | - |
| proxy/sse-chunk-stateless-defect.md#关联 | proxy | 关联 | SSE,streaming,stateless,chunk,frame-boundary,protocol | auto | - | active / →async-log-queue-backpressure,hot-path-buffers,stream-buf-unified-cap | [[stream-buf-unified-cap]] （上界单一真值源） · [[hot-path-buffers]] … |
| proxy/sse-chunk-stateless-defect.md#正解：尾行缓冲 + 无状态解析分离 | proxy | 正解：尾行缓冲 + 无状态解析分离 | SSE,streaming,stateless,chunk,frame-boundary,protocol | auto | - | active / →stream-buf-unified-cap | ### MUST 架构  - **尾行缓冲层**：在无状态解析函数外层加一个有状态的行重组器（如 `SseLineRea… |
| proxy/sse-chunk-stateless-defect.md#缺陷：逐 chunk 无状态解析导致完整行静默丢失 | proxy | 缺陷：逐 chunk 无状态解析导致完整行静默丢失 | SSE,streaming,stateless,chunk,frame-boundary,protocol | auto | - | active | > 协议转换分支的 chunk 循环逐 chunk 独立调用 `parse_upstream_sse(&text, ..… |
| proxy/sse-chunk-stateless-defect.md#触发场景 | proxy | 触发场景 | SSE,streaming,stateless,chunk,frame-boundary,protocol | auto | - | active | 流式转发（协议转换分支）中逐 chunk 调用无状态的 SSE 解析函数（如 `adapter::parse_upstr… |
| proxy/sse-chunk-stateless-defect.md#适用场景 | proxy | 适用场景 | SSE,streaming,stateless,chunk,frame-boundary,protocol | auto | - | active | - SSE/Server-Sent-Events 协议转换 - 逐 chunk 处理的流式数据（WebSocket up… |
| proxy/sse-chunk-stateless-defect.md#验收基准 | proxy | 验收基准 | SSE,streaming,stateless,chunk,frame-boundary,protocol | auto | - | active | - [ ] 缓冲层与无状态解析完全分离 - [ ] 完整行立即下发，有用例证明（喂「完整行 + 残行」，断言完整行立刻出… |
| reuse/grep-before-write.md#修改前搜索验收清单 | reuse | 修改前搜索验收清单 | grep,search,verification,refactor,change-audit | auto | - | active | - |
| reuse/grep-before-write.md#修改前搜索验收清单 | reuse | 修改前搜索验收清单 | grep,search,verification,refactor,change-audit | auto | - | active | 对任何源文件的修改（新增列 / 删除列 / 重命名列）提交前，必须 grep 全库检查「所有读取、初始化、比较该列的代码… |
| reuse/grep-before-write.md#关联 | reuse | 关联 | grep,search,verification,refactor,change-audit | auto | - | active / →component-extraction-grep-callsites | [[component-extraction-grep-callsites]] 组件提取时的同步策略 |
| reuse/grep-before-write.md#适用 | reuse | 适用 | grep,search,verification,refactor,change-audit | auto | - | active | - 修改任何共享结构（SQL 表 / JSON schema / Rust struct 用到多地） - 特别是跨层结构… |
| reuse/grep-before-write.md#验收 | reuse | 验收 | grep,search,verification,refactor,change-audit | auto | - | active | - [ ] 修改前准备完整的「代码路径清单」文档 - [ ] 清单覆盖：初始化 / 读 / 写 / 业务逻辑四类 - [… |
| shadcn/dialog-open-explicit-null.md#Dialog.open 需显式 null 判断 | shadcn | Dialog.open 需显式 null 判断 | Dialog,open,null,Promise | auto | - | active | Dialog.open 属性需要 bool 类型，当使用 Promise resolve 型 state 时需显式 nu… |
| shadcn/dialog-open-explicit-null.md#dialog-open-explicit-null | shadcn | dialog-open-explicit-null | Dialog,open,null,Promise | auto | - | active | - |
| shadcn/dialog-open-explicit-null.md#关联 | shadcn | 关联 | Dialog,open,null,Promise | auto | - | active / →radix-dialog-requires-title | [[radix-dialog-requires-title]] |
| shadcn/dialog-open-explicit-null.md#模式模板 | shadcn | 模式模板 | Dialog,open,null,Promise | auto | - | active | ```tsx const [modalState, setModalState] = useState<{resolve… |
| shadcn/dialog-open-explicit-null.md#适用 | shadcn | 适用 | Dialog,open,null,Promise | auto | - | active | - 任何 Promise resolve 型 state 控制弹窗开关的场景（如 async confirm/自定义 M… |
| shadcn/dialog-open-explicit-null.md#陷阱-正解 | shadcn | 陷阱-正解 | Dialog,open,null,Promise | auto | - | active | ❌ **陷阱**：直接用 `open={modalState}` 会将 null/对象转为 bool，无法正确反映语义。… |
| shadcn/dnd-kit-sortable-preserve-logic.md#dnd-kit SortableList 迁移保留拖拽逻辑 | shadcn | dnd-kit SortableList 迁移保留拖拽逻辑 | dnd-kit,SortableList,拖拽 | auto | - | active | dnd-kit SortableList 组件迁移时，只需替换内部 button/视觉组件，拖拽逻辑保持不变。 |
| shadcn/dnd-kit-sortable-preserve-logic.md#dnd-kit-sortable-preserve-logic | shadcn | dnd-kit-sortable-preserve-logic | dnd-kit,SortableList,拖拽 | auto | - | active | - |
| shadcn/dnd-kit-sortable-preserve-logic.md#关联 | shadcn | 关联 | dnd-kit,SortableList,拖拽 | auto | - | active / →radix-select-none-sentinel | [[radix-select-none-sentinel]] |
| shadcn/dnd-kit-sortable-preserve-logic.md#案例 | shadcn | 案例 | dnd-kit,SortableList,拖拽 | auto | - | active | - shadcn-pages task：Groups/GroupListItem SortableList 迁移，保留拖… |
| shadcn/dnd-kit-sortable-preserve-logic.md#模式模板 | shadcn | 模式模板 | dnd-kit,SortableList,拖拽 | auto | - | active | ```tsx // 保留：拖拽逻辑 const { attributes, listeners, setNodeRef,… |
| shadcn/dnd-kit-sortable-preserve-logic.md#适用 | shadcn | 适用 | dnd-kit,SortableList,拖拽 | auto | - | active | - dnd-kit SortableList 迁移至 shadcn - 保留拖拽逻辑仅换视觉的场景 |
| shadcn/dnd-kit-sortable-preserve-logic.md#陷阱-正解 | shadcn | 陷阱-正解 | dnd-kit,SortableList,拖拽 | auto | - | active | ❌ **陷阱**：重写整个拖拽逻辑，破坏已有行为。 ✅ **正解**：保留 dnd-kit 的 useSortable/… |
| shadcn/planning-scope-pregrep.md#planning 范围预筛纪律（grep） | shadcn | planning 范围预筛纪律（grep） | planning,预筛,grep,范围 | auto | - | active | planning 阶段需要预先用 grep 检查目标范围是否真的需要该改动，避免对不含相关代码的文件跑不必要的改造。 |
| shadcn/planning-scope-pregrep.md#planning-scope-pregrep | shadcn | planning-scope-pregrep | planning,预筛,grep,范围 | auto | - | active | - |
| shadcn/planning-scope-pregrep.md#例子 | shadcn | 例子 | planning,预筛,grep,范围 | auto | - | active | - **shadcn 迁移**：检查是否有 `<button` / `<input` / `<select` 等表单控件… |
| shadcn/planning-scope-pregrep.md#关联 | shadcn | 关联 | planning,预筛,grep,范围 | auto | - | active / →platform-creation-entry-consolidation,radix-select-none-sentinel | [[radix-select-none-sentinel]]、[[platform-creation-entry-con… |
| shadcn/planning-scope-pregrep.md#适用 | shadcn | 适用 | planning,预筛,grep,范围 | auto | - | active | - planning 阶段大范围变更（框架升级、组件库迁移、业务重构） - 确保 task 范围精准，避免 false … |
| shadcn/planning-scope-pregrep.md#陷阱-正解 | shadcn | 陷阱-正解 | planning,预筛,grep,范围 | auto | - | active | ❌ **陷阱**：planning 时未预筛，按通用模板对所有目标域跑变更逻辑，对不含相关代码的区域产生误判。 ✅ **… |
| shadcn/planning-scope-pregrep.md#预筛命令模板 | shadcn | 预筛命令模板 | planning,预筛,grep,范围 | auto | - | active | ```bash # 检查是否存在相关代码 grep -r "相关代码模式" 目标路径/ --include="*.ts*… |
| shadcn/radix-dialog-requires-title.md#MUST 硬约束 | shadcn | MUST 硬约束 | Radix,Dialog,DialogTitle,a11y | auto | - | active | Radix Dialog **必须包含 DialogTitle**，否则会触发 a11y 警告。 |
| shadcn/radix-dialog-requires-title.md#Radix Dialog 必须含 DialogTitle | shadcn | Radix Dialog 必须含 DialogTitle | Radix,Dialog,DialogTitle,a11y | auto | - | active | Radix Dialog 组件必须包含 DialogTitle 以满足无障碍（a11y）要求。自定义 header 时使… |
| shadcn/radix-dialog-requires-title.md#radix-dialog-requires-title | shadcn | radix-dialog-requires-title | Radix,Dialog,DialogTitle,a11y | auto | - | active | - |
| shadcn/radix-dialog-requires-title.md#关联 | shadcn | 关联 | Radix,Dialog,DialogTitle,a11y | auto | - | active / →dialog-open-explicit-null | [[dialog-open-explicit-null]] |
| shadcn/radix-dialog-requires-title.md#实现模式 | shadcn | 实现模式 | Radix,Dialog,DialogTitle,a11y | auto | - | active | ❌ **陷阱**：自定义 header 时完全省略 DialogTitle，破坏 a11y。 ✅ **正解**：用 `s… |
| shadcn/radix-dialog-requires-title.md#案例 | shadcn | 案例 | Radix,Dialog,DialogTitle,a11y | auto | - | active | - `src/components/settings/editors/StatusLineSection/Segment… |
| shadcn/radix-dialog-requires-title.md#模式模板 | shadcn | 模式模板 | Radix,Dialog,DialogTitle,a11y | auto | - | active | ```tsx import { Dialog, DialogContent, DialogTitle } from "@… |
| shadcn/radix-dialog-requires-title.md#适用 | shadcn | 适用 | Radix,Dialog,DialogTitle,a11y | auto | - | active | - 所有 Radix Dialog 用法（@/components/ui/dialog） - 需要完全自定义 heade… |
| shadcn/radix-select-none-sentinel.md#radix Select 空值哨兵模式 | shadcn | radix Select 空值哨兵模式 | radix,Select,空值,哨兵 | auto | - | active | 使用 radix Select 组件时，value 属性需要处理空值/undefined 状态，使用哨兵值避免内部验证错… |
| shadcn/radix-select-none-sentinel.md#radix-select-none-sentinel | shadcn | radix-select-none-sentinel | radix,Select,空值,哨兵 | auto | - | active | - |
| shadcn/radix-select-none-sentinel.md#关联 | shadcn | 关联 | radix,Select,空值,哨兵 | auto | - | active / →radix-select-number-mapping | [[radix-select-number-mapping]] |
| shadcn/radix-select-none-sentinel.md#案例 | shadcn | 案例 | radix,Select,空值,哨兵 | auto | - | active | - `src/pages/platforms/PlatformPicker.tsx:105-109` 可选平台选择器 |
| shadcn/radix-select-none-sentinel.md#模式模板 | shadcn | 模式模板 | radix,Select,空值,哨兵 | auto | - | active | ```tsx // 定义哨兵常量 const NONE = "__none__";  // 组件使用 <Select  … |
| shadcn/radix-select-none-sentinel.md#适用 | shadcn | 适用 | radix,Select,空值,哨兵 | auto | - | active | - radix Select 组件（@/components/ui/select） - 需要空值占位符的下拉选择场景 |
| shadcn/radix-select-none-sentinel.md#陷阱-正解 | shadcn | 陷阱-正解 | radix,Select,空值,哨兵 | auto | - | active | ❌ **陷阱**：直接使用 `value=""` 会触发 radix Select 内部验证错误（SelectItem … |
| shadcn/radix-select-number-mapping.md#radix Select number 双向映射 | shadcn | radix Select number 双向映射 | radix,Select,number,String | auto | - | active | radix Select 的 value 属性只接受 string 类型，需要处理 number 类型数据时使用双向映射… |
| shadcn/radix-select-number-mapping.md#radix-select-number-mapping | shadcn | radix-select-number-mapping | radix,Select,number,String | auto | - | active | - |
| shadcn/radix-select-number-mapping.md#关联 | shadcn | 关联 | radix,Select,number,String | auto | - | active / →radix-select-none-sentinel | [[radix-select-none-sentinel]] |
| shadcn/radix-select-number-mapping.md#案例 | shadcn | 案例 | radix,Select,number,String | auto | - | active | - `src/pages/Logs/primitives.tsx:374` Pagination pageSize: `… |
| shadcn/radix-select-number-mapping.md#模式模板 | shadcn | 模式模板 | radix,Select,number,String | auto | - | active | ```tsx <Select   value={String(numberValue)}  // 存储/显示：numbe… |
| shadcn/radix-select-number-mapping.md#适用 | shadcn | 适用 | radix,Select,number,String | auto | - | active | - radix Select value 仅收 string（类型约束） - 需要处理 number 选项的分页器/数值… |
| shadcn/radix-select-number-mapping.md#陷阱-正解 | shadcn | 陷阱-正解 | radix,Select,number,String | auto | - | active | ❌ **陷阱**：直接传 number 会触发类型错误或运行时异常。 ✅ **正解**：存储/显示时 String() … |
| shadcn/shadcn-button-svg-16px.md#Button cva 基类 | shadcn | Button cva 基类 | shadcn,Button,cva,svg | auto | - | active | ```tsx variants: {   // ...   base: "inline-flex items-cente… |
| shadcn/shadcn-button-svg-16px.md#MUST 硬约束 | shadcn | MUST 硬约束 | shadcn,Button,cva,svg | auto | - | active | shadcn Button 内的 svg 图标会被强制压至 16px（`size-4` = 1rem = 16px），自… |
| shadcn/shadcn-button-svg-16px.md#shadcn Button cva 基类压 svg 16px | shadcn | shadcn Button cva 基类压 svg 16px | shadcn,Button,cva,svg | auto | - | active | shadcn Button 组件 cva 基类含 `[&_svg]:size-4` 规则，统一压内部 svg 至 16p… |
| shadcn/shadcn-button-svg-16px.md#shadcn-button-svg-16px | shadcn | shadcn-button-svg-16px | shadcn,Button,cva,svg | auto | - | active | - |
| shadcn/shadcn-button-svg-16px.md#关联 | shadcn | 关联 | shadcn,Button,cva,svg | auto | - | active / →dialog-open-explicit-null | [[dialog-open-explicit-null]] |
| shadcn/shadcn-button-svg-16px.md#适用 | shadcn | 适用 | shadcn,Button,cva,svg | auto | - | active | - 所有 shadcn Button 用法（@/components/ui/button） - nav icon 等小图… |
| skein/coding-plan-utilization-calib-fix-27.md#task 查重: 同模块非重复, 先看 PRD 边界互引 | skein | task 查重: 同模块非重复, 先看 PRD 边界互引 | skein,dedup,task-boundary,prd | auto | - | active | dedup/查重判定重叠维度前, MUST 先看两 task 的 PRD 边界条款是否已显式互相引用切割 (如双向标注对… |
| skein/decision-documentation.md#实测推翻设计假设时的处理范式（留痕+不硬凑） | skein | 实测推翻设计假设时的处理范式（留痕+不硬凑） | planning,execution,hypothesis-testing,decision-logging,design-vs-reality | auto | - | active | 当 task 执行过程中发现「planning 写的验收文本与 exec 实测结果矛盾」时，按以下范式处理：  **模式… |
| skein/parallel-subtask-prop-contract.md#3.5 并行契约（S2/S3 同时跑，锁死边界） | skein | 3.5 并行契约（S2/S3 同时跑，锁死边界） | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | ### 文件划分（禁止跨界改动） - **S2 负责**：`PlatformEditForm.tsx`（给 Models… |
| skein/parallel-subtask-prop-contract.md#关联 | skein | 关联 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active / →dirty-float-hour-normalization,form-level-tz-state-sharing | [[dirty-float-hour-normalization]] · [[form-level-tz-state-s… |
| skein/parallel-subtask-prop-contract.md#反例 / 常见错误 | skein | 反例 / 常见错误 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | / 错误                            / 为什么错                      … |
| skein/parallel-subtask-prop-contract.md#案例 | skein | 案例 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | - time-models-timezone task (design.md §3.5) — S2/S3 并行，prop… |
| skein/parallel-subtask-prop-contract.md#正解：planning 阶段锁定 prop 契约（硬约束，关键） | skein | 正解：planning 阶段锁定 prop 契约（硬约束，关键） | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | ### MUST 在 design.md 明确标记文件分工  ```markdown |
| skein/parallel-subtask-prop-contract.md#落地 checklist | skein | 落地 checklist | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | ```bash # 集成前逐项验证 # 1. 文件分工 git log --oneline time-models-ti… |
| skein/parallel-subtask-prop-contract.md#触发场景 | skein | 触发场景 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | 两个或多个 subtask 需要同时改造同一组件树中的多个文件（例如 S2 改 `PlatformEditForm.ts… |
| skein/parallel-subtask-prop-contract.md#适用 | skein | 适用 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | - 并行多个 subtask 改造同一组件树的不同部分 - 跨团队开发中需要接口预协商的场景（prop 签名即"API … |
| skein/parallel-subtask-prop-contract.md#陷阱：未锁定 prop 契约导致运行时 BAD_REQUEST / TS 类型错 | skein | 陷阱：未锁定 prop 契约导致运行时 BAD_REQUEST / TS 类型错 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | > S2 和 S3 分别并行改造组件树的不同部分，但 S2 声明的 prop 接收端签名（如 `ModelsMatrix… |
| skein/parallel-subtask-prop-contract.md#验证场景 | skein | 验证场景 | 并行执行,subtask,文件划分,prop 契约,TS 类型,运行时零冲突,planning | auto | - | active | 1. S2 提交：`usePlatformForm.ts` 新增 `windowsTz` state，design 文档… |
| skein/subagent-hook-scope.md#subagent hook 禁写主仓报告文件 | skein | subagent hook 禁写主仓报告文件 | subagent,hook,scope,worktree,output-format,repo-pollution | auto | - | active | - |
| skein/subagent-hook-scope.md#反例（错误模式） | skein | 反例（错误模式） | subagent,hook,scope,worktree,output-format,repo-pollution | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / 派时未限制产物路径 / 明确 `工作目录: research/`，… |
| skein/subagent-hook-scope.md#案例 | skein | 案例 | subagent,hook,scope,worktree,output-format,repo-pollution | auto | - | active | 派 researcher 调研某模块时，它直接在主仓根目录产生 `findings.md` 和 `recommendat… |
| skein/subagent-hook-scope.md#触发场景 | skein | 触发场景 | subagent,hook,scope,worktree,output-format,repo-pollution | auto | - | active | 派 researcher / workflow subagent 时，如果在 hook（如 subagent 中断返回）… |
| skein/subagent-hook-scope.md#适用 | skein | 适用 | subagent,hook,scope,worktree,output-format,repo-pollution | auto | - | active | - 派遣 researcher / workflow / skill 等 data-producing subagent… |
| skein/subagent-hook-scope.md#陷阱 & 正解 | skein | 陷阱 & 正解 | subagent,hook,scope,worktree,output-format,repo-pollution | auto | - | active | ❌ **陷阱**：派 researcher 时不限制产物路径，允许 hook 中写报告文件  ```python # 派… |
| skein/subagent-sendmessage.md#agent 零回传真因 = 未调 SendMessage | skein | agent 零回传真因 = 未调 SendMessage | subagent,sendmessage,return-value,coordinator,message-passing | auto | - | active | - |
| skein/subagent-sendmessage.md#反例（错误模式） | skein | 反例（错误模式） | subagent,sendmessage,return-value,coordinator,message-passing | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / 仅 print/echo 文本输出 / print 文本 + 调 … |
| skein/subagent-sendmessage.md#案例 | skein | 案例 | subagent,sendmessage,return-value,coordinator,message-passing | auto | - | active | 既有约定记录 3 个实例系统性不回传。根因是这些 subagent 仅写 stdout，未调 SendMessage。修… |
| skein/subagent-sendmessage.md#触发场景 | skein | 触发场景 | subagent,sendmessage,return-value,coordinator,message-passing | auto | - | active | 派 subagent（如 researcher / checker 等）时，应答端只写 stdout 文本输出，未调用 … |
| skein/subagent-sendmessage.md#适用 | skein | 适用 | subagent,sendmessage,return-value,coordinator,message-passing | auto | - | active | - 所有派 subagent 的场景（researcher / checker / workflow / skill） … |
| skein/subagent-sendmessage.md#陷阱 & 正解 | skein | 陷阱 & 正解 | subagent,sendmessage,return-value,coordinator,message-passing | auto | - | active | ❌ **陷阱**：仅写文本输出，不调 SendMessage 工具  ```python # subagent 应答端 … |
| style/logging-trace-id-contract.md#logging-trace-id-contract | style | logging-trace-id-contract | log,trace,traceid,ansi | auto | - | active | - |
| style/logging-trace-id-contract.md#traceid 取值链 (MUST) | style | traceid 取值链 (MUST) | log,trace,traceid,ansi | auto | - | active | 每行 MUST 含 traceid，取值三级兜底 MUST：`trace_id_from_span_scope` → `… |
| style/logging-trace-id-contract.md#代码位置 | style | 代码位置 | log,trace,traceid,ansi | auto | - | active | - **应用层格式器**：`src-tauri/crates/aidog_core/src/logging.rs:16-… |
| style/logging-trace-id-contract.md#何时被读 | style | 何时被读 | log,trace,traceid,ansi | auto | - | active | - 改 `src-tauri/crates/aidog_core/src/logging.rs` 的格式器、新加 `to… |
| style/logging-trace-id-contract.md#关联 | style | 关联 | log,trace,traceid,ansi | auto | - | active / →auto-disable-401-403-402,remote-defaults-sync-chain | [[auto-disable-401-403-402]]、[[remote-defaults-sync-chain]]（… |
| style/logging-trace-id-contract.md#异步分支 id 传播 (MUST) | style | 异步分支 id 传播 (MUST) | log,trace,traceid,ansi | auto | - | active | - **新加 `tokio::spawn` MUST 走 `spawn_traced(name, fut)` helpe… |
| style/logging-trace-id-contract.md#日志字段顺序 (MUST) | style | 日志字段顺序 (MUST) | log,trace,traceid,ansi | auto | - | active | - **MUST 5 段严格顺序**: `time` → `level` → `file:line func` → `m… |
| style/logging-trace-id-contract.md#日志格式与 traceid 取值链契约 | style | 日志格式与 traceid 取值链契约 | log,trace,traceid,ansi | auto | - | active | 日志格式与 traceid 取值链必须保持对称，两者配合构成诊断链。应用层（logging.rs）负责格式化和 thre… |
| style/logging-trace-id-contract.md#验收基准 | style | 验收基准 | log,trace,traceid,ansi | auto | - | active | - [ ] header `x-aidog-trace` id grep 日志命中 ≥1 行 - [ ] console… |
| test/cross-crate-test-path.md#关联 | test | 关联 | cross-crate,testing,integration,workspace,test-utils | auto | - | active / →invoke-name-source-of-truth | [[invoke-name-source-of-truth]] |
| test/cross-crate-test-path.md#案例 | test | 案例 | cross-crate,testing,integration,workspace,test-utils | auto | - | active | - |
| test/cross-crate-test-path.md#正解 | test | 正解 | cross-crate,testing,integration,workspace,test-utils | auto | - | active | 将所有 `aidog_core::` 前缀改为 `crate::`（当前 crate 的自引用）： ```rust //… |
| test/cross-crate-test-path.md#触发场景 | test | 触发场景 | cross-crate,testing,integration,workspace,test-utils | auto | - | active | 测试代码从外部 crate 迁移进 aidog_core 内部时。 |
| test/cross-crate-test-path.md#跨 Crate 测试路径 | test | 跨 Crate 测试路径 | cross-crate,testing,integration,workspace,test-utils | auto | - | active | - |
| test/cross-crate-test-path.md#适用 | test | 适用 | cross-crate,testing,integration,workspace,test-utils | auto | - | active | - 跨 crate 迁移测试文件 - 模块合并时 - 测试代码路径清理 |
| test/cross-crate-test-path.md#陷阱 | test | 陷阱 | cross-crate,testing,integration,workspace,test-utils | auto | - | active | 保持原外部 crate 的全限定路径 `aidog_core::xxx::yyy`，但新位置是 aidog_core 内… |
| test/shadcn-test-behavior-assert.md#MUST 硬约束 | test | MUST 硬约束 | shadcn,testing,behavior,assertion,radix | auto | - | active | 测试改测行为而非 className；shadcn 迁移后 snapshot 应改为行为断言。 |
| test/shadcn-test-behavior-assert.md#Shadcn 组件行为断言测试 | test | Shadcn 组件行为断言测试 | shadcn,testing,behavior,assertion,radix | auto | - | active | - |
| test/shadcn-test-behavior-assert.md#关联 | test | 关联 | shadcn,testing,behavior,assertion,radix | auto | - | active / →radix-select-none-sentinel | [[radix-select-none-sentinel]] |
| test/shadcn-test-behavior-assert.md#案例 | test | 案例 | shadcn,testing,behavior,assertion,radix | auto | - | active | - shadcn-pages task：PlatformCard.test.tsx snapshot → 行为断言（删除… |
| test/shadcn-test-behavior-assert.md#触发场景 | test | 触发场景 | shadcn,testing,behavior,assertion,radix | auto | - | active | shadcn 迁移导致组件 className/结构变化，现有 snapshot 测试会因视觉差异失败。 |
| test/shadcn-test-behavior-assert.md#迁移模式 | test | 迁移模式 | shadcn,testing,behavior,assertion,radix | auto | - | active | ```tsx // ❌ 旧：测试 className（脆弱） expect(screen.getByTestId("ca… |
| test/shadcn-test-behavior-assert.md#适用 | test | 适用 | shadcn,testing,behavior,assertion,radix | auto | - | active | - PlatformCard/BalanceBar 等组件测试 - shadcn 迁移导致 className/结构变化… |
| testing/deterministic-pseudorandom-loadgen.md#关键点 | testing | 关键点 | deterministic,pseudorandom,loadgen,testing,reproducibility | auto | - | active | - **确定性**：给定 error_rate 的序列完全由进程启动顺序决定，重复压测结果稳定 - **分布均匀**：s… |
| testing/deterministic-pseudorandom-loadgen.md#压测可复现的确定性伪随机（原子计数器+哈希） | testing | 压测可复现的确定性伪随机（原子计数器+哈希） | deterministic,pseudorandom,loadgen,testing,reproducibility | auto | - | active | - |
| testing/deterministic-pseudorandom-loadgen.md#方案 | testing | 方案 | deterministic,pseudorandom,loadgen,testing,reproducibility | auto | - | active | **进程级原子计数器 + 乘法哈希** (`proxy/mock.rs:2-16`)：  ```rust static … |
| testing/deterministic-pseudorandom-loadgen.md#用途 | testing | 用途 | deterministic,pseudorandom,loadgen,testing,reproducibility | auto | - | active | - mock 平台的 error_rate 注入 - 压测场景的确定性故障模拟 - 内存/CPU 基准测试（需要重复压测… |
| testing/deterministic-pseudorandom-loadgen.md#确定性伪随机负载生成 | testing | 确定性伪随机负载生成 | deterministic,pseudorandom,loadgen,testing,reproducibility | auto | - | active | - |
| testing/deterministic-pseudorandom-loadgen.md#问题 | testing | 问题 | deterministic,pseudorandom,loadgen,testing,reproducibility | auto | - | active | 压测场景（尤其是性能/内存压测）需要可复现的伪随机行为，用于注入 `error_rate=0.05`（5% 请求返回 4… |
| testing/module-load-time-constant-test-rule.md#关联 | testing | 关联 | module-load,performance,constant,testing,startup | auto | - | active / →time-zone-minute-arithmetic | [[time-zone-minute-arithmetic]] (时区换算硬约束) |
| testing/module-load-time-constant-test-rule.md#反例 / 常见错误 | testing | 反例 / 常见错误 | module-load,performance,constant,testing,startup | auto | - | active | / 错误                          / 为什么错                        … |
| testing/module-load-time-constant-test-rule.md#案例 | testing | 案例 | module-load,performance,constant,testing,startup | auto | - | active | - time-models-timezone task (commit d5b00753) — peakHours.ts… |
| testing/module-load-time-constant-test-rule.md#模块加载时间常数测试规则 | testing | 模块加载时间常数测试规则 | module-load,performance,constant,testing,startup | auto | - | active | - |
| testing/module-load-time-constant-test-rule.md#正解：纯函数内核参数化（硬约束，关键） | testing | 正解：纯函数内核参数化（硬约束，关键） | module-load,performance,constant,testing,startup | auto | - | active | ### MUST 两层函数分离（参数化内核 + 便捷包装）  ```ts /** 公开常数：模块加载时求值，用于默认行为… |
| testing/module-load-time-constant-test-rule.md#落地 checklist | testing | 落地 checklist | module-load,performance,constant,testing,startup | auto | - | active | ```bash # 1. 验证纯函数内核（offset 参数显式） grep -A5 "export function … |
| testing/module-load-time-constant-test-rule.md#触发场景 | testing | 触发场景 | module-load,performance,constant,testing,startup | auto | - | active | 模块在加载时求值的常数（如本地时区偏移 `LOCAL_OFFSET_MINUTES`），需要在单测中覆盖不同时区场景。 |
| testing/module-load-time-constant-test-rule.md#适用 | testing | 适用 | module-load,performance,constant,testing,startup | auto | - | active | - 任何模块加载时求值的常数（时区、配置、初始化状态）需参数化单测的场景 - 纯函数测试（数学函数、格式转换、换算） |
| testing/module-load-time-constant-test-rule.md#陷阱：vi.spyOn(Date.prototype, "getTimezoneOffset") 对模块常数无效 | testing | 陷阱：vi.spyOn(Date.prototype, "getTimezoneOffset") 对模块常数无效 | module-load,performance,constant,testing,startup | auto | - | active | > 时区常数 `LOCAL_OFFSET_MINUTES = -new Date().getTimezoneOffset… |
| ts-rust-boundary/mock-config-4layer-consistency.md#Mock 平台四层配置一致性 | ts-rust-boundary | Mock 平台四层配置一致性 | mock,config,consistency,ts-rust-boundary,adapter | auto | - | active | - |
| ts-rust-boundary/mock-config-4layer-consistency.md#mock 配置四层覆盖的字段一致性检查 | ts-rust-boundary | mock 配置四层覆盖的字段一致性检查 | mock,config,consistency,ts-rust-boundary,adapter | auto | - | active | - |
| ts-rust-boundary/mock-config-4layer-consistency.md#失配场景 | ts-rust-boundary | 失配场景 | mock,config,consistency,ts-rust-boundary,adapter | auto | - | active | / 症状 / 原因 / /---/---/ / TS 编辑器赋值后无效 / `serializeMockConfig` … |
| ts-rust-boundary/mock-config-4layer-consistency.md#检查表（四处同步） | ts-rust-boundary | 检查表（四处同步） | mock,config,consistency,ts-rust-boundary,adapter | auto | - | active | ### 1. Rust struct 定义 (`config.rs:11-25`) - [ ] 新字段声明的类型：`Op… |
| ts-rust-boundary/mock-config-4layer-consistency.md#用途 | ts-rust-boundary | 用途 | mock,config,consistency,ts-rust-boundary,adapter | auto | - | active | Rust↔TS 跨边界的配置字段迭代通用检查表。适用于： - 平台/插件配置扩展 - 新增可选设置 - 配置升级 mig… |
| ts-rust-boundary/mock-config-4layer-consistency.md#问题 | ts-rust-boundary | 问题 | mock,config,consistency,ts-rust-boundary,adapter | auto | - | active | mock 配置在四层跨 Rust↔TS 边界流转，任一处字段定义/序列化不一致都导致静默失配：  1. **Rust s… |
| ts-rust-boundary/optional-config-backward-compat.md#Option<T> 可选字段的向后兼容方案 | ts-rust-boundary | Option<T> 可选字段的向后兼容方案 | optional-config,backward-compatibility,ts-rust-boundary,adapter | auto | - | active | - |
| ts-rust-boundary/optional-config-backward-compat.md#关键点 | ts-rust-boundary | 关键点 | optional-config,backward-compatibility,ts-rust-boundary,adapter | auto | - | active | - **旧字段保留**：必须保留兼容入口，不删不改 - **Option/undefined 对应**：Rust `Op… |
| ts-rust-boundary/optional-config-backward-compat.md#可选配置向后兼容性 | ts-rust-boundary | 可选配置向后兼容性 | optional-config,backward-compatibility,ts-rust-boundary,adapter | auto | - | active | - |
| ts-rust-boundary/optional-config-backward-compat.md#方案 | ts-rust-boundary | 方案 | optional-config,backward-compatibility,ts-rust-boundary,adapter | auto | - | active | **Rust 端** (`config.rs:11-25`)： ```rust pub struct MockConfi… |
| ts-rust-boundary/optional-config-backward-compat.md#用途 | ts-rust-boundary | 用途 | optional-config,backward-compatibility,ts-rust-boundary,adapter | auto | - | active | 配置迭代的通用方案，适用于： - 新增可选旋钮 - 旧版本平台配置升级 - 分阶段特性开关（旧特性先 disable，新… |
| ts-rust-boundary/optional-config-backward-compat.md#问题 | ts-rust-boundary | 问题 | optional-config,backward-compatibility,ts-rust-boundary,adapter | auto | - | active | 新旋钮常需跨 Rust↔TS 边界，并与旧配置字段共存以确保向后兼容。  例：`mock` 配置新增 `ttft_ms`… |
