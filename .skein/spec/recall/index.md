# SKEIN recall 规则索引 (章节粒度: 一行一条规则)

类目: arch(116), build(51), cross-layer(10), db(23), domain(77), encoding(4), frontend(46), git(6), i18n(9), ops(5), optimization(6), proxy(39), reuse(6), shadcn(49), skein(2), style(11), test(12), testing(5), theme(5), ts-rust-boundary(10) · 关联见 [backlinks.md](backlinks.md)

| rule (topic.md#标题) | category | title | keywords | status/出链 | summary |
|---|---|---|---|---|---|
| arch/auto-fix-downgrade-33.md#关联 | arch | 关联 | agent,handler,branch,platform,wire,sse | active / →trellis-04 | dashmap-sharding (session 映射) [[trellis-04]] (enum 变体同步) |
| arch/auto-fix-downgrade-33.md#判定：分支 vs wire | arch | 判定：分支 vs wire | agent,handler,branch,platform,wire,sse | active | / 特征 / wire 层 / handler 分支 / /------/---------/-------------… |
| arch/auto-fix-downgrade-33.md#反例 | arch | 反例 | agent,handler,branch,platform,wire,sse | active | ❌ 新 agent 平台塞 wire 层 → adapter 改到吐血 ❌ 分支内做多候选 retry → agent … |
| arch/auto-fix-downgrade-33.md#触发场景 | arch | 触发场景 | agent,handler,branch,platform,wire,sse | active | 新增「agent-as-LLM」类平台（无标准 chat completions wire，API 形态是 sessio… |
| arch/auto-fix-downgrade-33.md#适用 | arch | 适用 | agent,handler,branch,platform,wire,sse | active | agent-as-LLM 平台接入（Mock/ClaudeCode/Devin/Factory） |
| arch/auto-fix-downgrade-33.md#陷阱-正解 | arch | 陷阱-正解 | agent,handler,branch,platform,wire,sse | active | - **陷阱**: 新平台硬塞 wire 层 → adapter/converter 反复打补丁、协议转换丢字段、候选切… |
| arch/auto-fix-downgrade-34.md#关联 | arch | 关联 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | active / →auto-fix-downgrade-35,cross-db-subquery-handle-selection | [[cross-db-subquery-handle-selection]] (跨库读两阶段) [[auto-fix-d… |
| arch/auto-fix-downgrade-34.md#反例 | arch | 反例 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | active | ❌ 只 grep `call_traced` → 6 处 `write_conn` 漏网（s3 错误模式） ❌ 只 gr… |
| arch/auto-fix-downgrade-34.md#触发场景 | arch | 触发场景 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | active | 表从一个 SQLite 库拆到另一个库（主库→log.db / platform.db），需把该表所有访问点切到新 ha… |
| arch/auto-fix-downgrade-34.md#适用 | arch | 适用 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | active | DB 拆库迁移、表访问点归属审计 |
| arch/auto-fix-downgrade-34.md#陷阱-正解 | arch | 陷阱-正解 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | active | - **陷阱**: 只查 `call_*_traced` chokepoint → 漏掉 `.write_conn()`… |
| arch/auto-fix-downgrade-34.md#验收命令 | arch | 验收命令 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | active | ```bash # 1. wrapper 形式 grep -rn "call_platform_traced\/call… |
| arch/auto-fix-downgrade-35.md#关联 | arch | 关联 | dedup,空字段,key,数据丢失,合并 | active / →shadcn-infra-32 | [[shadcn-infra-32]] (数据清理) |
| arch/auto-fix-downgrade-35.md#反例 | arch | 反例 | dedup,空字段,key,数据丢失,合并 | active | ❌ (provider.source_segment, provider.base_url) 其中 base_url 全… |
| arch/auto-fix-downgrade-35.md#正解 | arch | 正解 | dedup,空字段,key,数据丢失,合并 | active | dedup key 选择优先级： 1. **业务唯一键**(user_id / email / name) — 最稳 2… |
| arch/auto-fix-downgrade-35.md#测试 | arch | 测试 | dedup,空字段,key,数据丢失,合并 | active | 构造 N 个对象(该字段全空但其余不同)，dedup 后必须保留 N 个(非合并为 1)。 |
| arch/auto-fix-downgrade-35.md#触发场景 | arch | 触发场景 | dedup,空字段,key,数据丢失,合并 | active | 写任何 dedup / 去重 / 合并逻辑(HashSet key / HashMap key / groupBy ke… |
| arch/auto-fix-downgrade-35.md#适用 | arch | 适用 | dedup,空字段,key,数据丢失,合并 | active | dedup / 去重 / 合并逻辑、数据导入解析 |
| arch/auto-fix-downgrade-35.md#陷阱 | arch | 陷阱 | dedup,空字段,key,数据丢失,合并 | active | 字段设计为空(待后续回填 / 占位)但被用作 dedup key → N 个对象共享同一空值 → HashSet 全撞 … |
| arch/auto-fix-downgrade-38.md#MUST 流程 | arch | MUST 流程 | enum,serde,db,migration,rust,panic | active | 1. 写 migration: DELETE FROM table WHERE enum_column = 'delet… |
| arch/auto-fix-downgrade-38.md#关联 | arch | 关联 | enum,serde,db,migration,rust,panic | active / →shadcn-infra-32,trellis-04 | [[shadcn-infra-32]] (locale 清理) [[trellis-04]] (TS ↔ Rust en… |
| arch/auto-fix-downgrade-38.md#反例 | arch | 反例 | enum,serde,db,migration,rust,panic | active | ❌ 先删代码再 migration → migration 期间所有访问 panic ❌ 只改 TS 未改 Rust e… |
| arch/auto-fix-downgrade-38.md#硬约束 | arch | 硬约束 | enum,serde,db,migration,rust,panic | active | **删 serde 落库的 enum 变体前必须先 migration DELETE DB 旧值**，否则代码中 `fr… |
| arch/auto-fix-downgrade-38.md#触发场景 | arch | 触发场景 | enum,serde,db,migration,rust,panic | active | 删 serde 落库的 enum 变体时。 |
| arch/auto-fix-downgrade-38.md#适用 | arch | 适用 | enum,serde,db,migration,rust,panic | active | serde enum 变体删除、DB schema enum 迁移、前后端 enum 同步 |
| arch/coding-plan-utilization-calib-fix-25.md#coding plan 校准链路 base_url 真值源 = endpoint 级 | arch | coding plan 校准链路 base_url 真值源 = endpoint 级 | coding-plan,base_url,quota,calibration,finish,est_coding_plan | active | coding plan 平台 preset 平台级 base_url 恒为 None (真 base_url 在 end… |
| arch/cross-db-subquery-handle-selection.md#Cross-ref | arch | Cross-ref | db,sqlite,跨库,补查,handle,闭包,cpp,平台名,N+1 | active / →auto-fix-downgrade-34 | - sqlite-cross-db-no-join（跨库禁 JOIN，强制拆闭包 + Rust 合并） - [[auto… |
| arch/cross-db-subquery-handle-selection.md#MUST 规则 | arch | MUST 规则 | db,sqlite,跨库,补查,handle,闭包,cpp,平台名,N+1 | active | 跨库补查闭包的 handle **必须按补查表的库归属选**，禁顺手复用主表 handle。 |
| arch/cross-db-subquery-handle-selection.md#正确写法（✅） | arch | 正确写法（✅） | db,sqlite,跨库,补查,handle,闭包,cpp,平台名,N+1 | active | ```rust // 主查走 log.db handle let logs = proxy_log_handle.cal… |
| arch/cross-db-subquery-handle-selection.md#错误样本（❌） | arch | 错误样本（❌） | db,sqlite,跨库,补查,handle,闭包,cpp,平台名,N+1 | active | ```rust // proxy_log 在 log.db，补查 cpp.name 在 platform.db prox… |
| arch/cross-db-subquery-handle-selection.md#验收 | arch | 验收 | db,sqlite,跨库,补查,handle,闭包,cpp,平台名,N+1 | active | ```bash # 找跨库补查点（同函数 / 同闭包内出现多库表名） grep -rn 'FROM "proxy_log… |
| arch/non-typical-sql-audit-pattern.md#Cross-ref | arch | Cross-ref | db,sqlite,sql,审计,helper,裸sql,grep,易漏,访问点 | active / →auto-fix-downgrade-34 | - [[auto-fix-downgrade-34]]（访问点审计总则，本文是其子形式之一） |
| arch/non-typical-sql-audit-pattern.md#MUST 审计两形态 | arch | MUST 审计两形态 | db,sqlite,sql,审计,helper,裸sql,grep,易漏,访问点 | active | 拆库审计时 **禁只 grep helper 函数名**，必须同时查：  1. **Helper 函数形式**：`loa… |
| arch/non-typical-sql-audit-pattern.md#漏网样本（task config-db-split s5） | arch | 漏网样本（task config-db-split s5） | db,sqlite,sql,审计,helper,裸sql,grep,易漏,访问点 | active | - `SELECT ... FROM "group" WHERE auto_from_platform` 不经任何 he… |
| arch/non-typical-sql-audit-pattern.md#验收命令 | arch | 验收命令 | db,sqlite,sql,审计,helper,裸sql,grep,易漏,访问点 | active | ```bash # 按被拆表名 grep（FROM "table"），覆盖所有访问形态 grep -rn 'FROM "… |
| arch/parser-multi-path-format-symmetry.md#Cross-ref | arch | Cross-ref | parser,多路径,symmetry,对称,格式识别,抽函数,复用,入口分裂,oauth | active / →auto-fix-downgrade-35,cpa-oauth-credential-format | - `src-tauri/crates/aidog_core/src/gateway/cpa_import/parser… |
| arch/parser-multi-path-format-symmetry.md#How to apply | arch | How to apply | parser,多路径,symmetry,对称,格式识别,抽函数,复用,入口分裂,oauth | active | 1. grep parser 所有入口(`parse_*` / `scan_*` / `import_*`), 列各入口… |
| arch/parser-multi-path-format-symmetry.md#Why | arch | Why | parser,多路径,symmetry,对称,格式识别,抽函数,复用,入口分裂,oauth | active | 多入口是常见模式(用户单文件 vs 批量目录 vs 压缩包)。格式识别逻辑若内联在各入口, 易漏对称: - 入口 A 加… |
| arch/parser-multi-path-format-symmetry.md#规则 | arch | 规则 | parser,多路径,symmetry,对称,格式识别,抽函数,复用,入口分裂,oauth | active | parser 有多个入口(parse_single_file / scan_dir / scan_auth_dir / … |
| arch/rule-49.md#关联 | arch | 关联 | tauri,window,popover,performance,复用,hide/show,NSWindow | active / →rule-45,trellis-03,trellis-18 | [[rule-45]] (popover 域划分) / [[trellis-03]] (Crate 边界契约) / [[… |
| arch/rule-49.md#反例 | arch | 反例 | tauri,window,popover,performance,复用,hide/show,NSWindow | active | ```rust // ❌ 陷阱实现（每次销毁） if let Some(w) = app.get_webview_win… |
| arch/rule-49.md#实现清单 | arch | 实现清单 | tauri,window,popover,performance,复用,hide/show,NSWindow | active | - [ ] `app_setup.rs::setup` 阶段 `prebuild_popover()`：`.visibl… |
| arch/rule-49.md#性能收益 | arch | 性能收益 | tauri,window,popover,performance,复用,hide/show,NSWindow | active | - 消除冷启 webview (setup 预建一次)。 - 去掉 tray click 时的 4 路 IPC 瀑布（背… |
| arch/rule-49.md#案例 | arch | 案例 | tauri,window,popover,performance,复用,hide/show,NSWindow | active | - popover-perf task (commit 14ec141d)：预建隐藏窗 + toggle hide/sh… |
| arch/rule-49.md#触发场景 | arch | 触发场景 | tauri,window,popover,performance,复用,hide/show,NSWindow | active | 实现 Tauri 桌面应用的浮窗（如托盘 popover）时，需要避免每次点击都冷启 webview，导致的延迟与卡顿。 |
| arch/rule-49.md#适用 | arch | 适用 | tauri,window,popover,performance,复用,hide/show,NSWindow | active | - Tauri 桌面应用浮窗（托盘 popover、context menu、floating panel） - 需要快… |
| arch/rule-49.md#陷阱-正解 | arch | 陷阱-正解 | tauri,window,popover,performance,复用,hide/show,NSWindow | active | ❌ **陷阱**：tray 点击每次 destroy + 新建窗口 → 冷启 webview + 瀑布 IPC 4 路 … |
| arch/rule-56.md#关联 | arch | 关联 | gemini,sse,streaming,adapter,parameter | active / →rule-57,rule-58 | [[rule-57]] [[rule-58]] |
| arch/rule-56.md#案例 | arch | 案例 | gemini,sse,streaming,adapter,parameter | active | - arch-deepen-2 commit `39a6614c`：gateway/proxy/forward.rs:2… |
| arch/rule-56.md#正解 | arch | 正解 | gemini,sse,streaming,adapter,parameter | active | 向 Gemini 端点拼入 `?alt=sse` 参数，确保响应格式为 Server-Sent Events。 |
| arch/rule-56.md#触发场景 | arch | 触发场景 | gemini,sse,streaming,adapter,parameter | active | 改 gemini adapter 或调试 Gemini streaming 响应时。 |
| arch/rule-56.md#适用 | arch | 适用 | gemini,sse,streaming,adapter,parameter | active | - Gemini 协议 SSE 响应处理 - 其他 SSE 适配器的对称性检查（防止他协议有类似参数需求遗漏） |
| arch/rule-56.md#陷阱 | arch | 陷阱 | gemini,sse,streaming,adapter,parameter | active | 不带 `?alt=sse` 参数时，Gemini API 响应体不是 SSE 格式（返回普通 JSON 数组），`str… |
| arch/rule-57.md#关联 | arch | 关联 | protocol,serde,wire,codegen,enum | active / →rule-05 | [[rule-05]] |
| arch/rule-57.md#案例 | arch | 案例 | protocol,serde,wire,codegen,enum | active | - gateway/models/protocol.rs:173 定义 wire_str() - arch-deepen… |
| arch/rule-57.md#正解 | arch | 正解 | protocol,serde,wire,codegen,enum | active | 统一用 `Protocol::wire_str()` 方法序列化协议名。 |
| arch/rule-57.md#触发场景 | arch | 触发场景 | protocol,serde,wire,codegen,enum | active | 在 proxy/forward 层需要获取协议名或序列化 Protocol enum 时。 |
| arch/rule-57.md#适用 | arch | 适用 | protocol,serde,wire,codegen,enum | active | - Protocol enum 序列化时 - adapter 分发时协议名判定 |
| arch/rule-57.md#陷阱 | arch | 陷阱 | protocol,serde,wire,codegen,enum | active | 禁手写 `serde_json::to_string(&x).trim_matches('"')` 或其他字符串转换，容… |
| arch/rule-58.md#关联 | arch | 关联 | adapter,dead_code,whitelist,protocol,authority | active / →rule-07 | [[rule-07]] |
| arch/rule-58.md#案例 | arch | 案例 | adapter,dead_code,whitelist,protocol,authority | active | - arch-deepen-2 commit `78e32df4`：删的 5 个 vendor adapter（glm_… |
| arch/rule-58.md#正解 | arch | 正解 | adapter,dead_code,whitelist,protocol,authority | active | **唯一权威 = `gateway/proxy/forward.rs:85-86` 的 `is_valid_wire_p… |
| arch/rule-58.md#触发场景 | arch | 触发场景 | adapter,dead_code,whitelist,protocol,authority | active | 删除 vendor adapter 文件或判定某 adapter 是否属于死代码时。 |
| arch/rule-58.md#适用 | arch | 适用 | adapter,dead_code,whitelist,protocol,authority | active | - adapter 文件管理时 - protocol 数量变更 - 编码规范卡关：为什么要删这个文件 |
| arch/rule-58.md#陷阱 | arch | 陷阱 | adapter,dead_code,whitelist,protocol,authority | active | 用文件名判定（如「vendor 名 = 协议名」），误删活代码；或遗漏实际有白名单的 adapter。 |
| arch/rule-59.md#关联 | arch | 关联 | refactor,component,extraction,grep,dead-code | active / →auto-fix-downgrade-36 | [[auto-fix-downgrade-36]] |
| arch/rule-59.md#案例 | arch | 案例 | refactor,component,extraction,grep,dead-code | active | - arch-deepen-2 commit `1eee3975`：删 ImportDialog 内联 91 行副本前先… |
| arch/rule-59.md#检查清单 | arch | 检查清单 | refactor,component,extraction,grep,dead-code | active | ```bash # 抽前 & 抽后各一次 grep -r "ProviderRow" --include="*.tsx"… |
| arch/rule-59.md#正解 | arch | 正解 | refactor,component,extraction,grep,dead-code | active | 1. grep 搜索原位置组件名，确认所有调用点 2. 逐个改为新 import 路径 3. 最后删旧副本前再 grep… |
| arch/rule-59.md#触发场景 | arch | 触发场景 | refactor,component,extraction,grep,dead-code | active | 从大文件抽出独立组件或把函数迁移到新位置时。 |
| arch/rule-59.md#适用 | arch | 适用 | refactor,component,extraction,grep,dead-code | active | - UI 组件抽取重构 - 函数迁 crate 时 - 任何多处定义的重复 |
| arch/rule-59.md#陷阱 | arch | 陷阱 | refactor,component,extraction,grep,dead-code | active | 只 import 不渲染 = 死代码副本。原文件可能仍有内联副本，抽取后遗漏切换会导致两份代码。 |
| arch/rule-60.md#关联 | arch | 关联 | command,tauri,handler,migration,invoke,symmetry | active | - |
| arch/rule-60.md#案例 | arch | 案例 | command,tauri,handler,migration,invoke,symmetry | active | - arch-deepen-2 batch 3：commands 迁 aidog_core 时，verify 用 com… |
| arch/rule-60.md#正解 | arch | 正解 | command,tauri,handler,migration,invoke,symmetry | active | **invoke 名的真值源 = `src-tauri/src/startup.rs:41` 的 `tauri::gen… |
| arch/rule-60.md#触发场景 | arch | 触发场景 | command,tauri,handler,migration,invoke,symmetry | active | command 跨 crate 搬迁后（新增、删除、拆分 command）。 |
| arch/rule-60.md#适用 | arch | 适用 | command,tauri,handler,migration,invoke,symmetry | active | - command 跨 crate 搬迁 - 新增/删除 command - 重构后 sanity check |
| arch/rule-60.md#陷阱 | arch | 陷阱 | command,tauri,handler,migration,invoke,symmetry | active | 改了 Rust 函数签名或迁移位置，却漏改了前端 invoke 名或 startup.rs 注册，导致静默失败。 |
| arch/rule-62.md#关联 | arch | 关联 | i18n,migration,locale,key,coverage,comm | active | - |
| arch/rule-62.md#案例 | arch | 案例 | i18n,migration,locale,key,coverage,comm | active | - arch-deepen-2 c3-commands batch 3：搬迁时检查 system/ai_tools/cl… |
| arch/rule-62.md#正解 | arch | 正解 | i18n,migration,locale,key,coverage,comm | active | 搬迁前后比对 locale key 集合（grep 源代码找 namespace 模式），用 comm -23 差集查漏… |
| arch/rule-62.md#触发场景 | arch | 触发场景 | i18n,migration,locale,key,coverage,comm | active | command/组件迁 crate 或改名时，若涉及 i18n key（如 UI 文案）。 |
| arch/rule-62.md#适用 | arch | 适用 | i18n,migration,locale,key,coverage,comm | active | - 跨 crate 搬迁涉及 i18n - rename command 时 - 删减功能前验证 |
| arch/rule-62.md#陷阱 | arch | 陷阱 | i18n,migration,locale,key,coverage,comm | active | 不动 locale 文件时 `yarn check-i18n` 查不出搬迁丢 key（新位置 key 可能取名不同）。 |
| arch/rule-64.md#关联 | arch | 关联 | tauri,command,macro,parameter,mut | active | - |
| arch/rule-64.md#案例 | arch | 案例 | tauri,command,macro,parameter,mut | active | - arch-deepen-2：迁 command 时遇此限制 |
| arch/rule-64.md#正解 | arch | 正解 | tauri,command,macro,parameter,mut | active | 去掉函数签名中的 `mut`，在函数体首行用 `let mut x = x;` 重绑定： ```rust // 错误 #… |
| arch/rule-64.md#触发场景 | arch | 触发场景 | tauri,command,macro,parameter,mut | active | Tauri command 函数形参中使用 `mut` 修饰时。 |
| arch/rule-64.md#适用 | arch | 适用 | tauri,command,macro,parameter,mut | active | - Tauri command 签名设计 - 其他 proc macro 类似限制排查 |
| arch/rule-64.md#陷阱 | arch | 陷阱 | tauri,command,macro,parameter,mut | active | `tauri_command!` 宏模式 `$($arg:ident : $ty:ty),*` 不匹配 `mut x: … |
| arch/shadcn-infra-32.md#关联 | arch | 关联 | locale,dead-key,cleanup,responsibility,theme | active / →auto-fix-downgrade-38 | [[auto-fix-downgrade-38]] (同任务 enum 删约定) |
| arch/shadcn-infra-32.md#反例 | arch | 反例 | locale,dead-key,cleanup,responsibility,theme | active | ❌ 删 palette 只改代码不清理 locale → 死键残留 ❌ 甩给「下次整理 locale 时」→ 永远不清理… |
| arch/shadcn-infra-32.md#案例 | arch | 案例 | locale,dead-key,cleanup,responsibility,theme | active | - shadcn-infra task: 删 palette 时应同步清理 theme.color.* locale 键 |
| arch/shadcn-infra-32.md#正解 | arch | 正解 | locale,dead-key,cleanup,responsibility,theme | active | 1. **删 palette 主题**: 清理所有 `theme.color.{palette}` 相关 locale … |
| arch/shadcn-infra-32.md#流程约定 | arch | 流程约定 | locale,dead-key,cleanup,responsibility,theme | active | **删除主题/功能导致的 locale 死键，由删该主题/功能的 task 同源清理**，不甩给下游消费 task。 |
| arch/shadcn-infra-32.md#适用 | arch | 适用 | locale,dead-key,cleanup,responsibility,theme | active | locale 清理、主题删除、功能下架、enum 变体删除 |
| arch/shadcn-infra-32.md#陷阱 | arch | 陷阱 | locale,dead-key,cleanup,responsibility,theme | active | - **陷阱**: 删代码只删 TS 类型，locale 死键留给后续清理 → 下次改 locale 人困惑 - **陷… |
| arch/trellis-03.md#C8 复查清单模式 (MUST，迁移期临时合法 → 后续 task 改) | arch | C8 复查清单模式 (MUST，迁移期临时合法 → 后续 task 改) | crate,boundary,边界,commands,aidog_core,event,依赖 | active | - 迁 command 文件时若发现 **同 crate 内部** 跨域直调（如 `commands_platform:… |
| arch/trellis-03.md#Cross-reference | arch | Cross-reference | crate,boundary,边界,commands,aidog_core,event,依赖 | active | - workspace 重构过程契约（PoC 骨架门禁 + 核心提取下沉防循环范式）: [Cargo Workspace… |
| arch/trellis-03.md#实例 | arch | 实例 | crate,boundary,边界,commands,aidog_core,event,依赖 | active | - task 07-10-cmd-proxy（C4 commands-proxy crate 落地）: 5 源文件（pr… |
| arch/trellis-03.md#范式 (MUST，稳态边界规则，与 cargo-workspace.md 重构过程契约互补) | arch | 范式 (MUST，稳态边界规则，与 cargo-workspace.md 重构过程契约互补) | crate,boundary,边界,commands,aidog_core,event,依赖 | active | workspace 拓扑（commands-restructure 落地后）：`crates/{aidog_core, … |
| arch/trellis-03.md#验收断言（可复用） | arch | 验收断言（可复用） | crate,boundary,边界,commands,aidog_core,event,依赖 | active | ```bash # 规则 1: commands_* 间零互依赖 grep -rn 'commands_platform… |
| arch/trellis-04.md#Cross-reference | arch | Cross-reference | protocol,enum,变体,grep,serde,match,union | active | - research 结论：`.trellis/tasks/archive/2026-07/07-10-protocol… |
| arch/trellis-04.md#serde round-trip + JSON key 对齐 (MUST) | arch | serde round-trip + JSON key 对齐 (MUST) | protocol,enum,变体,grep,serde,match,union | active | - `#[serde(rename = "<key>")]` 与 `platform-presets.json` pro… |
| arch/trellis-04.md#命中点 3 类分类（据实判定改动面） | arch | 命中点 3 类分类（据实判定改动面） | protocol,enum,变体,grep,serde,match,union | active | grep 同构变体命中点，按下列 3 类分类，**仅第 1 类必须改**：  1. **enum 定义 + serde … |
| arch/trellis-04.md#实例 | arch | 实例 | protocol,enum,变体,grep,serde,match,union | active | task 07-10-protocols-rust-enum：+3 cp 变体（KimiCoding/QianfanCo… |
| arch/trellis-04.md#新增变体 MUST 先 grep 同构变体命中点 (MUST) | arch | 新增变体 MUST 先 grep 同构变体命中点 (MUST) | protocol,enum,变体,grep,serde,match,union | active | 新增 `Protocol` 变体前，**MUST** grep 现有同构变体全链命中点，据实际命中分类决定改动面，禁预设… |
| arch/trellis-04.md#零专属 match 臂 → 加枚举即覆盖 (MUST) | arch | 零专属 match 臂 → 加枚举即覆盖 (MUST) | protocol,enum,变体,grep,serde,match,union | active | **反直觉发现**：`router.rs` / `adapter/converter.rs` / `quota.rs` … |
| arch/trellis-04.md#验收断言（可复用） | arch | 验收断言（可复用） | protocol,enum,变体,grep,serde,match,union | active | ```bash # 新变体字面量全链命中点清单（据分类决定改动面） grep -rn '<NewVariant>\/<n… |
| arch/trellis-05.md#AppContext 预热缓存 (best-effort) | arch | AppContext 预热缓存 (best-effort) | derived,constants,docpromise,defaults,派生,presets,async | active | AppContext 顶层调一次 `buildXFromPresets().catch(console.error)` … |
| arch/trellis-05.md#Cross-reference | arch | Cross-reference | derived,constants,docpromise,defaults,派生,presets,async | active | - 真值源: `src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆… |
| arch/trellis-05.md#单真值源派生 (MUST) | arch | 单真值源派生 (MUST) | derived,constants,docpromise,defaults,派生,presets,async | active | 前端平台 / 协议类大枚举常量（`PROTOCOLS` / `PROTOCOL_LABELS` / `PROTOCOL_… |
| arch/trellis-05.md#实例 | arch | 实例 | derived,constants,docpromise,defaults,派生,presets,async | active | task 07-10-protocols-frontend-derive（C3）： - 删 `PROTOCOLS`（81… |
| arch/trellis-05.md#小常量例外（保留硬编码） | arch | 小常量例外（保留硬编码） | derived,constants,docpromise,defaults,派生,presets,async | active | 非后端真值源映射的小常量（请求格式协议 5 条 `ENDPOINT_PROTOCOLS` / 路由判定 / UI 固定枚… |
| arch/trellis-05.md#调用点 async 化范式 (MUST) | arch | 调用点 async 化范式 (MUST) | derived,constants,docpromise,defaults,派生,presets,async | active | 派生函数 async 后，所有 caller **MUST** 改 `useEffect + useState` 模式，… |
| arch/trellis-05.md#验收断言（可复用） | arch | 验收断言（可复用） | derived,constants,docpromise,defaults,派生,presets,async | active | ```bash # 派生层单 RPC 缓存（docPromise module-level 单次 invoke，非函数内… |
| build/rule-05.md#MUST 硬约束 | build | MUST 硬约束 | - | active | 新增 wire protocol 时必须同步更新以下白名单，否则新协议会导致 route fail： - forward… |
| build/rule-05.md#关联 | build | 关联 | - | active / →rule-52,rule-53 | [[rule-52]] [[rule-53]] |
| build/rule-05.md#反例 | build | 反例 | - | active | - 新增 protocol X 但未加入白名单 → matched_ep=None 时 fallback 到 platf… |
| build/rule-05.md#触发场景 | build | 触发场景 | - | active | - converter-reasoning-content task：bug1 根因分析发现 matched_ep=No… |
| build/rule-05.md#适用 | build | 适用 | - | active | - 所有新增 wire protocol（endpoint 协议层）的变更 - 非 platform_type（平台别名… |
| build/rule-06.md#MUST 硬约束 | build | MUST 硬约束 | - | active | converter 双向转（source→wire 请求 + wire→source 响应）与 endpoint 选择解… |
| build/rule-06.md#关联 | build | 关联 | - | active | - |
| build/rule-06.md#反例 | build | 反例 | - | active | - ❌ 误判：endpoint 层限制只许选同协议 → converter 能力已就绪，endpoint 无需自我限制 … |
| build/rule-06.md#案例 | build | 案例 | - | active / →rule-07,rule-55 | - endpoint-cross-protocol-fallback task：converter 5×5 已就绪，en… |
| build/rule-06.md#适用 | build | 适用 | - | active | - 所有新增 wire protocol 的变更 - endpoint 跨协议回退扩展 - converter 双向转换… |
| build/rule-07.md#MUST 硬约束 | build | MUST 硬约束 | - | active | is_valid_wire_protocol gate 触发（502）说明 endpoint 选择失败（matched_… |
| build/rule-07.md#关联 | build | 关联 | - | active | - |
| build/rule-07.md#反例 | build | 反例 | - | active | - 只修白名单而未修 select → 新协议仍 502（根因未除） - 误判为 endpoint 配置缺 protoc… |
| build/rule-07.md#案例 | build | 案例 | - | active / →rule-05,rule-54 | - converter-reasoning-content bug1：preset 未加载致 matched_ep=No… |
| build/rule-07.md#适用 | build | 适用 | - | active | - 所有 502 route fail 场景 - is_valid_wire_protocol gate 触发 - en… |
| build/rule-61.md#关联 | build | 关联 | cargo,clippy,cache,warning,touch,rebuild | active / →rule-63 | [[rule-63]] |
| build/rule-61.md#案例 | build | 案例 | cargo,clippy,cache,warning,touch,rebuild | active | - arch-deepen-2：迁移函数后 clippy 无新输出，touch 才触发重编检查 |
| build/rule-61.md#正解 | build | 正解 | cargo,clippy,cache,warning,touch,rebuild | active | 修改源文件后跑 clippy 前，先 `touch` 该文件强制重编： ```bash touch src-tauri/… |
| build/rule-61.md#触发场景 | build | 触发场景 | cargo,clippy,cache,warning,touch,rebuild | active | 修改后再跑 `cargo clippy` 判断 warning 数时。 |
| build/rule-61.md#适用 | build | 适用 | cargo,clippy,cache,warning,touch,rebuild | active | - 验证 clippy 改动效果 - 高频编译场景 - 持续集成前检查 |
| build/rule-61.md#陷阱 | build | 陷阱 | cargo,clippy,cache,warning,touch,rebuild | active | 同命令第二次跑输出为空（命中编译缓存），易误判「0 warning」实际仍有。 |
| build/rule-63.md#关联 | build | 关联 | env,compile-time,build.rs,cargo:rustc-env,scope | active / →rule-61 | [[rule-61]] |
| build/rule-63.md#案例 | build | 案例 | env,compile-time,build.rs,cargo:rustc-env,scope | active | - arch-deepen-2 c3-commands batch 3：commands_tray/commands_s… |
| build/rule-63.md#检查 | build | 检查 | env,compile-time,build.rs,cargo:rustc-env,scope | active | ```bash # 检查迁移后是否仍能编译通过 cargo build -p aidog_core  # 应无 env!… |
| build/rule-63.md#正解 | build | 正解 | env,compile-time,build.rs,cargo:rustc-env,scope | active | 迁移代码到新 crate 后，给**新 crate 补等价的 build.rs**，重新定义环境变量。 |
| build/rule-63.md#触发场景 | build | 触发场景 | env,compile-time,build.rs,cargo:rustc-env,scope | active | 用 `env!("XXX")` 的代码从一个 crate 迁移到另一个 crate 时。 |
| build/rule-63.md#适用 | build | 适用 | env,compile-time,build.rs,cargo:rustc-env,scope | active | - 任何用 env!() 的代码跨 crate 迁移 - workspace 多 crate 场景 - build.rs… |
| build/rule-63.md#陷阱 | build | 陷阱 | env,compile-time,build.rs,cargo:rustc-env,scope | active | `cargo:rustc-env=` 在 build.rs 中定义的环境变量**只对定义它的 crate 生效**，跨 … |
| build/shadcn-infra-28.md#关联 | build | 关联 | shadcn,cva,yarn,dependency,class-variance-authority | active / →shadcn-infra-31 | [[shadcn-infra-31]] (同任务产出的前端规则) |
| build/shadcn-infra-28.md#反例 | build | 反例 | shadcn,cva,yarn,dependency,class-variance-authority | active | ❌ 只加 UI 组件不验证 cva → 运行时崩 ❌ 改 package.json 后不 yarn install → … |
| build/shadcn-infra-28.md#案例 | build | 案例 | shadcn,cva,yarn,dependency,class-variance-authority | active | - shadcn-infra task: 首次 `shadcn add` 后运行时崩，发现 cva 缺失 - 根因: y… |
| build/shadcn-infra-28.md#触发场景 | build | 触发场景 | shadcn,cva,yarn,dependency,class-variance-authority | active | 运行 `npx shadcn add` 批量添加组件后，依赖树中仅含 `@radix-ui/react-slot` 等 … |
| build/shadcn-infra-28.md#适用 | build | 适用 | shadcn,cva,yarn,dependency,class-variance-authority | active | yarn 4+ / pnp 环境，shadcn 批量 add 场景 |
| build/shadcn-infra-28.md#陷阱-正解 | build | 陷阱-正解 | shadcn,cva,yarn,dependency,class-variance-authority | active | - **陷阱**: shadcn CLI 在 yarn 4+ / pnp 环境下可能未正确解析 cva 传递依赖，只装直… |
| build/shadcn-infra-29.md#关联 | build | 关联 | vite,alias,resolve,shadcn,tsconfig | active / →shadcn-infra-28 | [[shadcn-infra-28]] (同任务 cva 依赖) |
| build/shadcn-infra-29.md#反例 | build | 反例 | vite,alias,resolve,shadcn,tsconfig | active | ❌ 只配 vite alias 不配 tsconfig → 类型检查报错 ❌ 用相对路径 `../../componen… |
| build/shadcn-infra-29.md#案例 | build | 案例 | vite,alias,resolve,shadcn,tsconfig | active | - shadcn-infra task: shadcn 生成的组件含 `import @/components/xxx`… |
| build/shadcn-infra-29.md#触发场景 | build | 触发场景 | vite,alias,resolve,shadcn,tsconfig | active | 使用 shadcn/ui 或其他假设存在 `@` 别名的库时，项目原无 `@` → `src` 的路径别名配置，导致 `… |
| build/shadcn-infra-29.md#适用 | build | 适用 | vite,alias,resolve,shadcn,tsconfig | active | shadcn/ui 迁移、Vite 从零配置、路径别名标准化 |
| build/shadcn-infra-29.md#陷阱-正解 | build | 陷阱-正解 | vite,alias,resolve,shadcn,tsconfig | active | - **陷阱**: shadcn 假设 vite 已有 `@` 别名（标准 scaffolding 如 Vite 默认模… |
| build/trellis-02.md#Cross-reference | build | Cross-reference | cargo,workspace,crate,build.rs,重构,门禁,下沉 | active | - parent design：`.trellis/tasks/07-10-commands-restructure/d… |
| build/trellis-02.md#GUI 冒烟降级（worktree 无 display 时） | build | GUI 冒烟降级（worktree 无 display 时） | cargo,workspace,crate,build.rs,重构,门禁,下沉 | active | worktree 无 `node_modules` / 无 display 无法跑 `yarn tauri dev` 全… |
| build/trellis-02.md#PoC 空骨架门禁 (MUST) | build | PoC 空骨架门禁 (MUST) | cargo,workspace,crate,build.rs,重构,门禁,下沉 | active | 单 crate → workspace 多 crate 重构 **MUST 先建空骨架 PoC 门禁**，过才放行全量迁… |
| build/trellis-02.md#PoC 门禁验收 (MUST，全量迁移前必过) | build | PoC 门禁验收 (MUST，全量迁移前必过) | cargo,workspace,crate,build.rs,重构,门禁,下沉 | active | 1. `cargo build --workspace`：0 errors（含现 root crate + N 空壳 +… |
| build/trellis-02.md#root 过渡路径迁移 (MUST) | build | root 过渡路径迁移 (MUST) | cargo,workspace,crate,build.rs,重构,门禁,下沉 | active | core 提取后 root package **过渡保留**（binary crate C10 才建），加 `aidog… |
| build/trellis-02.md#workspace.dependencies 版本对齐 (MUST) | build | workspace.dependencies 版本对齐 (MUST) | cargo,workspace,crate,build.rs,重构,门禁,下沉 | active | - `[workspace.dependencies]` 版本号 + features **MUST 逐项照抄**现 r… |
| build/trellis-02.md#子 crate 规范 (MUST) | build | 子 crate 规范 (MUST) | cargo,workspace,crate,build.rs,重构,门禁,下沉 | active | - `name` 用下划线（`commands_platform` 等，非 hyphen；目录名连字符是 Cargo 惯… |
| build/trellis-02.md#实例 | build | 实例 | cargo,workspace,crate,build.rs,重构,门禁,下沉 | active | task 07-10-ws-skeleton（commands-restructure C1）：src-tauri 单 … |
| build/trellis-02.md#核心提取下沉防循环范式 (MUST) | build | 核心提取下沉防循环范式 (MUST) | cargo,workspace,crate,build.rs,重构,门禁,下沉 | active | PoC 空骨架过门后，业务代码入 `aidog_core` 时**MUST** 据依赖关系分类下沉，防 core→com… |
| build/trellis-02.md#验收断言（可复用） | build | 验收断言（可复用） | cargo,workspace,crate,build.rs,重构,门禁,下沉 | active | ```bash # baseline 不回归 cargo test --workspace --lib / grep -… |
| build/trellis-02.md#验收断言（核心提取，可复用） | build | 验收断言（核心提取，可复用） | cargo,workspace,crate,build.rs,重构,门禁,下沉 | active | ```bash # 路径迁移彻底（root 残留核心域路径 = 漏改） grep -rn 'crate::gateway… |
| cross-layer/trellis-20.md#CRUD Pattern (MUST) | cross-layer | CRUD Pattern (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | active | - 每个 resource 必须在 `api.ts` 提供 `{ create, list, get, update, … |
| cross-layer/trellis-20.md#Data Flow (MUST) | cross-layer | Data Flow (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | active | - 数据流必须单向: Rust command → `invoke` → React `useState` → JSX … |
| cross-layer/trellis-20.md#Format Contracts (MUST) | cross-layer | Format Contracts (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | active | - 后端返回 timestamp 必须为 ISO 8601 string (`chrono::DateTime<Utc>… |
| cross-layer/trellis-20.md#Rust enum → type alias arbitrary 全 JSON 驱动 (MUST) | cross-layer | Rust enum → type alias arbitrary 全 JSON 驱动 (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | active | Rust enum 当变体集合属「后端 JSON 真值源派生」类（值集合由 `src-tauri/defaults/*.… |
| cross-layer/trellis-20.md#Rust 执行层 match 臂 → JSON 真值源配置驱动引擎 (MUST) | cross-layer | Rust 执行层 match 臂 → JSON 真值源配置驱动引擎 (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | active | Rust 执行层（如 proxy headers 注入）写死 per-variant dispatch (`match … |
| cross-layer/trellis-20.md#Tauri 窗口生命周期事件 (MUST) | cross-layer | Tauri 窗口生命周期事件 (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | active | - 窗口生命周期事件 (失焦 `Focused` / 关闭 `CloseRequested` / 缩放 `Resized… |
| cross-layer/trellis-20.md#Tauri↔React Boundary (MUST) | cross-layer | Tauri↔React Boundary (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | active | - 后端新增 Tauri command 必须在前端 `api.ts` 添加对应 invoke 包装函数 - invok… |
| cross-layer/trellis-20.md#Verification | cross-layer | Verification | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | active | ```bash # 所有 invoke 集中在 api.ts grep -rn 'invoke(' src/ / gre… |
| cross-layer/trellis-20.md#反模式 (禁) | cross-layer | 反模式 (禁) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | active | / 反模式 / 正确做法 / 触发后果 / / --- / --- / --- / / `invoke(` 散落在组件 … |
| cross-layer/trellis-20.md#持久化路径换、公共契约零改 (MUST) | cross-layer | 持久化路径换、公共契约零改 (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | active | 换持久化路径（专属表 → `setting` / JSON / 他处）时，跨 Rust↔TS **公共契约层禁改** —… |
| db/crash-safe-db-split-migration.md#Cross-ref | db | Cross-ref | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | active / →auto-fix-downgrade-34 | - [[auto-fix-downgrade-34]]（访问点审计） - dual-db-aggregate-is-me… |
| db/crash-safe-db-split-migration.md#MUST 四阶段模式（✅） | db | MUST 四阶段模式（✅） | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | active | ``` Phase 1: read-without-drop（源库读全行入 Vec，不 DROP） Phase 2: 目… |
| db/crash-safe-db-split-migration.md#crash 恢复矩阵 | db | crash 恢复矩阵 | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | active | / crash 点 / 重启行为 / /---/---/ / Phase 1 前/中 / 源表在，重读 / / Phas… |
| db/crash-safe-db-split-migration.md#保 id（MUST） | db | 保 id（MUST） | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | active | `INSERT INTO platform SELECT *` / 显式列含 id 保原 id。log.db.proxy… |
| db/crash-safe-db-split-migration.md#实例 | db | 实例 | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | active | task config-db-split（s2）：platform / group / group_platform /… |
| db/crash-safe-db-split-migration.md#禁用模式（❌） | db | 禁用模式（❌） | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | active | `read → DROP 源表 → INSERT 目标库`（notification migration 049 原模式… |
| db/filter-semantics.md#排斥列默认过滤需明确确认为产品设计意图 | db | 排斥列默认过滤需明确确认为产品设计意图 | filter,exclude,semantics,product-design,default-behavior | active | 当 task 涉及「默认排斥某类请求」的过滤逻辑时（如 Logs 主页默认隐藏 test/quota 请求），确认这是*… |
| db/pagination-offset.md#LIMIT+1 探测分页无精确总数 | db | LIMIT+1 探测分页无精确总数 | pagination,limit,offset,has_more,count,full-table-scan | active | 当分页 UI 仅需「有无下一页」而不需精确总数时，改用 LIMIT offset+pageSize+1 探测有下一页，而… |
| db/sqlite-partial-index.md#参数化查询无法触发 partial index（字面量盲区） | db | 参数化查询无法触发 partial index（字面量盲区） | sqlite,partial-index,query-plan,parameter-binding,sargable | active | SQLite 查询规划器对 partial index 的匹配仅识别 SQL 文本中的**字面量常量**谓词，不识别**… |
| db/trellis-00.md#Column Naming (MUST) | db | Column Naming (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | active | - 平台主类型列名为 `platform_type`（禁 `protocol`）；其值用 `serde_json::to… |
| db/trellis-00.md#Migration (MUST) | db | Migration (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | active | - schema 破坏式变更必须提供独立一次性迁移脚本（`scripts/`，非 app 运行时代码），迁移完成后删除 … |
| db/trellis-00.md#No NULL (MUST) | db | No NULL (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | active | - 所有 `TEXT` 列 `NOT NULL DEFAULT ''`；所有 `INTEGER` 列 `NOT NULL… |
| db/trellis-00.md#Primary Key (MUST) | db | Primary Key (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | active | - 业务表主键必须 `id INTEGER PRIMARY KEY AUTOINCREMENT`，Rust 映射 `u6… |
| db/trellis-00.md#Relations & Mappings (MUST) | db | Relations & Mappings (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | active | - 关联表（如 `group_platform`）加代理 `id` 自增主键 + 保留业务复合 `UNIQUE(grou… |
| db/trellis-00.md#Soft Delete (MUST) | db | Soft Delete (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | active | - 删除必须逻辑删：`UPDATE <table> SET deleted_at = <now_ms> WHERE id… |
| db/trellis-00.md#Table Naming (MUST) | db | Table Naming (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | active | - 表名必须**单数**，禁复数：`platform` / `group` / `group_platform` / `… |
| db/trellis-00.md#Time Fields (MUST) | db | Time Fields (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | active | - 每个表必须含 `created_at` / `updated_at` / `deleted_at`，类型 `INTE… |
| db/trellis-00.md#Verification | db | Verification | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | active | ```bash # 复数表名残留 sqlite3 ~/.aidog/aidog.db ".tables" / grep … |
| db/trellis-00.md#专属表 → setting 迁移模式 (MUST) | db | 专属表 → setting 迁移模式 (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | active | 域数据从专属表迁通用 `setting` 表时（`scope=<域>, key=<实体>` JSON），走 app 内置… |
| db/trellis-01.md#反例（禁） | db | 反例（禁） | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | active | - 禁在 handler 层才重试 route（只覆盖 route 路径，写连接死亡无法兜底；Db 层统一兜底全覆盖）。… |
| db/trellis-01.md#契约（MUST） | db | 契约（MUST） | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | active | - `call_traced` / `call_read_traced` 检测 `Error::ConnectionCl… |
| db/trellis-01.md#根因（tokio_rusqlite 0.6.0 已知行为，库层不可改） | db | 根因（tokio_rusqlite 0.6.0 已知行为，库层不可改） | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | active | - `Connection` 内部 `event_loop`（`tokio-rusqlite-0.6.0/src/lib… |
| db/trellis-01.md#验证（可 grep / 可 test） | db | 验证（可 grep / 可 test） | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | active | - `grep -n "ConnectionClosed\/reopen_write_conn\/pool.pick" … |
| domain/coding-plan-utilization-calib-fix-26.md#coding plan 订阅制平台普遍无公开用量查询 API | domain | coding plan 订阅制平台普遍无公开用量查询 API | coding-plan,quota,upstream-api,degrade,custom-quota-script | active | bailian/qianfan/xiaomi/compshare 等 coding plan 订阅制平台上游均无公开程序… |
| domain/cpa-oauth-credential-format.md#Cross-ref | domain | Cross-ref | cpa,oauth,credential,cliproxyapi,access_token,model_aliases,xai,multi-account,凭据,导入 | active / →auto-fix-downgrade-35,parser-multi-path-format-symmetry | - `src-tauri/crates/aidog_core/src/gateway/cpa_import/parser… |
| domain/cpa-oauth-credential-format.md#OAuth 类型枚举（CpaOAuthType） | domain | OAuth 类型枚举（CpaOAuthType） | cpa,oauth,credential,cliproxyapi,access_token,model_aliases,xai,multi-account,凭据,导入 | active | codex / claude / kimi / xai / vertex / aistudio / antigravit… |
| domain/cpa-oauth-credential-format.md#多账号语义（CLIProxyAPI） | domain | 多账号语义（CLIProxyAPI） | cpa,oauth,credential,cliproxyapi,access_token,model_aliases,xai,multi-account,凭据,导入 | active / →auto-fix-downgrade-35 | - 同一 OAuth 类型(如 xai)可有多个凭据(各 email 不同)→ **各自独立平台**(负载均衡) - d… |
| domain/cpa-oauth-credential-format.md#格式结构 | domain | 格式结构 | cpa,oauth,credential,cliproxyapi,access_token,model_aliases,xai,multi-account,凭据,导入 | active | CLIProxyAPI OAuth 凭据 JSON(auth-dir 文件 / 导出 zip 内): ```json {… |
| domain/cpa-oauth-credential-format.md#识别逻辑 | domain | 识别逻辑 | cpa,oauth,credential,cliproxyapi,access_token,model_aliases,xai,multi-account,凭据,导入 | active | - `parse_oauth_json(content) -> Option<Vec<CpaProvider>>`(pa… |
| domain/rule-51.md#关联 | domain | 关联 | protocol endpoint converter platform_type | active / →rule-05,rule-53 | [[rule-05]] [[rule-53]] |
| domain/rule-51.md#关键不变量 | domain | 关键不变量 | protocol endpoint converter platform_type | active | endpoint 协议 = converter 模块支持的格式（convert_request + parse_sse） |
| domain/rule-51.md#反例 | domain | 反例 | protocol endpoint converter platform_type | active | - 把 glm/kimi/sensenova 当作 endpoint 协议 → 转换时 panic/未实现 - 误以为有… |
| domain/rule-51.md#案例 | domain | 案例 | protocol endpoint converter platform_type | active | - converter-reasoning-content task：5 协议是 N×N 互转矩阵的锚点 - glm/k… |
| domain/rule-51.md#触发场景 | domain | 触发场景 | protocol endpoint converter platform_type | active | - endpoint 协议层只 5 种（anthropic/openai/openai_responses/openai… |
| domain/rule-51.md#适用 | domain | 适用 | protocol endpoint converter platform_type | active | - converter 模块扩展（新增 wire protocol） - N×N 协议互转设计（真值源） - 平台接入时… |
| domain/rule-51.md#陷阱-正解 | domain | 陷阱-正解 | protocol endpoint converter platform_type | active | - ❌ 混淆：以为所有 Protocol 枚举值都是「协议」 - ✅ 区分：仅 5 个可作为 endpoint 协议参与… |
| domain/rule-52.md#关联 | domain | 关联 | reasoning thinking anthropic signature converter | active / →rule-52,rule-53 | [[rule-53]] [[rule-52]] |
| domain/rule-52.md#决策背景 | domain | 决策背景 | reasoning thinking anthropic signature converter | active | - TrueFoundry/LiteLLM #8927 调研佐证：第三方 reasoning 无 signature -… |
| domain/rule-52.md#反例 | domain | 反例 | reasoning thinking anthropic signature converter | active | - 强行出 thinking 块 → CC 多轮交互时 400/empty or malformed - 空 reaso… |
| domain/rule-52.md#实现 | domain | 实现 | reasoning thinking anthropic signature converter | active | - openai/response.rs:13：reasoning_content 被忽略，不影响 content/to… |
| domain/rule-52.md#触发场景 | domain | 触发场景 | reasoning thinking anthropic signature converter | active | - 第三方（deepseek/sensenova/glm）reasoning_content 纯文本无 signatur… |
| domain/rule-52.md#适用 | domain | 适用 | reasoning thinking anthropic signature converter | active | - 所有第三方 → anthropic 跨协议转换 - reasoning 扩展字段处理（未来第三方新增非标准字段） |
| domain/rule-52.md#陷阱-正解 | domain | 陷阱-正解 | reasoning thinking anthropic signature converter | active | - ❌ 方案 A（标准协议）：出 thinking 块 → signature 风险 - ✅ 方案 B（务实方案）：re… |
| domain/rule-53.md#关联 | domain | 关联 | converter NonStreamResponse parse render protocol | active / →rule-52,rule-54 | [[rule-52]] [[rule-54]] |
| domain/rule-53.md#反例 | domain | 反例 | converter NonStreamResponse parse render protocol | active | - 点对点设计：新增协议时改 N 处 → O(N²) 维护成本 - 无中间归一：无法跨协议组合（如 openai→gem… |
| domain/rule-53.md#案例 | domain | 案例 | converter NonStreamResponse parse render protocol | active | - converter-reasoning-content：5×5 互转矩阵用 NonStreamResponse - … |
| domain/rule-53.md#覆盖范围 | domain | 覆盖范围 | converter NonStreamResponse parse render protocol | active | - 当前：openai → anthropic 真转换（convert_response） - 其余组合：回退透传（re… |
| domain/rule-53.md#触发场景 | domain | 触发场景 | converter NonStreamResponse parse render protocol | active | - N 协议互转设计选择：内部归一（路 A）vs 点对点（路 B） - O(N) parse + render vs O… |
| domain/rule-53.md#设计决策 | domain | 设计决策 | converter NonStreamResponse parse render protocol | active | 路 A（内部归一）： 1. 上游响应 → parse → NonStreamResponse（归一） 2. NonStr… |
| domain/rule-53.md#适用 | domain | 适用 | converter NonStreamResponse parse render protocol | active | - converter 模块扩展（新增协议/转换组合） - N×N 互转矩阵设计（converter-reasoning… |
| domain/rule-53.md#陷阱-正解 | domain | 陷阱-正解 | converter NonStreamResponse parse render protocol | active | - ❌ 路 B：点对点 N×N 函数 → 新增协议需加 N 个函数 - ✅ 路A：NonStreamResponse 作… |
| domain/rule-54.md#修复方案 | domain | 修复方案 | target_protocol platform_type matched_ep preset | active | - is_valid_wire_protocol 白名单：5 协议（anthropic/openai/openai_re… |
| domain/rule-54.md#关联 | domain | 关联 | target_protocol platform_type matched_ep preset | active / →rule-05 | [[rule-05]] |
| domain/rule-54.md#关键不变量 | domain | 关键不变量 | target_protocol platform_type matched_ep preset | active | matched_ep=None 的合法情况：preset 未加载（DB endpoints 空），非用户配置错误 |
| domain/rule-54.md#反例 | domain | 反例 | target_protocol platform_type matched_ep preset | active | - ❌ 误判：endpoint 配置缺 protocol → 实际是 DB endpoints 字段空 - ❌ 误修：加… |
| domain/rule-54.md#根因分析 | domain | 根因分析 | target_protocol platform_type matched_ep preset | active | 1. matched_ep=None 时 `unwrap_or((&route.platform.platform_ty… |
| domain/rule-54.md#案例 | domain | 案例 | target_protocol platform_type matched_ep preset | active | - converter-reasoning-content bug1：preset 未加载致 matched_ep=No… |
| domain/rule-54.md#触发场景 | domain | 触发场景 | target_protocol platform_type matched_ep preset | active | - proxy_log.target_protocol 落平台名（如 "glm"）而非 endpoint 协议（如 "o… |
| domain/rule-54.md#适用 | domain | 适用 | target_protocol platform_type matched_ep preset | active | - target_protocol 异常落平台名 - 新增 wire protocol 后 route fail - p… |
| domain/rule-55.md#关联 | domain | 关联 | - | active | - |
| domain/rule-55.md#分层不变量 | domain | 分层不变量 | - | active | - 回退仅在普通平台生效：普通平台允许跨协议回退（降低 502 率） - coding 平台永不落非 coding：步骤… |
| domain/rule-55.md#反例 | domain | 反例 | - | active | - ❌ 误判：coding 平台也跨协议回退 → 破坏 401 防护 - ❌ 误修：只修普通平台回退，忘了 coding… |
| domain/rule-55.md#案例 | domain | 案例 | - | active / →rule-06,rule-07 | - endpoint-cross-protocol-fallback task：普通平台步骤 4 泛化（同协议 > op… |
| domain/rule-55.md#触发场景 | domain | 触发场景 | - | active | - 普通平台 endpoint 选择时协议不匹配（如 anthropic 入站 + 仅 openai endpoint）… |
| domain/rule-55.md#适用 | domain | 适用 | - | active | - endpoint.rs select_endpoint_for_protocol 修改 - 跨协议回退逻辑扩展 - … |
| domain/rule-55.md#陷阱-正解 | domain | 陷阱-正解 | - | active | **陷阱**: 误以为跨协议回退可应用于所有平台类型，或回退优先级混乱。  **正解**: 普通平台步骤 4 泛化为三级… |
| domain/trellis-06.md#Config Carrier — extra.mock (MUST) | domain | Config Carrier — extra.mock (MUST) | mock,platform,extra,test,builder,error_mode | active | - mock 配置载体必须为现有 `platform.extra`（TEXT JSON 列），禁新增专用 DB 列（零迁… |
| domain/trellis-06.md#Response Builders (MUST) | domain | Response Builders (MUST) | mock,platform,extra,test,builder,error_mode | active | - 非流式: `build_response(cfg, source_protocol, model)` 按 5 协议返… |
| domain/trellis-06.md#Three-Layer Config Override (MUST) | domain | Three-Layer Config Override (MUST) | mock,platform,extra,test,builder,error_mode | active | 最终生效值 = 逐字段按优先级取首个存在者（`resolve_mock_config(extra, chat_req, … |
| domain/trellis-06.md#Verification | domain | Verification | mock,platform,extra,test,builder,error_mode | active | ```bash cd src-tauri && cargo test mock   # 全部通过（三层覆盖 / 5 协议… |
| domain/trellis-06.md#What & When (MUST) | domain | What & When (MUST) | mock,platform,extra,test,builder,error_mode | active | - `Protocol::Mock`（`models.rs`，serde rename `"mock"`）是**平台主类… |
| domain/trellis-06.md#error_mode Semantics (MUST) | domain | error_mode Semantics (MUST) | mock,platform,extra,test,builder,error_mode | active | `handle_mock`（proxy.rs）按 `error_mode` 分派，两类语义并存（delay 与 erro… |
| domain/trellis-06.md#proxy_log (MUST) | domain | proxy_log (MUST) | mock,platform,extra,test,builder,error_mode | active | - mock 分支直接写最终生效值 `log.{input_tokens,output_tokens,cache_tok… |
| domain/trellis-07.md#Frontend (MUST) | domain | Frontend (MUST) | claude,passthrough,透传,subscription,header | active | - `api.ts` Protocol union 含 `/ "claude_code"` - `Platforms.t… |
| domain/trellis-07.md#Intercept Point (MUST) | domain | Intercept Point (MUST) | claude,passthrough,透传,subscription,header | active | - 拦截点：`select_platform` 之后、`convert_request` 之前（与 mock 拦截点同区… |
| domain/trellis-07.md#No Transform / No Inject (MUST) | domain | No Transform / No Inject (MUST) | claude,passthrough,透传,subscription,header | active | - 禁 `convert_request` / 禁 `build_upstream_headers` / 禁 `appl… |
| domain/trellis-07.md#Original Request Capture (MUST) | domain | Original Request Capture (MUST) | claude,passthrough,透传,subscription,header | active | - `proxy.rs` handle_proxy 在 `req.into_parts()` **之前**捕获原始量（对… |
| domain/trellis-07.md#Verification | domain | Verification | claude,passthrough,透传,subscription,header | active | ```bash cd src-tauri && cargo test passthrough   # URL 拼接 / … |
| domain/trellis-07.md#What & When (MUST) | domain | What & When (MUST) | claude,passthrough,透传,subscription,header | active | - `Protocol::ClaudeCode`（`models.rs`，serde rename `"claude_c… |
| domain/trellis-07.md#handle_passthrough Semantics (MUST) | domain | handle_passthrough Semantics (MUST) | claude,passthrough,透传,subscription,header | active | 1. **目标 URL** = `base_url` + 客户端原始 path（+ query）。**约定 CC 平台 … |
| domain/trellis-07.md#proxy_log (MUST) | domain | proxy_log (MUST) | claude,passthrough,透传,subscription,header | active | - 透传分支**正常记** `proxy_log`：   - `source_protocol` = `target_p… |
| domain/trellis-08.md#C1 — auto_disable 触发状态码 (MUST) | domain | C1 — auto_disable 触发状态码 (MUST) | platform,error,429,auto_disable,熔断,purge,stream,status | active | `non_success.rs` handle_non_success 中, 上游非 2xx 仅以下触发 `set_pl… |
| domain/trellis-08.md#C2 — 429 分类只看 message 文本 (MUST NOT 按 error.type) | domain | C2 — 429 分类只看 message 文本 (MUST NOT 按 error.type) | platform,error,429,auto_disable,熔断,purge,stream,status | active | `classify_429(message) -> bool`(retry.rs)区分 429:  - **配额耗尽**… |
| domain/trellis-08.md#C3 — 熔断与 auto_disable 解耦 (MUST) | domain | C3 — 熔断与 auto_disable 解耦 (MUST) | platform,error,429,auto_disable,熔断,purge,stream,status | active | 熔断计数(`record_failure` vs `record_ignored`)按下表:  / 错误 / 熔断 / … |
| domain/trellis-08.md#C4 — purge 只删 401/403 或已过期 (MUST) | domain | C4 — purge 只删 401/403 或已过期 (MUST) | platform,error,429,auto_disable,熔断,purge,stream,status | active | `purge_auto_disabled_platforms`(platform_lifecycle.rs)全局 + 分… |
| domain/trellis-08.md#C5 — last_error 优先存 message 不存完整 body (MUST) | domain | C5 — last_error 优先存 message 不存完整 body (MUST) | platform,error,429,auto_disable,熔断,purge,stream,status | active | 写 `set_platform_last_error` 前用 `extract_error_message(body)`… |
| domain/trellis-08.md#C6 — stream 字段单向性：禁用 unwrap_or(false) 区分漏发与显式非流式 (MUST) | domain | C6 — stream 字段单向性：禁用 unwrap_or(false) 区分漏发与显式非流式 (MUST) | platform,error,429,auto_disable,熔断,purge,stream,status | active | **背景**：DB 全库实证（2026-07-02）—— 客户端（Claude Code）stream 字段是**单向*… |
| domain/trellis-08.md#C7 — 空流/空body 失败时 response_body MUST 落上游真实首块 (MUST) | domain | C7 — 空流/空body 失败时 response_body MUST 落上游真实首块 (MUST) | platform,error,429,auto_disable,熔断,purge,stream,status | active | **背景**：proxy 流式 peek 判 `EmptyOrError`（上游 200 但流无内容/秒断/立即[DON… |
| domain/trellis-09.md#delete_platform 契约 | domain | delete_platform 契约 | platform,delete,软删,group_platform,purge,lifecycle | active | `delete_platform(db, id)`（`src-tauri/src/gateway/db/platform… |
| domain/trellis-09.md#purge_auto_disabled_platforms | domain | purge_auto_disabled_platforms | platform,delete,软删,group_platform,purge,lifecycle | active | 复用 `delete_platform` 的语义，**不重写关联清理逻辑**：  - **全局（`group_id = … |
| domain/trellis-09.md#purge_old_soft_deleted_platforms | domain | purge_old_soft_deleted_platforms | platform,delete,软删,group_platform,purge,lifecycle | active | 定时任务（每日）：物理删除 `deleted_at > 0 AND deleted_at < now() - older… |
| domain/trellis-09.md#测试契约（test_platform_lifecycle.rs） | domain | 测试契约（test_platform_lifecycle.rs） | platform,delete,软删,group_platform,purge,lifecycle | active | - `delete_platform_preserves_groups_with_other_members`：手动组 … |
| domain/trellis-10.md#HTTP client (MUST) | domain | HTTP client (MUST) | logo,sync,favicon,simpleicons,clearbit,png | active | - **MUST 复用 `build_http_client_system`** (非 `build_http_clie… |
| domain/trellis-10.md#presets JSON 读取 (MUST) | domain | presets JSON 读取 (MUST) | logo,sync,favicon,simpleicons,clearbit,png | active | - `read_local_presets_json` 优先级: `~/.aidog/platform-presets.… |
| domain/trellis-10.md#三路 fallback 顺序 (MUST, 首成功即止) | domain | 三路 fallback 顺序 (MUST, 首成功即止) | logo,sync,favicon,simpleicons,clearbit,png | active | 固定顺序, **禁重排**, 见 `sync_one_into`:  1. **simpleicons CDN** — … |
| domain/trellis-10.md#入口 | domain | 入口 | logo,sync,favicon,simpleicons,clearbit,png | active | - `sync_all_logos(db, app_data_dir)` — 后台批量同步 (app 启动 / 手动触发… |
| domain/trellis-10.md#关联 | domain | 关联 | logo,sync,favicon,simpleicons,clearbit,png | active | - [http-client-forward.md](./http-client-forward.md) — build… |
| domain/trellis-10.md#缓存契约 (MUST) | domain | 缓存契约 (MUST) | logo,sync,favicon,simpleicons,clearbit,png | active | - 缓存路径 `~/.aidog/logos/<protocol_id>.png` (`logo_cache_path`… |
| domain/trellis-10.md#验收基准 (可复用) | domain | 验收基准 (可复用) | logo,sync,favicon,simpleicons,clearbit,png | active | - [ ] 清空 `~/.aidog/logos/` 后, 有 `logo_url` 的 protocol 命中路 1;… |
| domain/trellis-10.md#验证命令 | domain | 验证命令 | logo,sync,favicon,simpleicons,clearbit,png | active | ```bash # 三路 URL 模板存在且顺序 grep -n "cdn.simpleicons.org\//favi… |
| encoding/trellis-21.md#MUST | encoding | MUST | json,script,application/json,parse,template,embedding,序列化 | active | `<script type="application/json">` 的 textContent 是 **raw tex… |
| encoding/trellis-21.md#MUST NOT | encoding | MUST NOT | json,script,application/json,parse,template,embedding,序列化 | active | - 禁对嵌入 script 的 JSON payload 用任何 HTML 实体转义（`html.escape` / `… |
| encoding/trellis-21.md#Verification | encoding | Verification | json,script,application/json,parse,template,embedding,序列化 | active | ```bash # 抽取嵌入 JSON + 校验可解析 + 无实体 python3 -c " import json, … |
| encoding/trellis-21.md#踩坑来源 | encoding | 踩坑来源 | json,script,application/json,parse,template,embedding,序列化 | active | task `07-07-presets-html-json-escape-fix`：`scripts/presets_v… |
| frontend/auto-fix-downgrade-37.md#MUST 用 Tauri onDragDropEvent，禁 HTML5 onDrop | frontend | MUST 用 Tauri onDragDropEvent，禁 HTML5 onDrop | tauri,drag,drop,wkwebview,html5,ondragdropevent | active | macOS WKWebView 的 HTML5 `drop` 事件不触发。Tauri `getCurrentWebvie… |
| frontend/auto-fix-downgrade-37.md#event.payload.type | frontend | event.payload.type | tauri,drag,drop,wkwebview,html5,ondragdropevent | active | - enter/over: paths[] → 高亮判断 - drop: paths[] → 取目标文件 - leave… |
| frontend/auto-fix-downgrade-37.md#关联 | frontend | 关联 | tauri,drag,drop,wkwebview,html5,ondragdropevent | active / →modal-state-architecture | [[modal-state-architecture]] (Tauri UI 约束) |
| frontend/auto-fix-downgrade-37.md#约束 | frontend | 约束 | tauri,drag,drop,wkwebview,html5,ondragdropevent | active | - 禁混 HTML5 onDrop（macOS WKWebView 不触发） - MUST unlisten（clean… |
| frontend/auto-fix-downgrade-37.md#范本 | frontend | 范本 | tauri,drag,drop,wkwebview,html5,ondragdropevent | active | ```typescript useEffect(() => {   let unlisten: (() => void)… |
| frontend/auto-fix-downgrade-37.md#触发场景 | frontend | 触发场景 | tauri,drag,drop,wkwebview,html5,ondragdropevent | active | Tauri 前端实现文件拖拽导入时。 |
| frontend/auto-fix-downgrade-37.md#适用 | frontend | 适用 | tauri,drag,drop,wkwebview,html5,ondragdropevent | active | Tauri 文件拖拽导入、跨平台拖拽 |
| frontend/cpa-drag-import-22.md#WKWebView 退化（best-effort） | frontend | WKWebView 退化（best-effort） | authdir,dragtarget,ondragenter,wkwebview,best-effort,退化,DOM target | active | macOS WKWebView HTML5 `drop` 不触发，`onDragEnter` **可能同病**（未实测）… |
| frontend/cpa-drag-import-22.md#关联 | frontend | 关联 | authdir,dragtarget,ondragenter,wkwebview,best-effort,退化,DOM target | active | - core/frontend/tauri-drag-drop-api.md（依赖） |
| frontend/cpa-drag-import-22.md#模式: HTML5 onDragEnter 标记 + Tauri drop 读 ref | frontend | 模式: HTML5 onDragEnter 标记 + Tauri drop 读 ref | authdir,dragtarget,ondragenter,wkwebview,best-effort,退化,DOM target | active | ```typescript const dragTargetRef = useRef<"source" / "authd… |
| frontend/cpa-drag-import-22.md#问题: Tauri onDragDropEvent 无 DOM target | frontend | 问题: Tauri onDragDropEvent 无 DOM target | authdir,dragtarget,ondragenter,wkwebview,best-effort,退化,DOM target | active | `onDragDropEvent` 是 webview 级事件，payload **不含 DOM target 信息**… |
| frontend/cpa-drag-import-23.md#模式: baseIdx 全局偏移（orderLenRef） | frontend | 模式: baseIdx 全局偏移（orderLenRef） | rowid,unique,多源,import,baseidx,偏移,batch,react key | active | ```typescript const orderLenRef = useRef(0);  const parseAnd… |
| frontend/cpa-drag-import-23.md#清理 | frontend | 清理 | rowid,unique,多源,import,baseidx,偏移,batch,react key | active | modal 关闭重置 `orderLenRef.current = 0`，下次打开从 0 起。 |
| frontend/cpa-drag-import-23.md#问题: 跨源 rowId 撞 id | frontend | 问题: 跨源 rowId 撞 id | rowid,unique,多源,import,baseidx,偏移,batch,react key | active | 每源 rowId 从 `${0}::` 起递增，不同源同索引条目撞 id。 |
| frontend/cpa-drag-import-23.md#验收 | frontend | 验收 | rowid,unique,多源,import,baseidx,偏移,batch,react key | active | - [ ] 多源 drop → 所有条目 rowId 唯一 - [ ] modal 重开 → orderLenRef 清… |
| frontend/cpa-drag-import-24.md#模式: useRef 计数（parseInFlightRef） | frontend | 模式: useRef 计数（parseInFlightRef） | parseinflight,concurrent,多源,异步,ref,计数,loading,boolean | active | ```typescript const parseInFlightRef = useRef(0);  const par… |
| frontend/cpa-drag-import-24.md#清理 | frontend | 清理 | parseinflight,concurrent,多源,异步,ref,计数,loading,boolean | active | modal 关闭 `parseInFlightRef.current = 0; setParsing(false)`。 |
| frontend/cpa-drag-import-24.md#问题: boolean 无法表达「任一在解析」 | frontend | 问题: boolean 无法表达「任一在解析」 | parseinflight,concurrent,多源,异步,ref,计数,loading,boolean | active | 源 A 完成设 false，源 B 还在跑但 UI 已显示非解析态。互斥锁过重（JS 单线程无需真锁）。 |
| frontend/cpa-drag-import-24.md#验收 | frontend | 验收 | parseinflight,concurrent,多源,异步,ref,计数,loading,boolean | active | - [ ] 快速拖 N 源 → parsing 恒 true 直到全完 - [ ] 某源失败 → 其他继续，最后完成才 … |
| frontend/modal-state-architecture.md#两类 Modal 区分 | frontend | 两类 Modal 区分 | modal, state, architecture, PlatformEditForm, usePlatformForm, PlatformPasteCtx, CpaImportModal, SmartPasteModal | active | ### 直接灌表单 Modal（SmartPasteModal 模式） - **State 位置**: `usePlat… |
| frontend/modal-state-architecture.md#后续新 Modal 决策树 | frontend | 后续新 Modal 决策树 | modal, state, architecture, PlatformEditForm, usePlatformForm, PlatformPasteCtx, CpaImportModal, SmartPasteModal | active | ``` 新 Modal (如 Sub2Api) ├─ onApply 直接填表单字段？ │  └─ 是 → SmartP… |
| frontend/modal-state-architecture.md#架构原则 | frontend | 架构原则 | modal, state, architecture, PlatformEditForm, usePlatformForm, PlatformPasteCtx, CpaImportModal, SmartPasteModal | active | 1. **Modal 直接操作表单字段 → state 放 hook，通过 PlatformPasteCtx 传 set… |
| frontend/modal-state-architecture.md#验收 | frontend | 验收 | modal, state, architecture, PlatformEditForm, usePlatformForm, PlatformPasteCtx, CpaImportModal, SmartPasteModal | active | - [ ] grep `showCpaImport` / `showPaste` 在 PlatformEditForm … |
| frontend/shadcn-infra-30.md#关联 | frontend | 关联 | css,var,alias,live-resolution,migration | active / →shadcn-infra-02 | [[shadcn-infra-02]] (同任务 Tailwind 约束) |
| frontend/shadcn-infra-30.md#对比 | frontend | 对比 | css,var,alias,live-resolution,migration | active | / 方式 / 改动量 / 误伤风险 / 回滚 / /------/--------/---------/------/ … |
| frontend/shadcn-infra-30.md#技巧 | frontend | 技巧 | css,var,alias,live-resolution,migration | active | CSS 变量改名时，用 :root 定义别名层实现 live resolution，替代批量 sed 替换（零误伤、可回… |
| frontend/shadcn-infra-30.md#案例 | frontend | 案例 | css,var,alias,live-resolution,migration | active | - shadcn-infra task: 主题变量改名用别名层，globals.css 加 10 行 vs sed 70… |
| frontend/shadcn-infra-30.md#正解 | frontend | 正解 | css,var,alias,live-resolution,migration | active | 1. 在 :root 定义别名：`--legacy: var(--shadcn);` 2. 所有引用用旧名 `--leg… |
| frontend/shadcn-infra-30.md#适用 | frontend | 适用 | css,var,alias,live-resolution,migration | active | CSS 变量迁移、主题重构、大型 CSS 重构中间状态 |
| frontend/shadcn-infra-31.md#关联 | frontend | 关联 | shadcn,theme,token,runtime,css,var | active / →shadcn-infra-28,shadcn-infra-30 | [[shadcn-infra-30]] (同任务 CSS 技巧) [[shadcn-infra-28]] (shadcn… |
| frontend/shadcn-infra-31.md#反例 | frontend | 反例 | shadcn,theme,token,runtime,css,var | active | ❌ 用 !important 覆盖所有 token → 优先级混乱 ❌ 依赖静态 @import → 运行时无法切换 |
| frontend/shadcn-infra-31.md#技巧 | frontend | 技巧 | shadcn,theme,token,runtime,css,var | active | shadcn 主题 token 在运行时动态切换时，用 `applyTheme` + `setProperty` inl… |
| frontend/shadcn-infra-31.md#案例 | frontend | 案例 | shadcn,theme,token,runtime,css,var | active | - shadcn-infra task: 运行时主题切换用 setProperty inline，避免 !importa… |
| frontend/shadcn-infra-31.md#正解 | frontend | 正解 | shadcn,theme,token,runtime,css,var | active | 1. applyTheme 函数直接设置 CSS var：    ```ts    document.documentE… |
| frontend/shadcn-infra-31.md#适用 | frontend | 适用 | shadcn,theme,token,runtime,css,var | active | shadcn 主题运行时切换、动态主题系统、CSS var 运行时更新 |
| frontend/shadcn-infra-31.md#陷阱 | frontend | 陷阱 | shadcn,theme,token,runtime,css,var | active | - **陷阱**: 用 !important 强制覆盖 → 级联爆炸、难以维护 - **陷阱**: 依赖 @import… |
| frontend/trellis-18.md#API Layer (MUST) | frontend | API Layer (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | active | > 违反代价: invoke 散落各文件 / 静默丢错 → 后端 command 改名时编译期不报、运行时静默失败难排查… |
| frontend/trellis-18.md#CRUD 刷新链契约 (MUST) | frontend | CRUD 刷新链契约 (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | active | > 违反代价: 后端真删/真改的 CRUD 入口（如 `platformApi.delete`）仅刷关联 state（g… |
| frontend/trellis-18.md#Component Patterns (MUST) | frontend | Component Patterns (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | active | > 违反代价: 引入 CSS Modules / CSS-in-JS → 样式系统割裂、主题切换失效；index 作 k… |
| frontend/trellis-18.md#Deep-Link 导入契约 (MUST) | frontend | Deep-Link 导入契约 (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | active | > 违反代价: 缓存重放 → 用户重访页面时旧导入弹窗反复弹；URL 承载格式与接收端解析不匹配 → 唤起后导入静默失败… |
| frontend/trellis-18.md#Directory Structure (MUST) | frontend | Directory Structure (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | active | > 违反代价: 文件放错层 → 后续 agent 按约定 grep 找不到 → 重复造同名文件 / import 路径混… |
| frontend/trellis-18.md#Hooks (MUST) | frontend | Hooks (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | active | > 违反代价: 不用 `use` 前缀 → React lint 规则失效、依赖检查漏报；≥2 组件复用却不提取 → 逻… |
| frontend/trellis-18.md#Large File Split — facade 模式 (MUST) | frontend | Large File Split — facade 模式 (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | active | > 违反代价: 巨石文件 (>800 行) → 增量改动成本指数增长、merge 冲突频发、agent 上下文爆炸；拆分… |
| frontend/trellis-18.md#State Management (MUST) | frontend | State Management (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | active | > 违反代价: 新建 store / 绕过 AppContext 读写 localStorage → 状态双源不一致、持… |
| frontend/trellis-18.md#Type Safety (MUST) | frontend | Type Safety (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | active | > 违反代价: 用 `any` / `string` 代替 union → 后端字段改动编译期不报错、运行时崩；漏同步 … |
| frontend/trellis-18.md#i18n (MUST) | frontend | i18n (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | active | - 所有用户可见文案必须用 `t("key")`，禁硬编码中/英文字面量（含 placeholder / title /… |
| git/rule-44.md#关联 | git | 关联 | git,并行,subtask,commit,竞态,staged,worktree | active | git-worktree-parallel-isolation |
| git/rule-44.md#处理流程 | git | 处理流程 | git,并行,subtask,commit,竞态,staged,worktree | active | ```bash # commit 前检查 staged 文件 git diff --cached --name-only… |
| git/rule-44.md#案例 | git | 案例 | git,并行,subtask,commit,竞态,staged,worktree | active | - shadcn-pages task 并行 m-groups/m-logs/m-stats 等子任务，需 commit… |
| git/rule-44.md#触发场景 | git | 触发场景 | git,并行,subtask,commit,竞态,staged,worktree | active | 同一 worktree 并行跑多个 subtask 时，不同 agent 可能对同一文件产生变更，导致 git inde… |
| git/rule-44.md#适用 | git | 适用 | git,并行,subtask,commit,竞态,staged,worktree | active | - 同 worktree 并行 subtask（skein parallel 模式） - 多 agent 同时改同一文件… |
| git/rule-44.md#陷阱-正解 | git | 陷阱-正解 | git,并行,subtask,commit,竞态,staged,worktree | active | ❌ **陷阱**：多个并行 subtask 各自 commit，兄弟 staged 文件可能被误入彼此的 commit（… |
| i18n/trellis-19.md#RTL | i18n | RTL | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | active | `ar-SA` 是唯一 RTL locale (`RTL_LOCALES`, `index.ts:28`); `isRT… |
| i18n/trellis-19.md#三层一致 (MUST) | i18n | 三层一致 (MUST) | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | active | 应用 i18n locale 标签跨三层必须**字面同一集合**:  1. **i18next** (`src/loca… |
| i18n/trellis-19.md#关联 | i18n | 关联 | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | active | - [backend/index.md](../backend/index.md) — presets JSON (后端… |
| i18n/trellis-19.md#多 locale 命名空间共存, 禁统一 (MUST NOT) | i18n | 多 locale 命名空间共存, 禁统一 (MUST NOT) | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | active | 应用内存在 **4 套独立 locale 命名空间**, 各服务不同消费者, 标签约定不同是有意设计, **禁强行统一*… |
| i18n/trellis-19.md#应用 i18n locale 标签 = BCP 47 script 子标签 (MUST) | i18n | 应用 i18n locale 标签 = BCP 47 script 子标签 (MUST) | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | active | - **MUST `zh-Hans`** (script 子标签), **禁 `zh-CN`** (region 子标签… |
| i18n/trellis-19.md#持久化迁移 (MUST, 单向) | i18n | 持久化迁移 (MUST, 单向) | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | active | - `src/context/AppContext.tsx:98` 启动读用户设置时: `raw.locale === … |
| i18n/trellis-19.md#测试 fixture / 文档 URL (合法残留, 非命名空间) | i18n | 测试 fixture / 文档 URL (合法残留, 非命名空间) | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | active | - 测试用 `zh-CN` fixture (`test_sync_settings.rs` / `test_apply… |
| i18n/trellis-19.md#验收基准 (可复用) | i18n | 验收基准 (可复用) | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | active | - [ ] `ALL_LOCALES` 集合 == presets JSON 任一 protocol 的 `name` … |
| i18n/trellis-19.md#验证命令 | i18n | 验证命令 | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | active | ```bash # zh-CN 残留审计 (合法点见上 "测试 fixture / 文档 URL" + 4 命名空间表;… |
| ops/trellis-17.md#Cross-reference | ops | Cross-reference | sync,defaults,json,jsdelivr,remote,validate,presets,hash | active | - 先例代码: `crates/aidog_core/src/gateway/defaults_sync.rs`（pla… |
| ops/trellis-17.md#实例 | ops | 实例 | sync,defaults,json,jsdelivr,remote,validate,presets,hash | active | - task 07-09-*（platform-presets 同步首次落地，`defaults_sync.rs` 先例… |
| ops/trellis-17.md#数据流架构 (MUST，禁前端直读 github) | ops | 数据流架构 (MUST，禁前端直读 github) | sync,defaults,json,jsdelivr,remote,validate,presets,hash | active | ``` github (master) ──rust sync (<x>_sync.rs)──▶ ~/.aidog/<f… |
| ops/trellis-17.md#范式 (MUST，照抄先例 `gateway/defaults_sync.rs`) | ops | 范式 (MUST，照抄先例 `gateway/defaults_sync.rs`) | sync,defaults,json,jsdelivr,remote,validate,presets,hash | active | `defaults/*.json` 远端同步**MUST** 实现完整 7 件套，缺一致命。先例 `crates/aid… |
| ops/trellis-17.md#验收断言（可复用） | ops | 验收断言（可复用） | sync,defaults,json,jsdelivr,remote,validate,presets,hash | active | ```bash # 7 件套齐全（双源 / last_updated / 24h / 三路触发 / schema gat… |
| optimization/api-payload-optimization.md#后端 DISTINCT 替代前端集合去重降低 IPC payload | optimization | 后端 DISTINCT 替代前端集合去重降低 IPC payload | api,payload,ipc,distinct,set-deduplication,query-optimization | active | 后端改为返回去重后的单列（如 DISTINCT model），而非拉全字段摘要行数组到前端，再用集合去重。  **收益*… |
| optimization/manual-budget-empty-shortcircuit.md#manual_budget 零配额短路：进写连接前预检 | optimization | manual_budget 零配额短路：进写连接前预检 | manual-budget,optimization,db-write,shortcircuit,loadgen | active | - |
| optimization/manual-budget-empty-shortcircuit.md#关键点 | optimization | 关键点 | manual-budget,optimization,db-write,shortcircuit,loadgen | active | - **硬约束**：配额存在时行为不变，短路仅对「零配额」分支生效 - **非 mock 专属**：真实转发路径共用同一… |
| optimization/manual-budget-empty-shortcircuit.md#方案 | optimization | 方案 | manual-budget,optimization,db-write,shortcircuit,loadgen | active | **分两阶段：**  1. **只读池预检**（`has_any_budget`，line:189-203）：用只读池（… |
| optimization/manual-budget-empty-shortcircuit.md#用途 | optimization | 用途 | manual-budget,optimization,db-write,shortcircuit,loadgen | active | 高频转发路径的每请求冷路径优化，减少单线程 DB 写锁争。适用于： - mock/真实平台混用的压测 - 用户未配额时的… |
| optimization/manual-budget-empty-shortcircuit.md#问题 | optimization | 问题 | manual-budget,optimization,db-write,shortcircuit,loadgen | active | `apply_manual_budgets`（`manual_budget.rs:211-246`）处理用户手动配额时，… |
| proxy/rule-50.md#关联 | proxy | 关联 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | active / →trellis-00,trellis-11 | [[trellis-11]] （proxy 统计不污染） · [[trellis-00]] （DB 表设计） |
| proxy/rule-50.md#反例 / 常见错误 | proxy | 反例 / 常见错误 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | active | / 错误                          / 为什么错                        … |
| proxy/rule-50.md#案例 | proxy | 案例 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | active | - log-async-write task (commit 529e571b) — proxy_log 改为单 wri… |
| proxy/rule-50.md#正解：方案 B（单 writer + 有界 queue + 分级背压 + 串行快照） | proxy | 正解：方案 B（单 writer + 有界 queue + 分级背压 + 串行快照） | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | active | ### 架构骨架 ``` 热路径 (request handler)       后台 writer task ────… |
| proxy/rule-50.md#落库路径升级 checklist | proxy | 落库路径升级 checklist | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | active | ```rust // 新增高频异步操作时参考此模式： // 1. 定义枚举消息类型 pub(crate) enum Yo… |
| proxy/rule-50.md#触发场景 | proxy | 触发场景 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | active | 高频热路径中需要异步写入数据库（如 proxy_log upsert），不能阻塞请求处理；需要保证最终结果不丢且落库顺序… |
| proxy/rule-50.md#适用 | proxy | 适用 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | active | - proxy_log 异步写入（已实现 s1） - 其他高频日志 / 统计 / 聚合表的异步更新（future 可参考… |
| proxy/rule-50.md#陷阱：同步写会阻塞热路径 + 异步不保证持久性 | proxy | 陷阱：同步写会阻塞热路径 + 异步不保证持久性 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | active | > proxy_log 原先热路径内同步调 `upsert_log(db).await` → 所有请求必须等 DB 写入… |
| proxy/rule-50.md#验证 | proxy | 验证 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | active | ```bash # 背压分级（中间态 try_send vs 终态 send） cd src-tauri && grep… |
| proxy/trellis-11.md#CONNECT target 多源解析 (MUST) | proxy | CONNECT target 多源解析 (MUST) | proxy,connect,tunnel,axum,hyper,TcpStream | active | > 违反代价: `req.uri().path()` 对 authority-form URI 返空 → `target… |
| proxy/trellis-11.md#CONNECT 路由契约 (MUST) | proxy | CONNECT 路由契约 (MUST) | proxy,connect,tunnel,axum,hyper,TcpStream | active | > 违反代价: `.route()` 注册 CONNECT → authority-form URI `host:por… |
| proxy/trellis-11.md#MITM CA 信任库安装 (MUST — 三 OS 原生提权) | proxy | MITM CA 信任库安装 (MUST — 三 OS 原生提权) | proxy,connect,tunnel,axum,hyper,TcpStream | active | > 违反代价: 假 CA 装不进系统信任库 → 客户端不信任 AirDog 签的 host 证书 → MITM 解密全挂… |
| proxy/trellis-11.md#TCP 双向隧道 (MUST) | proxy | TCP 双向隧道 (MUST) | proxy,connect,tunnel,axum,hyper,TcpStream | active | - `tokio::io::copy` 双向 + `tokio::join!` 同时转发两向 - 字节 u64 返回值:… |
| proxy/trellis-11.md#hyper-util upgrade downcast 类型 (MUST) | proxy | hyper-util upgrade downcast 类型 (MUST) | proxy,connect,tunnel,axum,hyper,TcpStream | active | > 违反代价: downcast 类型错 → 取不到底层流 → 隧道空转 / panic。research 说 `dow… |
| proxy/trellis-11.md#proxy_log 写入契约 (MUST — 不污染 stats_agg) | proxy | proxy_log 写入契约 (MUST — 不污染 stats_agg) | proxy,connect,tunnel,axum,hyper,TcpStream | active | > 违反代价: CONNECT 流量走 `upsert_log` → 触发 `upsert_stats_agg` + `… |
| proxy/trellis-11.md#前端筛选 sentinel (MUST) | proxy | 前端筛选 sentinel (MUST) | proxy,connect,tunnel,axum,hyper,TcpStream | active | - Logs/Stats 平台筛选「无平台」: value `"0"` → `Number("0")=0` → `pla… |
| proxy/trellis-11.md#平台 host 匹配 (MUST) | proxy | 平台 host 匹配 (MUST) | proxy,connect,tunnel,axum,hyper,TcpStream | active | - `match_platform_by_host` (新增, `endpoint.rs`) — CONNECT tar… |
| proxy/trellis-11.md#验证 | proxy | 验证 | proxy,connect,tunnel,axum,hyper,TcpStream | active | ```bash # CONNECT 分流 early return, 非 CONNECT 原 fallthrough g… |
| proxy/trellis-12.md#host self 判定分支 (复用, 不变) | proxy | host self 判定分支 (复用, 不变) | proxy,fallback,host,route,mitm,path | active | loopback 名 (`localhost`/`127.0.0.1`/`0.0.0.0`) + listen ip 字… |
| proxy/trellis-12.md#关联 | proxy | 关联 | proxy,fallback,host,route,mitm,path | active | - CONNECT 隧道 / relay 层: [proxy-connect-relay.md](proxy-conne… |
| proxy/trellis-12.md#核心契约 (MUST) | proxy | 核心契约 (MUST) | proxy,fallback,host,route,mitm,path | active | - **`should_fallback_passthrough` host 判定 MUST 前置于 path/is_a… |
| proxy/trellis-12.md#验收基准 (复用断言) | proxy | 验收基准 (复用断言) | proxy,fallback,host,route,mitm,path | active | - MITM 灌入: host=`open.bigmodel.cn` + path=`/api/anthropic/v1… |
| proxy/trellis-13.md#absolute-form URI 路由契约 (MUST) | proxy | absolute-form URI 路由契约 (MUST) | proxy,forward,absolute,scheme,relay,host | active | > 违反代价: axum 按 `Request::uri().path()` 匹配路由，absolute-form `G… |
| proxy/trellis-13.md#forward URL scheme 自适应 (MUST) | proxy | forward URL scheme 自适应 (MUST) | proxy,forward,absolute,scheme,relay,host | active | > 违反代价: `forward_passthrough_to_orig_host` 硬编码 `https://{hos… |
| proxy/trellis-13.md#proxy_log 落虚拟桶 (MUST — 与 MITM fallback 同语义) | proxy | proxy_log 落虚拟桶 (MUST — 与 MITM fallback 同语义) | proxy,forward,absolute,scheme,relay,host | active | > 违反代价: forward 流量走独立 upsert 路径 / 单独统计 → 与 MITM 解密 fallback … |
| proxy/trellis-13.md#跨层 / 关联 spec | proxy | 跨层 / 关联 spec | proxy,forward,absolute,scheme,relay,host | active | - [Proxy CONNECT Relay](./proxy-connect-relay.md) — CONNECT … |
| proxy/trellis-13.md#路由层契约 (MUST) | proxy | 路由层契约 (MUST) | proxy,forward,absolute,scheme,relay,host | active | - **`build_router(state: Arc<ProxyState>) -> Router`** — Rou… |
| proxy/trellis-13.md#验证 | proxy | 验证 | proxy,forward,absolute,scheme,relay,host | active | ```bash # absolute-form middleware 存在 + 路由顶层包装 grep -n "abso… |
| proxy/trellis-14.md#为何 502 路径不触发 / 200 路径触发 | proxy | 为何 502 路径不触发 / 200 路径触发 | reqwest,no_proxy,http_client,forward,env,递归 | active | - **502 路径** (上游 `nonexistent.invalid`): reqwest 走 env proxy… |
| proxy/trellis-14.md#禁 env proxy 契约 (MUST) | proxy | 禁 env proxy 契约 (MUST) | reqwest,no_proxy,http_client,forward,env,递归 | active | > 违反代价: AirDog 自身是代理 (监听 :9892), 转发上游时若 reqwest 读 `HTTPS_PRO… |
| proxy/trellis-14.md#验证 | proxy | 验证 | reqwest,no_proxy,http_client,forward,env,递归 | active | ```bash # use_proxy=false 分支有 .no_proxy() grep -n "no_proxy"… |
| proxy/trellis-15.md#Helper 复用契约 (MUST) | proxy | Helper 复用契约 (MUST) | proxy,header,diagnostic,trace,blind_relay,debug | active | > 违反代价: 各响应构造点重复实现 `cfg!(debug_assertions)` gate, 新加诊断 heade… |
| proxy/trellis-15.md#blind_relay 物理豁免 (MUST NOT) | proxy | blind_relay 物理豁免 (MUST NOT) | proxy,header,diagnostic,trace,blind_relay,debug | active | > 违反代价: blind_relay 是 CONNECT 隧道建好后 TCP 字节透传, AirDog 看见的是加密 … |
| proxy/trellis-15.md#header 名规范 (MUST) | proxy | header 名规范 (MUST) | proxy,header,diagnostic,trace,blind_relay,debug | active | - **header 名 MUST 小写** (`x-aidog-trace` 等), 用 `HeaderName::f… |
| proxy/trellis-15.md#id 取值链 (MUST) | proxy | id 取值链 (MUST) | proxy,header,diagnostic,trace,blind_relay,debug | active | > 违反代价: 各处自造 id 失去与 proxy_log / span 的关联, 诊断时无法客户端报错 → AirDo… |
| proxy/trellis-15.md#release build 行为 (MUST) | proxy | release build 行为 (MUST) | proxy,header,diagnostic,trace,blind_relay,debug | active | - **release build MUST 不注入** —— helper 内 `if cfg!(debug_asse… |
| proxy/trellis-15.md#跨协议注入选址参考 | proxy | 跨协议注入选址参考 | proxy,header,diagnostic,trace,blind_relay,debug | active | `07-05-proxy-trace-id-header` 实施时枚举的 47 调用点分布: - `handler.rs… |
| proxy/trellis-15.md#验收基准 (可复用) | proxy | 验收基准 (可复用) | proxy,header,diagnostic,trace,blind_relay,debug | active | - [ ] debug build: 所有 AirDog **直构**响应含诊断 header (grep `injec… |
| proxy/trellis-15.md#验证命令 | proxy | 验证命令 | proxy,header,diagnostic,trace,blind_relay,debug | active | ```bash # helper 调用计数 (debug 注入点) grep -rn "inject_trace_hea… |
| reuse/auto-fix-downgrade-36.md#Abstract Threshold | reuse | Abstract Threshold | grep,reuse,复用,组件,utility,抽象,dry | active | - ≥ 3 处相同逻辑 → 必须 abstract - 2 处相同逻辑 → 必须 grep 确认，commit mess… |
| reuse/auto-fix-downgrade-36.md#MUST | reuse | MUST | grep,reuse,复用,组件,utility,抽象,dry | active | - 写新函数前必须 `grep -rE '<关键词>' src/` 查已有实现；命中则复用，禁重写 - 新增平台协议必须… |
| reuse/auto-fix-downgrade-36.md#MUST NOT | reuse | MUST NOT | grep,reuse,复用,组件,utility,抽象,dry | active | - 禁止为新页面复制已有页面的 CRUD 模板代码而不提取公共组件 - 禁止定义与 `api.ts` 中已有 names… |
| reuse/auto-fix-downgrade-36.md#关联 | reuse | 关联 | grep,reuse,复用,组件,utility,抽象,dry | active / →shadcn-infra-28 | [[shadcn-infra-28]] (shadcn 依赖复用) |
| reuse/auto-fix-downgrade-36.md#触发场景 | reuse | 触发场景 | grep,reuse,复用,组件,utility,抽象,dry | active | 写新函数 / 新组件 / 新 utility 前。 |
| reuse/auto-fix-downgrade-36.md#适用 | reuse | 适用 | grep,reuse,复用,组件,utility,抽象,dry | active | 写新代码前查复用、防止重复实现 |
| build/shadcn/shadcn-primitives-39.md#关联 | shadcn | 关联 | shadcn,add,dependencies,yarn,tailwind,verification | active / →shadcn-infra-02 | [[shadcn-infra-02]] |
| build/shadcn/shadcn-primitives-39.md#正解 | shadcn | 正解 | shadcn,add,dependencies,yarn,tailwind,verification | active | add 后必 grep package.json 验证依赖在，缺则 `yarn add <pkg>` 补。 |
| build/shadcn/shadcn-primitives-39.md#规则 | shadcn | 规则 | shadcn,add,dependencies,yarn,tailwind,verification | active | 不预设必漏也不预设必装，每次 add 后验证。 |
| build/shadcn/shadcn-primitives-39.md#证据 | shadcn | 证据 | shadcn,add,dependencies,yarn,tailwind,verification | active | commit 2b79767a "补 class-variance-authority 依赖 (shadcn add 漏… |
| build/shadcn/shadcn-primitives-39.md#适用 | shadcn | 适用 | shadcn,add,dependencies,yarn,tailwind,verification | active | yarn 4+ + tailwind 4 + shadcn add 操作 |
| build/shadcn/shadcn-primitives-39.md#问题 | shadcn | 问题 | shadcn,add,dependencies,yarn,tailwind,verification | active | shadcn add 在 yarn4+tailwind4 下 "Installing dependencies" 阶段不… |
| shadcn/rule-03.md#MUST 硬约束 | shadcn | MUST 硬约束 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | active | Radix Dialog **必须包含 DialogTitle**，否则会触发 a11y 警告。 |
| shadcn/rule-03.md#关联 | shadcn | 关联 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | active / →rule-43 | [[rule-43]] |
| shadcn/rule-03.md#实现模式 | shadcn | 实现模式 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | active | ❌ **陷阱**：自定义 header 时完全省略 DialogTitle，破坏 a11y。 ✅ **正解**：用 `s… |
| shadcn/rule-03.md#案例 | shadcn | 案例 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | active | - `src/components/settings/editors/StatusLineSection/Segment… |
| shadcn/rule-03.md#模式模板 | shadcn | 模式模板 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | active | ```tsx import { Dialog, DialogContent, DialogTitle } from "@… |
| shadcn/rule-03.md#触发场景 | shadcn | 触发场景 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | active | 使用 Radix Dialog 组件时，必须满足无障碍（a11y）要求。 |
| shadcn/rule-03.md#适用 | shadcn | 适用 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | active | - 所有 Radix Dialog 用法（@/components/ui/dialog） - 需要完全自定义 heade… |
| shadcn/rule-41.md#关联 | shadcn | 关联 | radix,Select,空值,哨兵,__none__ | active / →rule-42 | [[rule-42]] |
| shadcn/rule-41.md#案例 | shadcn | 案例 | radix,Select,空值,哨兵,__none__ | active | - `src/pages/Logs/primitives.tsx:12-13` 定义 NONE 常量 + 注释说明 - … |
| shadcn/rule-41.md#模式模板 | shadcn | 模式模板 | radix,Select,空值,哨兵,__none__ | active | ```tsx // 定义哨兵常量 const NONE = "__none__";  // 组件使用 <Select  … |
| shadcn/rule-41.md#触发场景 | shadcn | 触发场景 | radix,Select,空值,哨兵,__none__ | active | 使用 radix Select 组件时，value 属性需要处理空值/undefined 状态。 |
| shadcn/rule-41.md#适用 | shadcn | 适用 | radix,Select,空值,哨兵,__none__ | active | - radix Select 组件（@/components/ui/select） - 需要空值占位符的下拉选择场景 |
| shadcn/rule-41.md#陷阱-正解 | shadcn | 陷阱-正解 | radix,Select,空值,哨兵,__none__ | active | ❌ **陷阱**：直接使用 `value=""` 会触发 radix Select 内部验证错误（SelectItem … |
| shadcn/rule-42.md#关联 | shadcn | 关联 | radix,Select,number,String,Number,双向映射 | active / →rule-41 | [[rule-41]] |
| shadcn/rule-42.md#案例 | shadcn | 案例 | radix,Select,number,String,Number,双向映射 | active | - `src/pages/Logs/primitives.tsx:374` Pagination pageSize: `… |
| shadcn/rule-42.md#模式模板 | shadcn | 模式模板 | radix,Select,number,String,Number,双向映射 | active | ```tsx <Select   value={String(numberValue)}  // 存储/显示：numbe… |
| shadcn/rule-42.md#触发场景 | shadcn | 触发场景 | radix,Select,number,String,Number,双向映射 | active | radix Select 的 value 属性只接受 string 类型，需要处理 number 类型数据。 |
| shadcn/rule-42.md#适用 | shadcn | 适用 | radix,Select,number,String,Number,双向映射 | active | - radix Select value 仅收 string（类型约束） - 需要处理 number 选项的分页器/数值… |
| shadcn/rule-42.md#陷阱-正解 | shadcn | 陷阱-正解 | radix,Select,number,String,Number,双向映射 | active | ❌ **陷阱**：直接传 number 会触发类型错误或运行时异常。 ✅ **正解**双向映射：存储/显示时 Strin… |
| shadcn/rule-43.md#关联 | shadcn | 关联 | Dialog,open,null,Promise,resolve,bool | active / →rule-41 | [[rule-41]] |
| shadcn/rule-43.md#案例 | shadcn | 案例 | Dialog,open,null,Promise,resolve,bool | active | - 通用模式：shadcn-pages 迁移中所有 Dialog 均用 `open={state !== null}` |
| shadcn/rule-43.md#模式模板 | shadcn | 模式模板 | Dialog,open,null,Promise,resolve,bool | active | ```tsx const [modalState, setModalState] = useState<{resolve… |
| shadcn/rule-43.md#触发场景 | shadcn | 触发场景 | Dialog,open,null,Promise,resolve,bool | active | Dialog.open 属性需要 bool 类型，但实际控制常来自 Promise resolve 型 state（如 … |
| shadcn/rule-43.md#适用 | shadcn | 适用 | Dialog,open,null,Promise,resolve,bool | active | - 任何 Promise resolve 型 state 控制弹窗开关的场景（如 async confirm/自定义 M… |
| shadcn/rule-43.md#陷阱-正解 | shadcn | 陷阱-正解 | Dialog,open,null,Promise,resolve,bool | active | ❌ **陷阱**：直接用 `open={modalState}` 会将 null/对象转为 bool，无法正确反映「有 … |
| shadcn/rule-45.md#关联 | shadcn | 关联 | popover,只读,shadcn,迁移,预筛,grep | active / →rule-41 | [[rule-41]] |
| shadcn/rule-45.md#案例 | shadcn | 案例 | popover,只读,shadcn,迁移,预筛,grep | active | - shadcn-pages task：PopoverConfigTab 经 grep 命中 0，确认无需迁移 |
| shadcn/rule-45.md#触发场景 | shadcn | 触发场景 | popover,只读,shadcn,迁移,预筛,grep | active | popover 独立窗口（TrayConfigTab）是只读展示域，无表单控件，不适用通用 shadcn 迁移模板。 |
| shadcn/rule-45.md#适用 | shadcn | 适用 | popover,只读,shadcn,迁移,预筛,grep | active | - popover 独立窗口（TrayConfigTab）等只读域 - planning 阶段 shadcn 迁移范围判… |
| shadcn/rule-45.md#陷阱-正解 | shadcn | 陷阱-正解 | popover,只读,shadcn,迁移,预筛,grep | active | ❌ **陷阱**：planning 阶段未预筛，按通用模板对所有页面跑 shadcn 迁移，对只读域产生误判（实际无 b… |
| shadcn/rule-45.md#预筛命令 | shadcn | 预筛命令 | popover,只读,shadcn,迁移,预筛,grep | active | ```bash # 检查目标域是否有可迁组件 grep -c "<button\/<input\/<select\/<t… |
| shadcn/rule-46.md#MUST 硬约束 | shadcn | MUST 硬约束 | shadcn,Button,cva,svg,16px,size-4 | active | shadcn Button 内的 svg 图标会被强制压至 16px（`size-4` = 1rem = 16px），自… |
| shadcn/rule-46.md#关联 | shadcn | 关联 | shadcn,Button,cva,svg,16px,size-4 | active / →rule-43 | [[rule-43]] |
| shadcn/rule-46.md#实现模式 | shadcn | 实现模式 | shadcn,Button,cva,svg,16px,size-4 | active | ```tsx // Button cva 基类（shadcn/ui/button.tsx） variants: {   … |
| shadcn/rule-46.md#案例 | shadcn | 案例 | shadcn,Button,cva,svg,16px,size-4 | active | - shadcn-pages task：Sidebar nav icon 迁移至 Button，接受 16px 默认 |
| shadcn/rule-46.md#触发场景 | shadcn | 触发场景 | shadcn,Button,cva,svg,16px,size-4 | active | shadcn Button 组件 cva 基类含 `[&_svg]:size-4` 规则，统一压内部 svg 至 16p… |
| shadcn/rule-46.md#适用 | shadcn | 适用 | shadcn,Button,cva,svg,16px,size-4 | active | - 所有 shadcn Button 用法（@/components/ui/button） - nav icon 等小图… |
| shadcn/rule-47.md#关联 | shadcn | 关联 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | active / →rule-41 | [[rule-41]] |
| shadcn/rule-47.md#案例 | shadcn | 案例 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | active | - shadcn-pages task：Groups/GroupListItem SortableList 迁移，保留拖… |
| shadcn/rule-47.md#模式模板 | shadcn | 模式模板 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | active | ```tsx // 保留：拖拽逻辑 const { attributes, listeners, setNodeRef,… |
| shadcn/rule-47.md#触发场景 | shadcn | 触发场景 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | active | dnd-kit SortableList 组件迁移时，只需替换内部 button/视觉组件，拖拽逻辑保持不变。 |
| shadcn/rule-47.md#适用 | shadcn | 适用 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | active | - dnd-kit SortableList 迁移至 shadcn - 保留拖拽逻辑仅换视觉的场景 |
| shadcn/rule-47.md#陷阱-正解 | shadcn | 陷阱-正解 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | active | ❌ **陷阱**：重写整个拖拽逻辑，破坏已有行为。 ✅ **正解**：保留 dnd-kit 的 useSortable/… |
| skein/coding-plan-utilization-calib-fix-27.md#task 查重: 同模块非重复, 先看 PRD 边界互引 | skein | task 查重: 同模块非重复, 先看 PRD 边界互引 | skein,dedup,task-boundary,prd | active | dedup/查重判定重叠维度前, MUST 先看两 task 的 PRD 边界条款是否已显式互相引用切割 (如双向标注对… |
| skein/decision-documentation.md#实测推翻设计假设时的处理范式（留痕+不硬凑） | skein | 实测推翻设计假设时的处理范式（留痕+不硬凑） | planning,execution,hypothesis-testing,decision-logging,design-vs-reality | active | 当 task 执行过程中发现「planning 写的验收文本与 exec 实测结果矛盾」时，按以下范式处理：  **模式… |
| style/trellis-16.md#ANSI 着色 (MUST) | style | ANSI 着色 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | active | - **console MUST ANSI on** (`AidogFormat { ansi: true }`), f… |
| style/trellis-16.md#id 双轨映射 (MUST) | style | id 双轨映射 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | active | > 违反代价: proxy 请求 header id 不能反查 proxy_log 行; 或全局统一随机失去诊断关联。 … |
| style/trellis-16.md#id 格式规范 (MUST) | style | id 格式规范 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | active | - **每级 id MUST 6 位 `[0-9a-z]`** (36^6 ≈ 2.2B 空间) - **多级 MUST… |
| style/trellis-16.md#thread-local 栈角色 (MUST) | style | thread-local 栈角色 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | active | - **thread-local `TRACE_ID_STACK` 仅同步业务代码 fallback** (inject… |
| style/trellis-16.md#traceid 取值链 (MUST) | style | traceid 取值链 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | active | > 违反代价: 日志行无 id 可 grep = 诊断 header 设计目的 (header↔日志映射) 失效。  -… |
| style/trellis-16.md#健康端点 span (MUST) | style | 健康端点 span (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | active | > 违反代价: 健康端点无 span → inject_trace_header 兜底现场造孤儿 id, header↔… |
| style/trellis-16.md#异步分支 id 传播 (MUST) | style | 异步分支 id 传播 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | active | > 违反代价: thread-local 栈在 tokio spawn 后失效 (跨线程执行不继承), 子任务内 tra… |
| style/trellis-16.md#日志字段顺序 (MUST) | style | 日志字段顺序 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | active | > 违反代价: 用户诊断时按位置 grep 失败, dev/release 字段顺序不一致需两套解析。  - **MUS… |
| style/trellis-16.md#跨层 / 关联 spec | style | 跨层 / 关联 spec | log,trace,traceid,ansi,format,spawn_traced,span | active | - [Proxy Diagnostic Headers](./proxy-diagnostic-headers.md) … |
| style/trellis-16.md#验收基准 (可复用) | style | 验收基准 (可复用) | log,trace,traceid,ansi,format,spawn_traced,span | active | - [ ] debug build: header `x-aidog-trace` id grep 日志命中 ≥1 行 … |
| style/trellis-16.md#验证命令 | style | 验证命令 | log,trace,traceid,ansi,format,spawn_traced,span | active | ```bash # 格式器装在 console + file 两层 grep -n "AidogFormat\/even… |
| test/rule-48.md#MUST 硬约束 | test | MUST 硬约束 | shadcn,测试,snapshot,行为断言,className | active | 测试改测行为而非 className；shadcn 迁移后 snapshot 应改为行为断言。 |
| test/rule-48.md#关联 | test | 关联 | shadcn,测试,snapshot,行为断言,className | active / →rule-41 | [[rule-41]] |
| test/rule-48.md#案例 | test | 案例 | shadcn,测试,snapshot,行为断言,className | active | - shadcn-pages task：PlatformCard.test.tsx snapshot → 行为断言（删除… |
| test/rule-48.md#触发场景 | test | 触发场景 | shadcn,测试,snapshot,行为断言,className | active | shadcn 迁移导致组件 className/结构变化，现有 snapshot 测试会因视觉差异失败。 |
| test/rule-48.md#迁移模式 | test | 迁移模式 | shadcn,测试,snapshot,行为断言,className | active | ```tsx // ❌ 旧：测试 className（脆弱） expect(screen.getByTestId("ca… |
| test/rule-48.md#适用 | test | 适用 | shadcn,测试,snapshot,行为断言,className | active | - PlatformCard/BalanceBar 等组件测试 - shadcn 迁移导致 className/结构变化… |
| test/rule-65.md#关联 | test | 关联 | test,migration,module,internal,path | active / →rule-60 | [[rule-60]] |
| test/rule-65.md#案例 | test | 案例 | test,migration,module,internal,path | active | - arch-deepen-2 c3-commands batch 3：迁 commands_*::src/test_*… |
| test/rule-65.md#正解 | test | 正解 | test,migration,module,internal,path | active | 将所有 `aidog_core::` 前缀改为 `crate::`（当前 crate 的自引用）： ```rust //… |
| test/rule-65.md#触发场景 | test | 触发场景 | test,migration,module,internal,path | active | 测试代码从外部 crate 迁移进 aidog_core 内部时。 |
| test/rule-65.md#适用 | test | 适用 | test,migration,module,internal,path | active | - 跨 crate 迁移测试文件 - 模块合并时 - 测试代码路径清理 |
| test/rule-65.md#陷阱 | test | 陷阱 | test,migration,module,internal,path | active | 保持原外部 crate 的全限定路径 `aidog_core::xxx::yyy`，但新位置是 aidog_core 内… |
| testing/deterministic-pseudorandom-loadgen.md#关键点 | testing | 关键点 | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | active | - **确定性**：给定 error_rate 的序列完全由进程启动顺序决定，重复压测结果稳定 - **分布均匀**：s… |
| testing/deterministic-pseudorandom-loadgen.md#压测可复现的确定性伪随机（原子计数器+哈希） | testing | 压测可复现的确定性伪随机（原子计数器+哈希） | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | active | - |
| testing/deterministic-pseudorandom-loadgen.md#方案 | testing | 方案 | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | active | **进程级原子计数器 + 乘法哈希** (`proxy/mock.rs:2-16`)：  ```rust static … |
| testing/deterministic-pseudorandom-loadgen.md#用途 | testing | 用途 | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | active | - mock 平台的 error_rate 注入 - 压测场景的确定性故障模拟 - 内存/CPU 基准测试（需要重复压测… |
| testing/deterministic-pseudorandom-loadgen.md#问题 | testing | 问题 | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | active | 压测场景（尤其是性能/内存压测）需要可复现的伪随机行为，用于注入 `error_rate=0.05`（5% 请求返回 4… |
| frontend/theme/shadcn-primitives-40.md#关联 | theme | 关联 | next-themes,theme,conflict,shadcn,sonner | active / →modal-state-architecture | [[modal-state-architecture]] (同 task Modal 保留策略) |
| frontend/theme/shadcn-primitives-40.md#待决策 | theme | 待决策 | next-themes,theme,conflict,shadcn,sonner | active | - 留待 pages 层评估：是否切换到 next-themes 统一，或隔离 Sonner 主题逻辑 - 当前：保留冲… |
| frontend/theme/shadcn-primitives-40.md#证据 | theme | 证据 | next-themes,theme,conflict,shadcn,sonner | active | - src/components/ui/sonner.tsx line 3: `import { useTheme } … |
| frontend/theme/shadcn-primitives-40.md#适用 | theme | 适用 | next-themes,theme,conflict,shadcn,sonner | active | shadcn 组件集成 + 主题体系迁移 |
| frontend/theme/shadcn-primitives-40.md#问题 | theme | 问题 | next-themes,theme,conflict,shadcn,sonner | active | shadcn Sonner 组件导入 next-themes 的 `useTheme`，与本项目自有主题体系（`src/… |
| ts-rust-boundary/mock-config-4layer-consistency.md#mock 配置四层覆盖的字段一致性检查 | ts-rust-boundary | mock 配置四层覆盖的字段一致性检查 | ts-rust-boundary,mock-config,consistency,serde,json-boundary | active | - |
| ts-rust-boundary/mock-config-4layer-consistency.md#失配场景 | ts-rust-boundary | 失配场景 | ts-rust-boundary,mock-config,consistency,serde,json-boundary | active | / 症状 / 原因 / /---/---/ / TS 编辑器赋值后无效 / `serializeMockConfig` … |
| ts-rust-boundary/mock-config-4layer-consistency.md#检查表（四处同步） | ts-rust-boundary | 检查表（四处同步） | ts-rust-boundary,mock-config,consistency,serde,json-boundary | active | ### 1. Rust struct 定义 (`config.rs:11-25`) - [ ] 新字段声明的类型：`Op… |
| ts-rust-boundary/mock-config-4layer-consistency.md#用途 | ts-rust-boundary | 用途 | ts-rust-boundary,mock-config,consistency,serde,json-boundary | active | Rust↔TS 跨边界的配置字段迭代通用检查表。适用于： - 平台/插件配置扩展 - 新增可选设置 - 配置升级 mig… |
| ts-rust-boundary/mock-config-4layer-consistency.md#问题 | ts-rust-boundary | 问题 | ts-rust-boundary,mock-config,consistency,serde,json-boundary | active | mock 配置在四层跨 Rust↔TS 边界流转，任一处字段定义/序列化不一致都导致静默失配：  1. **Rust s… |
| ts-rust-boundary/optional-config-backward-compat.md#Option<T> 可选字段的向后兼容方案 | ts-rust-boundary | Option<T> 可选字段的向后兼容方案 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | active | - |
| ts-rust-boundary/optional-config-backward-compat.md#关键点 | ts-rust-boundary | 关键点 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | active | - **旧字段保留**：必须保留兼容入口，不删不改 - **Option/undefined 对应**：Rust `Op… |
| ts-rust-boundary/optional-config-backward-compat.md#方案 | ts-rust-boundary | 方案 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | active | **Rust 端** (`config.rs:11-25`)： ```rust pub struct MockConfi… |
| ts-rust-boundary/optional-config-backward-compat.md#用途 | ts-rust-boundary | 用途 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | active | 配置迭代的通用方案，适用于： - 新增可选旋钮 - 旧版本平台配置升级 - 分阶段特性开关（旧特性先 disable，新… |
| ts-rust-boundary/optional-config-backward-compat.md#问题 | ts-rust-boundary | 问题 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | active | 新旋钮常需跨 Rust↔TS 边界，并与旧配置字段共存以确保向后兼容。  例：`mock` 配置新增 `ttft_ms`… |
