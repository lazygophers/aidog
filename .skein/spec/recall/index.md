# SKEIN recall 规则索引 (章节粒度: 一行一条规则)

类目: arch(116), build(62), cross-layer(12), db(25), domain(93), encoding(4), frontend(100), git(6), i18n(24), ops(9), optimization(41), proxy(41), reuse(6), shadcn(49), skein(24), style(18), test(12), testing(13), theme(5), ts-rust-boundary(10) · 关联见 [backlinks.md](backlinks.md)

| rule (topic.md#标题) | category | title | keywords | inclusion | anchors | status/出链 | summary |
|---|---|---|---|---|---|---|---|
| arch/auto-fix-downgrade-33.md#关联 | arch | 关联 | agent,handler,branch,platform,wire,sse | auto | - | active / →trellis-04 | dashmap-sharding (session 映射) [[trellis-04]] (enum 变体同步) |
| arch/auto-fix-downgrade-33.md#判定：分支 vs wire | arch | 判定：分支 vs wire | agent,handler,branch,platform,wire,sse | auto | - | active | / 特征 / wire 层 / handler 分支 / /------/---------/-------------… |
| arch/auto-fix-downgrade-33.md#反例 | arch | 反例 | agent,handler,branch,platform,wire,sse | auto | - | active | ❌ 新 agent 平台塞 wire 层 → adapter 改到吐血 ❌ 分支内做多候选 retry → agent … |
| arch/auto-fix-downgrade-33.md#触发场景 | arch | 触发场景 | agent,handler,branch,platform,wire,sse | auto | - | active | 新增「agent-as-LLM」类平台（无标准 chat completions wire，API 形态是 sessio… |
| arch/auto-fix-downgrade-33.md#适用 | arch | 适用 | agent,handler,branch,platform,wire,sse | auto | - | active | agent-as-LLM 平台接入（Mock/ClaudeCode/Devin/Factory） |
| arch/auto-fix-downgrade-33.md#陷阱-正解 | arch | 陷阱-正解 | agent,handler,branch,platform,wire,sse | auto | - | active | - **陷阱**: 新平台硬塞 wire 层 → adapter/converter 反复打补丁、协议转换丢字段、候选切… |
| arch/auto-fix-downgrade-34.md#关联 | arch | 关联 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active / →auto-fix-downgrade-35,cross-db-subquery-handle-selection | [[cross-db-subquery-handle-selection]] (跨库读两阶段) [[auto-fix-d… |
| arch/auto-fix-downgrade-34.md#反例 | arch | 反例 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | ❌ 只 grep `call_traced` → 6 处 `write_conn` 漏网（s3 错误模式） ❌ 只 gr… |
| arch/auto-fix-downgrade-34.md#触发场景 | arch | 触发场景 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | 表从一个 SQLite 库拆到另一个库（主库→log.db / platform.db），需把该表所有访问点切到新 ha… |
| arch/auto-fix-downgrade-34.md#适用 | arch | 适用 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | DB 拆库迁移、表访问点归属审计 |
| arch/auto-fix-downgrade-34.md#陷阱-正解 | arch | 陷阱-正解 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | - **陷阱**: 只查 `call_*_traced` chokepoint → 漏掉 `.write_conn()`… |
| arch/auto-fix-downgrade-34.md#验收命令 | arch | 验收命令 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | auto | - | active | ```bash # 1. wrapper 形式 grep -rn "call_platform_traced\/call… |
| arch/auto-fix-downgrade-35.md#关联 | arch | 关联 | dedup,空字段,key,数据丢失,合并 | auto | - | active / →shadcn-infra-32 | [[shadcn-infra-32]] (数据清理) |
| arch/auto-fix-downgrade-35.md#反例 | arch | 反例 | dedup,空字段,key,数据丢失,合并 | auto | - | active | ❌ (provider.source_segment, provider.base_url) 其中 base_url 全… |
| arch/auto-fix-downgrade-35.md#正解 | arch | 正解 | dedup,空字段,key,数据丢失,合并 | auto | - | active | dedup key 选择优先级： 1. **业务唯一键**(user_id / email / name) — 最稳 2… |
| arch/auto-fix-downgrade-35.md#测试 | arch | 测试 | dedup,空字段,key,数据丢失,合并 | auto | - | active | 构造 N 个对象(该字段全空但其余不同)，dedup 后必须保留 N 个(非合并为 1)。 |
| arch/auto-fix-downgrade-35.md#触发场景 | arch | 触发场景 | dedup,空字段,key,数据丢失,合并 | auto | - | active | 写任何 dedup / 去重 / 合并逻辑(HashSet key / HashMap key / groupBy ke… |
| arch/auto-fix-downgrade-35.md#适用 | arch | 适用 | dedup,空字段,key,数据丢失,合并 | auto | - | active | dedup / 去重 / 合并逻辑、数据导入解析 |
| arch/auto-fix-downgrade-35.md#陷阱 | arch | 陷阱 | dedup,空字段,key,数据丢失,合并 | auto | - | active | 字段设计为空(待后续回填 / 占位)但被用作 dedup key → N 个对象共享同一空值 → HashSet 全撞 … |
| arch/auto-fix-downgrade-38.md#MUST 流程 | arch | MUST 流程 | enum,serde,db,migration,rust,panic | auto | - | active | 1. 写 migration: DELETE FROM table WHERE enum_column = 'delet… |
| arch/auto-fix-downgrade-38.md#关联 | arch | 关联 | enum,serde,db,migration,rust,panic | auto | - | active / →shadcn-infra-32,trellis-04 | [[shadcn-infra-32]] (locale 清理) [[trellis-04]] (TS ↔ Rust en… |
| arch/auto-fix-downgrade-38.md#反例 | arch | 反例 | enum,serde,db,migration,rust,panic | auto | - | active | ❌ 先删代码再 migration → migration 期间所有访问 panic ❌ 只改 TS 未改 Rust e… |
| arch/auto-fix-downgrade-38.md#硬约束 | arch | 硬约束 | enum,serde,db,migration,rust,panic | auto | - | active | **删 serde 落库的 enum 变体前必须先 migration DELETE DB 旧值**，否则代码中 `fr… |
| arch/auto-fix-downgrade-38.md#触发场景 | arch | 触发场景 | enum,serde,db,migration,rust,panic | auto | - | active | 删 serde 落库的 enum 变体时。 |
| arch/auto-fix-downgrade-38.md#适用 | arch | 适用 | enum,serde,db,migration,rust,panic | auto | - | active | serde enum 变体删除、DB schema enum 迁移、前后端 enum 同步 |
| arch/coding-plan-utilization-calib-fix-25.md#coding plan 校准链路 base_url 真值源 = endpoint 级 | arch | coding plan 校准链路 base_url 真值源 = endpoint 级 | coding-plan,base_url,quota,calibration,finish,est_coding_plan | auto | - | active | coding plan 平台 preset 平台级 base_url 恒为 None (真 base_url 在 end… |
| arch/cross-db-subquery-handle-selection.md#Cross-ref | arch | Cross-ref | db,sqlite,跨库,补查,handle,闭包,cpp,平台名,N+1 | auto | - | active / →auto-fix-downgrade-34 | - sqlite-cross-db-no-join（跨库禁 JOIN，强制拆闭包 + Rust 合并） - [[auto… |
| arch/cross-db-subquery-handle-selection.md#MUST 规则 | arch | MUST 规则 | db,sqlite,跨库,补查,handle,闭包,cpp,平台名,N+1 | auto | - | active | 跨库补查闭包的 handle **必须按补查表的库归属选**，禁顺手复用主表 handle。 |
| arch/cross-db-subquery-handle-selection.md#正确写法（✅） | arch | 正确写法（✅） | db,sqlite,跨库,补查,handle,闭包,cpp,平台名,N+1 | auto | - | active | ```rust // 主查走 log.db handle let logs = proxy_log_handle.cal… |
| arch/cross-db-subquery-handle-selection.md#错误样本（❌） | arch | 错误样本（❌） | db,sqlite,跨库,补查,handle,闭包,cpp,平台名,N+1 | auto | - | active | ```rust // proxy_log 在 log.db，补查 cpp.name 在 platform.db prox… |
| arch/cross-db-subquery-handle-selection.md#验收 | arch | 验收 | db,sqlite,跨库,补查,handle,闭包,cpp,平台名,N+1 | auto | - | active | ```bash # 找跨库补查点（同函数 / 同闭包内出现多库表名） grep -rn 'FROM "proxy_log… |
| arch/non-typical-sql-audit-pattern.md#Cross-ref | arch | Cross-ref | db,sqlite,sql,审计,helper,裸sql,grep,易漏,访问点 | auto | - | active / →auto-fix-downgrade-34 | - [[auto-fix-downgrade-34]]（访问点审计总则，本文是其子形式之一） |
| arch/non-typical-sql-audit-pattern.md#MUST 审计两形态 | arch | MUST 审计两形态 | db,sqlite,sql,审计,helper,裸sql,grep,易漏,访问点 | auto | - | active | 拆库审计时 **禁只 grep helper 函数名**，必须同时查：  1. **Helper 函数形式**：`loa… |
| arch/non-typical-sql-audit-pattern.md#漏网样本（task config-db-split s5） | arch | 漏网样本（task config-db-split s5） | db,sqlite,sql,审计,helper,裸sql,grep,易漏,访问点 | auto | - | active | - `SELECT ... FROM "group" WHERE auto_from_platform` 不经任何 he… |
| arch/non-typical-sql-audit-pattern.md#验收命令 | arch | 验收命令 | db,sqlite,sql,审计,helper,裸sql,grep,易漏,访问点 | auto | - | active | ```bash # 按被拆表名 grep（FROM "table"），覆盖所有访问形态 grep -rn 'FROM "… |
| arch/parser-multi-path-format-symmetry.md#Cross-ref | arch | Cross-ref | parser,多路径,symmetry,对称,格式识别,抽函数,复用,入口分裂,oauth | auto | - | active / →auto-fix-downgrade-35,cpa-oauth-credential-format | - `src-tauri/crates/aidog_core/src/gateway/cpa_import/parser… |
| arch/parser-multi-path-format-symmetry.md#How to apply | arch | How to apply | parser,多路径,symmetry,对称,格式识别,抽函数,复用,入口分裂,oauth | auto | - | active | 1. grep parser 所有入口(`parse_*` / `scan_*` / `import_*`), 列各入口… |
| arch/parser-multi-path-format-symmetry.md#Why | arch | Why | parser,多路径,symmetry,对称,格式识别,抽函数,复用,入口分裂,oauth | auto | - | active | 多入口是常见模式(用户单文件 vs 批量目录 vs 压缩包)。格式识别逻辑若内联在各入口, 易漏对称: - 入口 A 加… |
| arch/parser-multi-path-format-symmetry.md#规则 | arch | 规则 | parser,多路径,symmetry,对称,格式识别,抽函数,复用,入口分裂,oauth | auto | - | active | parser 有多个入口(parse_single_file / scan_dir / scan_auth_dir / … |
| arch/rule-49.md#关联 | arch | 关联 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active / →rule-45,trellis-03,trellis-18 | [[rule-45]] (popover 域划分) / [[trellis-03]] (Crate 边界契约) / [[… |
| arch/rule-49.md#反例 | arch | 反例 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | ```rust // ❌ 陷阱实现（每次销毁） if let Some(w) = app.get_webview_win… |
| arch/rule-49.md#实现清单 | arch | 实现清单 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | - [ ] `app_setup.rs::setup` 阶段 `prebuild_popover()`：`.visibl… |
| arch/rule-49.md#性能收益 | arch | 性能收益 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | - 消除冷启 webview (setup 预建一次)。 - 去掉 tray click 时的 4 路 IPC 瀑布（背… |
| arch/rule-49.md#案例 | arch | 案例 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | - popover-perf task (commit 14ec141d)：预建隐藏窗 + toggle hide/sh… |
| arch/rule-49.md#触发场景 | arch | 触发场景 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | 实现 Tauri 桌面应用的浮窗（如托盘 popover）时，需要避免每次点击都冷启 webview，导致的延迟与卡顿。 |
| arch/rule-49.md#适用 | arch | 适用 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | - Tauri 桌面应用浮窗（托盘 popover、context menu、floating panel） - 需要快… |
| arch/rule-49.md#陷阱-正解 | arch | 陷阱-正解 | tauri,window,popover,performance,复用,hide/show,NSWindow | auto | - | active | ❌ **陷阱**：tray 点击每次 destroy + 新建窗口 → 冷启 webview + 瀑布 IPC 4 路 … |
| arch/rule-56.md#关联 | arch | 关联 | gemini,sse,streaming,adapter,parameter | auto | - | active / →rule-57,rule-58 | [[rule-57]] [[rule-58]] |
| arch/rule-56.md#案例 | arch | 案例 | gemini,sse,streaming,adapter,parameter | auto | - | active | - arch-deepen-2 commit `39a6614c`：gateway/proxy/forward.rs:2… |
| arch/rule-56.md#正解 | arch | 正解 | gemini,sse,streaming,adapter,parameter | auto | - | active | 向 Gemini 端点拼入 `?alt=sse` 参数，确保响应格式为 Server-Sent Events。 |
| arch/rule-56.md#触发场景 | arch | 触发场景 | gemini,sse,streaming,adapter,parameter | auto | - | active | 改 gemini adapter 或调试 Gemini streaming 响应时。 |
| arch/rule-56.md#适用 | arch | 适用 | gemini,sse,streaming,adapter,parameter | auto | - | active | - Gemini 协议 SSE 响应处理 - 其他 SSE 适配器的对称性检查（防止他协议有类似参数需求遗漏） |
| arch/rule-56.md#陷阱 | arch | 陷阱 | gemini,sse,streaming,adapter,parameter | auto | - | active | 不带 `?alt=sse` 参数时，Gemini API 响应体不是 SSE 格式（返回普通 JSON 数组），`str… |
| arch/rule-57.md#关联 | arch | 关联 | protocol,serde,wire,codegen,enum | auto | - | active / →rule-05 | [[rule-05]] |
| arch/rule-57.md#案例 | arch | 案例 | protocol,serde,wire,codegen,enum | auto | - | active | - gateway/models/protocol.rs:173 定义 wire_str() - arch-deepen… |
| arch/rule-57.md#正解 | arch | 正解 | protocol,serde,wire,codegen,enum | auto | - | active | 统一用 `Protocol::wire_str()` 方法序列化协议名。 |
| arch/rule-57.md#触发场景 | arch | 触发场景 | protocol,serde,wire,codegen,enum | auto | - | active | 在 proxy/forward 层需要获取协议名或序列化 Protocol enum 时。 |
| arch/rule-57.md#适用 | arch | 适用 | protocol,serde,wire,codegen,enum | auto | - | active | - Protocol enum 序列化时 - adapter 分发时协议名判定 |
| arch/rule-57.md#陷阱 | arch | 陷阱 | protocol,serde,wire,codegen,enum | auto | - | active | 禁手写 `serde_json::to_string(&x).trim_matches('"')` 或其他字符串转换，容… |
| arch/rule-58.md#关联 | arch | 关联 | adapter,dead_code,whitelist,protocol,authority | auto | - | active / →rule-07 | [[rule-07]] |
| arch/rule-58.md#案例 | arch | 案例 | adapter,dead_code,whitelist,protocol,authority | auto | - | active | - arch-deepen-2 commit `78e32df4`：删的 5 个 vendor adapter（glm_… |
| arch/rule-58.md#正解 | arch | 正解 | adapter,dead_code,whitelist,protocol,authority | auto | - | active | **唯一权威 = `gateway/proxy/forward.rs:85-86` 的 `is_valid_wire_p… |
| arch/rule-58.md#触发场景 | arch | 触发场景 | adapter,dead_code,whitelist,protocol,authority | auto | - | active | 删除 vendor adapter 文件或判定某 adapter 是否属于死代码时。 |
| arch/rule-58.md#适用 | arch | 适用 | adapter,dead_code,whitelist,protocol,authority | auto | - | active | - adapter 文件管理时 - protocol 数量变更 - 编码规范卡关：为什么要删这个文件 |
| arch/rule-58.md#陷阱 | arch | 陷阱 | adapter,dead_code,whitelist,protocol,authority | auto | - | active | 用文件名判定（如「vendor 名 = 协议名」），误删活代码；或遗漏实际有白名单的 adapter。 |
| arch/rule-59.md#关联 | arch | 关联 | refactor,component,extraction,grep,dead-code | auto | - | active / →auto-fix-downgrade-36 | [[auto-fix-downgrade-36]] |
| arch/rule-59.md#案例 | arch | 案例 | refactor,component,extraction,grep,dead-code | auto | - | active | - arch-deepen-2 commit `1eee3975`：删 ImportDialog 内联 91 行副本前先… |
| arch/rule-59.md#检查清单 | arch | 检查清单 | refactor,component,extraction,grep,dead-code | auto | - | active | ```bash # 抽前 & 抽后各一次 grep -r "ProviderRow" --include="*.tsx"… |
| arch/rule-59.md#正解 | arch | 正解 | refactor,component,extraction,grep,dead-code | auto | - | active | 1. grep 搜索原位置组件名，确认所有调用点 2. 逐个改为新 import 路径 3. 最后删旧副本前再 grep… |
| arch/rule-59.md#触发场景 | arch | 触发场景 | refactor,component,extraction,grep,dead-code | auto | - | active | 从大文件抽出独立组件或把函数迁移到新位置时。 |
| arch/rule-59.md#适用 | arch | 适用 | refactor,component,extraction,grep,dead-code | auto | - | active | - UI 组件抽取重构 - 函数迁 crate 时 - 任何多处定义的重复 |
| arch/rule-59.md#陷阱 | arch | 陷阱 | refactor,component,extraction,grep,dead-code | auto | - | active | 只 import 不渲染 = 死代码副本。原文件可能仍有内联副本，抽取后遗漏切换会导致两份代码。 |
| arch/rule-60.md#关联 | arch | 关联 | command,tauri,handler,migration,invoke,symmetry | auto | - | active | - |
| arch/rule-60.md#案例 | arch | 案例 | command,tauri,handler,migration,invoke,symmetry | auto | - | active | - arch-deepen-2 batch 3：commands 迁 aidog_core 时，verify 用 com… |
| arch/rule-60.md#正解 | arch | 正解 | command,tauri,handler,migration,invoke,symmetry | auto | - | active | **invoke 名的真值源 = `src-tauri/src/startup.rs:41` 的 `tauri::gen… |
| arch/rule-60.md#触发场景 | arch | 触发场景 | command,tauri,handler,migration,invoke,symmetry | auto | - | active | command 跨 crate 搬迁后（新增、删除、拆分 command）。 |
| arch/rule-60.md#适用 | arch | 适用 | command,tauri,handler,migration,invoke,symmetry | auto | - | active | - command 跨 crate 搬迁 - 新增/删除 command - 重构后 sanity check |
| arch/rule-60.md#陷阱 | arch | 陷阱 | command,tauri,handler,migration,invoke,symmetry | auto | - | active | 改了 Rust 函数签名或迁移位置，却漏改了前端 invoke 名或 startup.rs 注册，导致静默失败。 |
| arch/rule-62.md#关联 | arch | 关联 | i18n,migration,locale,key,coverage,comm | auto | - | active | - |
| arch/rule-62.md#案例 | arch | 案例 | i18n,migration,locale,key,coverage,comm | auto | - | active | - arch-deepen-2 c3-commands batch 3：搬迁时检查 system/ai_tools/cl… |
| arch/rule-62.md#正解 | arch | 正解 | i18n,migration,locale,key,coverage,comm | auto | - | active | 搬迁前后比对 locale key 集合（grep 源代码找 namespace 模式），用 comm -23 差集查漏… |
| arch/rule-62.md#触发场景 | arch | 触发场景 | i18n,migration,locale,key,coverage,comm | auto | - | active | command/组件迁 crate 或改名时，若涉及 i18n key（如 UI 文案）。 |
| arch/rule-62.md#适用 | arch | 适用 | i18n,migration,locale,key,coverage,comm | auto | - | active | - 跨 crate 搬迁涉及 i18n - rename command 时 - 删减功能前验证 |
| arch/rule-62.md#陷阱 | arch | 陷阱 | i18n,migration,locale,key,coverage,comm | auto | - | active | 不动 locale 文件时 `yarn check-i18n` 查不出搬迁丢 key（新位置 key 可能取名不同）。 |
| arch/rule-64.md#关联 | arch | 关联 | tauri,command,macro,parameter,mut | auto | - | active | - |
| arch/rule-64.md#案例 | arch | 案例 | tauri,command,macro,parameter,mut | auto | - | active | - arch-deepen-2：迁 command 时遇此限制 |
| arch/rule-64.md#正解 | arch | 正解 | tauri,command,macro,parameter,mut | auto | - | active | 去掉函数签名中的 `mut`，在函数体首行用 `let mut x = x;` 重绑定： ```rust // 错误 #… |
| arch/rule-64.md#触发场景 | arch | 触发场景 | tauri,command,macro,parameter,mut | auto | - | active | Tauri command 函数形参中使用 `mut` 修饰时。 |
| arch/rule-64.md#适用 | arch | 适用 | tauri,command,macro,parameter,mut | auto | - | active | - Tauri command 签名设计 - 其他 proc macro 类似限制排查 |
| arch/rule-64.md#陷阱 | arch | 陷阱 | tauri,command,macro,parameter,mut | auto | - | active | `tauri_command!` 宏模式 `$($arg:ident : $ty:ty),*` 不匹配 `mut x: … |
| arch/shadcn-infra-32.md#关联 | arch | 关联 | locale,dead-key,cleanup,responsibility,theme | auto | - | active / →auto-fix-downgrade-38 | [[auto-fix-downgrade-38]] (同任务 enum 删约定) |
| arch/shadcn-infra-32.md#反例 | arch | 反例 | locale,dead-key,cleanup,responsibility,theme | auto | - | active | ❌ 删 palette 只改代码不清理 locale → 死键残留 ❌ 甩给「下次整理 locale 时」→ 永远不清理… |
| arch/shadcn-infra-32.md#案例 | arch | 案例 | locale,dead-key,cleanup,responsibility,theme | auto | - | active | - shadcn-infra task: 删 palette 时应同步清理 theme.color.* locale 键 |
| arch/shadcn-infra-32.md#正解 | arch | 正解 | locale,dead-key,cleanup,responsibility,theme | auto | - | active | 1. **删 palette 主题**: 清理所有 `theme.color.{palette}` 相关 locale … |
| arch/shadcn-infra-32.md#流程约定 | arch | 流程约定 | locale,dead-key,cleanup,responsibility,theme | auto | - | active | **删除主题/功能导致的 locale 死键，由删该主题/功能的 task 同源清理**，不甩给下游消费 task。 |
| arch/shadcn-infra-32.md#适用 | arch | 适用 | locale,dead-key,cleanup,responsibility,theme | auto | - | active | locale 清理、主题删除、功能下架、enum 变体删除 |
| arch/shadcn-infra-32.md#陷阱 | arch | 陷阱 | locale,dead-key,cleanup,responsibility,theme | auto | - | active | - **陷阱**: 删代码只删 TS 类型，locale 死键留给后续清理 → 下次改 locale 人困惑 - **陷… |
| arch/trellis-03.md#C8 复查清单模式 (MUST，迁移期临时合法 → 后续 task 改) | arch | C8 复查清单模式 (MUST，迁移期临时合法 → 后续 task 改) | crate,boundary,边界,commands,aidog_core,event,依赖 | auto | - | active | - 迁 command 文件时若发现 **同 crate 内部** 跨域直调（如 `commands_platform:… |
| arch/trellis-03.md#Cross-reference | arch | Cross-reference | crate,boundary,边界,commands,aidog_core,event,依赖 | auto | - | active | - workspace 重构过程契约（PoC 骨架门禁 + 核心提取下沉防循环范式）: [Cargo Workspace… |
| arch/trellis-03.md#实例 | arch | 实例 | crate,boundary,边界,commands,aidog_core,event,依赖 | auto | - | active | - task 07-10-cmd-proxy（C4 commands-proxy crate 落地）: 5 源文件（pr… |
| arch/trellis-03.md#范式 (MUST，稳态边界规则，与 cargo-workspace.md 重构过程契约互补) | arch | 范式 (MUST，稳态边界规则，与 cargo-workspace.md 重构过程契约互补) | crate,boundary,边界,commands,aidog_core,event,依赖 | auto | - | active | workspace 拓扑（commands-restructure 落地后）：`crates/{aidog_core, … |
| arch/trellis-03.md#验收断言（可复用） | arch | 验收断言（可复用） | crate,boundary,边界,commands,aidog_core,event,依赖 | auto | - | active | ```bash # 规则 1: commands_* 间零互依赖 grep -rn 'commands_platform… |
| arch/trellis-04.md#Cross-reference | arch | Cross-reference | protocol,enum,变体,grep,serde,match,union | auto | - | active | - research 结论：`.trellis/tasks/archive/2026-07/07-10-protocol… |
| arch/trellis-04.md#serde round-trip + JSON key 对齐 (MUST) | arch | serde round-trip + JSON key 对齐 (MUST) | protocol,enum,变体,grep,serde,match,union | auto | - | active | - `#[serde(rename = "<key>")]` 与 `platform-presets.json` pro… |
| arch/trellis-04.md#命中点 3 类分类（据实判定改动面） | arch | 命中点 3 类分类（据实判定改动面） | protocol,enum,变体,grep,serde,match,union | auto | - | active | grep 同构变体命中点，按下列 3 类分类，**仅第 1 类必须改**：  1. **enum 定义 + serde … |
| arch/trellis-04.md#实例 | arch | 实例 | protocol,enum,变体,grep,serde,match,union | auto | - | active | task 07-10-protocols-rust-enum：+3 cp 变体（KimiCoding/QianfanCo… |
| arch/trellis-04.md#新增变体 MUST 先 grep 同构变体命中点 (MUST) | arch | 新增变体 MUST 先 grep 同构变体命中点 (MUST) | protocol,enum,变体,grep,serde,match,union | auto | - | active | 新增 `Protocol` 变体前，**MUST** grep 现有同构变体全链命中点，据实际命中分类决定改动面，禁预设… |
| arch/trellis-04.md#零专属 match 臂 → 加枚举即覆盖 (MUST) | arch | 零专属 match 臂 → 加枚举即覆盖 (MUST) | protocol,enum,变体,grep,serde,match,union | auto | - | active | **反直觉发现**：`router.rs` / `adapter/converter.rs` / `quota.rs` … |
| arch/trellis-04.md#验收断言（可复用） | arch | 验收断言（可复用） | protocol,enum,变体,grep,serde,match,union | auto | - | active | ```bash # 新变体字面量全链命中点清单（据分类决定改动面） grep -rn '<NewVariant>\/<n… |
| arch/trellis-05.md#AppContext 预热缓存 (best-effort) | arch | AppContext 预热缓存 (best-effort) | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | AppContext 顶层调一次 `buildXFromPresets().catch(console.error)` … |
| arch/trellis-05.md#Cross-reference | arch | Cross-reference | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | - 真值源: `src-tauri/defaults/platform-presets.json`（手维护，禁机器生成覆… |
| arch/trellis-05.md#单真值源派生 (MUST) | arch | 单真值源派生 (MUST) | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | 前端平台 / 协议类大枚举常量（`PROTOCOLS` / `PROTOCOL_LABELS` / `PROTOCOL_… |
| arch/trellis-05.md#实例 | arch | 实例 | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | task 07-10-protocols-frontend-derive（C3）： - 删 `PROTOCOLS`（81… |
| arch/trellis-05.md#小常量例外（保留硬编码） | arch | 小常量例外（保留硬编码） | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | 非后端真值源映射的小常量（请求格式协议 5 条 `ENDPOINT_PROTOCOLS` / 路由判定 / UI 固定枚… |
| arch/trellis-05.md#调用点 async 化范式 (MUST) | arch | 调用点 async 化范式 (MUST) | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | 派生函数 async 后，所有 caller **MUST** 改 `useEffect + useState` 模式，… |
| arch/trellis-05.md#验收断言（可复用） | arch | 验收断言（可复用） | derived,constants,docpromise,defaults,派生,presets,async | auto | - | active | ```bash # 派生层单 RPC 缓存（docPromise module-level 单次 invoke，非函数内… |
| build/rule-05.md#MUST 硬约束 | build | MUST 硬约束 | - | auto | - | active | 新增 wire protocol 时必须同步更新以下白名单，否则新协议会导致 route fail： - forward… |
| build/rule-05.md#关联 | build | 关联 | - | auto | - | active / →rule-52,rule-53 | [[rule-52]] [[rule-53]] |
| build/rule-05.md#反例 | build | 反例 | - | auto | - | active | - 新增 protocol X 但未加入白名单 → matched_ep=None 时 fallback 到 platf… |
| build/rule-05.md#触发场景 | build | 触发场景 | - | auto | - | active | - converter-reasoning-content task：bug1 根因分析发现 matched_ep=No… |
| build/rule-05.md#适用 | build | 适用 | - | auto | - | active | - 所有新增 wire protocol（endpoint 协议层）的变更 - 非 platform_type（平台别名… |
| build/rule-06.md#MUST 硬约束 | build | MUST 硬约束 | - | auto | - | active | converter 双向转（source→wire 请求 + wire→source 响应）与 endpoint 选择解… |
| build/rule-06.md#关联 | build | 关联 | - | auto | - | active | - |
| build/rule-06.md#反例 | build | 反例 | - | auto | - | active | - ❌ 误判：endpoint 层限制只许选同协议 → converter 能力已就绪，endpoint 无需自我限制 … |
| build/rule-06.md#案例 | build | 案例 | - | auto | - | active / →rule-07,rule-55 | - endpoint-cross-protocol-fallback task：converter 5×5 已就绪，en… |
| build/rule-06.md#适用 | build | 适用 | - | auto | - | active | - 所有新增 wire protocol 的变更 - endpoint 跨协议回退扩展 - converter 双向转换… |
| build/rule-07.md#MUST 硬约束 | build | MUST 硬约束 | - | auto | - | active | is_valid_wire_protocol gate 触发（502）说明 endpoint 选择失败（matched_… |
| build/rule-07.md#关联 | build | 关联 | - | auto | - | active | - |
| build/rule-07.md#反例 | build | 反例 | - | auto | - | active | - 只修白名单而未修 select → 新协议仍 502（根因未除） - 误判为 endpoint 配置缺 protoc… |
| build/rule-07.md#案例 | build | 案例 | - | auto | - | active / →rule-05,rule-54 | - converter-reasoning-content bug1：preset 未加载致 matched_ep=No… |
| build/rule-07.md#适用 | build | 适用 | - | auto | - | active | - 所有 502 route fail 场景 - is_valid_wire_protocol gate 触发 - en… |
| build/rule-61.md#关联 | build | 关联 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active / →rule-63 | [[rule-63]] |
| build/rule-61.md#案例 | build | 案例 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | - arch-deepen-2：迁移函数后 clippy 无新输出，touch 才触发重编检查 |
| build/rule-61.md#正解 | build | 正解 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | 修改源文件后跑 clippy 前，先 `touch` 该文件强制重编： ```bash touch src-tauri/… |
| build/rule-61.md#触发场景 | build | 触发场景 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | 修改后再跑 `cargo clippy` 判断 warning 数时。 |
| build/rule-61.md#适用 | build | 适用 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | - 验证 clippy 改动效果 - 高频编译场景 - 持续集成前检查 |
| build/rule-61.md#陷阱 | build | 陷阱 | cargo,clippy,cache,warning,touch,rebuild | auto | - | active | 同命令第二次跑输出为空（命中编译缓存），易误判「0 warning」实际仍有。 |
| build/rule-63.md#关联 | build | 关联 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active / →rule-61 | [[rule-61]] |
| build/rule-63.md#案例 | build | 案例 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | - arch-deepen-2 c3-commands batch 3：commands_tray/commands_s… |
| build/rule-63.md#检查 | build | 检查 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | ```bash # 检查迁移后是否仍能编译通过 cargo build -p aidog_core  # 应无 env!… |
| build/rule-63.md#正解 | build | 正解 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | 迁移代码到新 crate 后，给**新 crate 补等价的 build.rs**，重新定义环境变量。 |
| build/rule-63.md#触发场景 | build | 触发场景 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | 用 `env!("XXX")` 的代码从一个 crate 迁移到另一个 crate 时。 |
| build/rule-63.md#适用 | build | 适用 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | - 任何用 env!() 的代码跨 crate 迁移 - workspace 多 crate 场景 - build.rs… |
| build/rule-63.md#陷阱 | build | 陷阱 | env,compile-time,build.rs,cargo:rustc-env,scope | auto | - | active | `cargo:rustc-env=` 在 build.rs 中定义的环境变量**只对定义它的 crate 生效**，跨 … |
| build/shadcn-infra-02.md#MUST 迁移方式 | build | MUST 迁移方式 | tailwind,v4,preflight,migration,css | auto | - | active | 1. 仅 import theme/utilities（跳过 preflight/base） 2. 或单行总导入：@im… |
| build/shadcn-infra-02.md#关联 | build | 关联 | tailwind,v4,preflight,migration,css | auto | - | active / →shadcn-infra-28,shadcn-infra-30 | [[shadcn-infra-30]] [[shadcn-infra-28]] |
| build/shadcn-infra-02.md#硬约束 | build | 硬约束 | tailwind,v4,preflight,migration,css | auto | - | active | Tailwind v4 迁移过程中**禁使用旧 v3 的三行导入方式**，必须用 v4 的 @import 方式。 |
| build/shadcn-infra-02.md#禁用的旧方式 | build | 禁用的旧方式 | tailwind,v4,preflight,migration,css | auto | - | active | ❌ @tailwind base;  /* v3 方式，v4 崩盘 */ ❌ @tailwind components;… |
| build/shadcn-infra-02.md#适用 | build | 适用 | tailwind,v4,preflight,migration,css | auto | - | active | Tailwind v3 → v4 迁移、新项目用 v4 |
| build/shadcn-infra-28.md#关联 | build | 关联 | shadcn,cva,yarn,dependency,class-variance-authority | auto | - | active / →shadcn-infra-31 | [[shadcn-infra-31]] (同任务产出的前端规则) |
| build/shadcn-infra-28.md#反例 | build | 反例 | shadcn,cva,yarn,dependency,class-variance-authority | auto | - | active | ❌ 只加 UI 组件不验证 cva → 运行时崩 ❌ 改 package.json 后不 yarn install → … |
| build/shadcn-infra-28.md#案例 | build | 案例 | shadcn,cva,yarn,dependency,class-variance-authority | auto | - | active | - shadcn-infra task: 首次 `shadcn add` 后运行时崩，发现 cva 缺失 - 根因: y… |
| build/shadcn-infra-28.md#触发场景 | build | 触发场景 | shadcn,cva,yarn,dependency,class-variance-authority | auto | - | active | 运行 `npx shadcn add` 批量添加组件后，依赖树中仅含 `@radix-ui/react-slot` 等 … |
| build/shadcn-infra-28.md#适用 | build | 适用 | shadcn,cva,yarn,dependency,class-variance-authority | auto | - | active | yarn 4+ / pnp 环境，shadcn 批量 add 场景 |
| build/shadcn-infra-28.md#陷阱-正解 | build | 陷阱-正解 | shadcn,cva,yarn,dependency,class-variance-authority | auto | - | active | - **陷阱**: shadcn CLI 在 yarn 4+ / pnp 环境下可能未正确解析 cva 传递依赖，只装直… |
| build/shadcn-infra-29.md#关联 | build | 关联 | vite,alias,resolve,shadcn,tsconfig | auto | - | active / →shadcn-infra-28 | [[shadcn-infra-28]] (同任务 cva 依赖) |
| build/shadcn-infra-29.md#反例 | build | 反例 | vite,alias,resolve,shadcn,tsconfig | auto | - | active | ❌ 只配 vite alias 不配 tsconfig → 类型检查报错 ❌ 用相对路径 `../../componen… |
| build/shadcn-infra-29.md#案例 | build | 案例 | vite,alias,resolve,shadcn,tsconfig | auto | - | active | - shadcn-infra task: shadcn 生成的组件含 `import @/components/xxx`… |
| build/shadcn-infra-29.md#触发场景 | build | 触发场景 | vite,alias,resolve,shadcn,tsconfig | auto | - | active | 使用 shadcn/ui 或其他假设存在 `@` 别名的库时，项目原无 `@` → `src` 的路径别名配置，导致 `… |
| build/shadcn-infra-29.md#适用 | build | 适用 | vite,alias,resolve,shadcn,tsconfig | auto | - | active | shadcn/ui 迁移、Vite 从零配置、路径别名标准化 |
| build/shadcn-infra-29.md#陷阱-正解 | build | 陷阱-正解 | vite,alias,resolve,shadcn,tsconfig | auto | - | active | - **陷阱**: shadcn 假设 vite 已有 `@` 别名（标准 scaffolding 如 Vite 默认模… |
| build/tauri-build-bundle.md#yarn tauri build --no-bundle 不产 .app | build | yarn tauri build --no-bundle 不产 .app | tauri,build,bundle,macos,app-package,binary | auto | - | active | - |
| build/tauri-build-bundle.md#反例（错误模式） | build | 反例（错误模式） | tauri,build,bundle,macos,app-package,binary | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / `yarn tauri build --no-bundle` / … |
| build/tauri-build-bundle.md#案例 | build | 案例 | tauri,build,bundle,macos,app-package,binary | auto | - | active | 性能测试中需要获取原始二进制做行为测试。尝试 `yarn tauri build --no-bundle` 后发现 `b… |
| build/tauri-build-bundle.md#触发场景 | build | 触发场景 | tauri,build,bundle,macos,app-package,binary | auto | - | active | Tauri macOS 构建时使用 `yarn tauri build --no-bundle` 时，只产生裸二进制 `… |
| build/tauri-build-bundle.md#适用 | build | 适用 | tauri,build,bundle,macos,app-package,binary | auto | - | active | - Tauri macOS 应用打包 - CI/CD 中需确保 .app 生成 - 区分二进制构建 vs app bun… |
| build/tauri-build-bundle.md#陷阱 & 正解 | build | 陷阱 & 正解 | tauri,build,bundle,macos,app-package,binary | auto | - | active | ❌ **陷阱**：假设 `--no-bundle` 仅跳过签名/通证，仍产 `.app`  ```bash yarn t… |
| build/trellis-02.md#Cross-reference | build | Cross-reference | cargo,workspace,crate,build.rs,重构,门禁,下沉 | auto | - | active | - parent design：`.trellis/tasks/07-10-commands-restructure/d… |
| build/trellis-02.md#GUI 冒烟降级（worktree 无 display 时） | build | GUI 冒烟降级（worktree 无 display 时） | cargo,workspace,crate,build.rs,重构,门禁,下沉 | auto | - | active | worktree 无 `node_modules` / 无 display 无法跑 `yarn tauri dev` 全… |
| build/trellis-02.md#PoC 空骨架门禁 (MUST) | build | PoC 空骨架门禁 (MUST) | cargo,workspace,crate,build.rs,重构,门禁,下沉 | auto | - | active | 单 crate → workspace 多 crate 重构 **MUST 先建空骨架 PoC 门禁**，过才放行全量迁… |
| build/trellis-02.md#PoC 门禁验收 (MUST，全量迁移前必过) | build | PoC 门禁验收 (MUST，全量迁移前必过) | cargo,workspace,crate,build.rs,重构,门禁,下沉 | auto | - | active | 1. `cargo build --workspace`：0 errors（含现 root crate + N 空壳 +… |
| build/trellis-02.md#root 过渡路径迁移 (MUST) | build | root 过渡路径迁移 (MUST) | cargo,workspace,crate,build.rs,重构,门禁,下沉 | auto | - | active | core 提取后 root package **过渡保留**（binary crate C10 才建），加 `aidog… |
| build/trellis-02.md#workspace.dependencies 版本对齐 (MUST) | build | workspace.dependencies 版本对齐 (MUST) | cargo,workspace,crate,build.rs,重构,门禁,下沉 | auto | - | active | - `[workspace.dependencies]` 版本号 + features **MUST 逐项照抄**现 r… |
| build/trellis-02.md#子 crate 规范 (MUST) | build | 子 crate 规范 (MUST) | cargo,workspace,crate,build.rs,重构,门禁,下沉 | auto | - | active | - `name` 用下划线（`commands_platform` 等，非 hyphen；目录名连字符是 Cargo 惯… |
| build/trellis-02.md#实例 | build | 实例 | cargo,workspace,crate,build.rs,重构,门禁,下沉 | auto | - | active | task 07-10-ws-skeleton（commands-restructure C1）：src-tauri 单 … |
| build/trellis-02.md#核心提取下沉防循环范式 (MUST) | build | 核心提取下沉防循环范式 (MUST) | cargo,workspace,crate,build.rs,重构,门禁,下沉 | auto | - | active | PoC 空骨架过门后，业务代码入 `aidog_core` 时**MUST** 据依赖关系分类下沉，防 core→com… |
| build/trellis-02.md#验收断言（可复用） | build | 验收断言（可复用） | cargo,workspace,crate,build.rs,重构,门禁,下沉 | auto | - | active | ```bash # baseline 不回归 cargo test --workspace --lib / grep -… |
| build/trellis-02.md#验收断言（核心提取，可复用） | build | 验收断言（核心提取，可复用） | cargo,workspace,crate,build.rs,重构,门禁,下沉 | auto | - | active | ```bash # 路径迁移彻底（root 残留核心域路径 = 漏改） grep -rn 'crate::gateway… |
| cross-layer/trellis-20.md#CRUD Pattern (MUST) | cross-layer | CRUD Pattern (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | auto | - | active | - 每个 resource 必须在 `api.ts` 提供 `{ create, list, get, update, … |
| cross-layer/trellis-20.md#Data Flow (MUST) | cross-layer | Data Flow (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | auto | - | active | - 数据流必须单向: Rust command → `invoke` → React `useState` → JSX … |
| cross-layer/trellis-20.md#Format Contracts (MUST) | cross-layer | Format Contracts (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | auto | - | active | - 后端返回 timestamp 必须为 ISO 8601 string (`chrono::DateTime<Utc>… |
| cross-layer/trellis-20.md#Rust enum → type alias arbitrary 全 JSON 驱动 (MUST) | cross-layer | Rust enum → type alias arbitrary 全 JSON 驱动 (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | auto | - | active | Rust enum 当变体集合属「后端 JSON 真值源派生」类（值集合由 `src-tauri/defaults/*.… |
| cross-layer/trellis-20.md#Rust 执行层 match 臂 → JSON 真值源配置驱动引擎 (MUST) | cross-layer | Rust 执行层 match 臂 → JSON 真值源配置驱动引擎 (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | auto | - | active | Rust 执行层（如 proxy headers 注入）写死 per-variant dispatch (`match … |
| cross-layer/trellis-20.md#Tauri 窗口生命周期事件 (MUST) | cross-layer | Tauri 窗口生命周期事件 (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | auto | - | active | - 窗口生命周期事件 (失焦 `Focused` / 关闭 `CloseRequested` / 缩放 `Resized… |
| cross-layer/trellis-20.md#Tauri↔React Boundary (MUST) | cross-layer | Tauri↔React Boundary (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | auto | - | active | - 后端新增 Tauri command 必须在前端 `api.ts` 添加对应 invoke 包装函数 - invok… |
| cross-layer/trellis-20.md#Verification | cross-layer | Verification | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | auto | - | active | ```bash # 所有 invoke 集中在 api.ts grep -rn 'invoke(' src/ / gre… |
| cross-layer/trellis-20.md#反模式 (禁) | cross-layer | 反模式 (禁) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | auto | - | active | / 反模式 / 正确做法 / 触发后果 / / --- / --- / --- / / `invoke(` 散落在组件 … |
| cross-layer/trellis-20.md#持久化路径换、公共契约零改 (MUST) | cross-layer | 持久化路径换、公共契约零改 (MUST) | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | auto | - | active | 换持久化路径（专属表 → `setting` / JSON / 他处）时，跨 Rust↔TS **公共契约层禁改** —… |
| cross-layer/ts-rust-symmetry.md#单启用平台判定对称性 (Rust ↔ TS) | cross-layer | 单启用平台判定对称性 (Rust ↔ TS) | cross-layer,symmetry,sole_platform,Rust,TypeScript,判定对称 | auto | - | active | - |
| cross-layer/ts-rust-symmetry.md#跨层对称硬规 (Rust ↔ TS) | cross-layer | 跨层对称硬规 (Rust ↔ TS) | cross-layer,symmetry,sole_platform,Rust,TypeScript,判定对称 | auto | - | active | ### 约束  **同一判定逻辑在 Rust 与 TS 各有一份实现，改一处必改另一处。**  口径须与互指注释锁定对称… |
| db/crash-safe-db-split-migration.md#Cross-ref | db | Cross-ref | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | auto | - | active / →auto-fix-downgrade-34 | - [[auto-fix-downgrade-34]]（访问点审计） - dual-db-aggregate-is-me… |
| db/crash-safe-db-split-migration.md#MUST 四阶段模式（✅） | db | MUST 四阶段模式（✅） | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | auto | - | active | ``` Phase 1: read-without-drop（源库读全行入 Vec，不 DROP） Phase 2: 目… |
| db/crash-safe-db-split-migration.md#crash 恢复矩阵 | db | crash 恢复矩阵 | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | auto | - | active | / crash 点 / 重启行为 / /---/---/ / Phase 1 前/中 / 源表在，重读 / / Phas… |
| db/crash-safe-db-split-migration.md#保 id（MUST） | db | 保 id（MUST） | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | auto | - | active | `INSERT INTO platform SELECT *` / 显式列含 id 保原 id。log.db.proxy… |
| db/crash-safe-db-split-migration.md#实例 | db | 实例 | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | auto | - | active | task config-db-split（s2）：platform / group / group_platform /… |
| db/crash-safe-db-split-migration.md#禁用模式（❌） | db | 禁用模式（❌） | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | auto | - | active | `read → DROP 源表 → INSERT 目标库`（notification migration 049 原模式… |
| db/filter-semantics.md#排斥列默认过滤需明确确认为产品设计意图 | db | 排斥列默认过滤需明确确认为产品设计意图 | filter,exclude,semantics,product-design,default-behavior | auto | - | active | 当 task 涉及「默认排斥某类请求」的过滤逻辑时（如 Logs 主页默认隐藏 test/quota 请求），确认这是*… |
| db/pagination-offset.md#LIMIT+1 探测分页无精确总数 | db | LIMIT+1 探测分页无精确总数 | pagination,limit,offset,has_more,count,full-table-scan | auto | - | active | 当分页 UI 仅需「有无下一页」而不需精确总数时，改用 LIMIT offset+pageSize+1 探测有下一页，而… |
| db/sqlite-cache-residency-probe-method.md#SQLite 页缓存常驻量的直接探针方法 | db | SQLite 页缓存常驻量的直接探针方法 | sqlite,page-cache,measurement,heap,malloc,probe | auto | - | active | - |
| db/sqlite-cache-residency-probe-method.md#页缓存常驻量探针 | db | 页缓存常驻量探针 | sqlite,page-cache,measurement,heap,malloc,probe | auto | - | active / →measure-window-exclusive-env,sqlite-cache-measurement-traps,sqlite-read-cache-config | ### 方法  用 `heap --addresses 'malloc[5k]'` 的 5KB 块数作为 SQLite … |
| db/sqlite-partial-index.md#参数化查询无法触发 partial index（字面量盲区） | db | 参数化查询无法触发 partial index（字面量盲区） | sqlite,partial-index,query-plan,parameter-binding,sargable | auto | - | active | SQLite 查询规划器对 partial index 的匹配仅识别 SQL 文本中的**字面量常量**谓词，不识别**… |
| db/trellis-00.md#Column Naming (MUST) | db | Column Naming (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | auto | - | active | - 平台主类型列名为 `platform_type`（禁 `protocol`）；其值用 `serde_json::to… |
| db/trellis-00.md#Migration (MUST) | db | Migration (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | auto | - | active | - schema 破坏式变更必须提供独立一次性迁移脚本（`scripts/`，非 app 运行时代码），迁移完成后删除 … |
| db/trellis-00.md#No NULL (MUST) | db | No NULL (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | auto | - | active | - 所有 `TEXT` 列 `NOT NULL DEFAULT ''`；所有 `INTEGER` 列 `NOT NULL… |
| db/trellis-00.md#Primary Key (MUST) | db | Primary Key (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | auto | - | active | - 业务表主键必须 `id INTEGER PRIMARY KEY AUTOINCREMENT`，Rust 映射 `u6… |
| db/trellis-00.md#Relations & Mappings (MUST) | db | Relations & Mappings (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | auto | - | active | - 关联表（如 `group_platform`）加代理 `id` 自增主键 + 保留业务复合 `UNIQUE(grou… |
| db/trellis-00.md#Soft Delete (MUST) | db | Soft Delete (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | auto | - | active | - 删除必须逻辑删：`UPDATE <table> SET deleted_at = <now_ms> WHERE id… |
| db/trellis-00.md#Table Naming (MUST) | db | Table Naming (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | auto | - | active | - 表名必须**单数**，禁复数：`platform` / `group` / `group_platform` / `… |
| db/trellis-00.md#Time Fields (MUST) | db | Time Fields (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | auto | - | active | - 每个表必须含 `created_at` / `updated_at` / `deleted_at`，类型 `INTE… |
| db/trellis-00.md#Verification | db | Verification | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | auto | - | active | ```bash # 复数表名残留 sqlite3 ~/.aidog/aidog.db ".tables" / grep … |
| db/trellis-00.md#专属表 → setting 迁移模式 (MUST) | db | 专属表 → setting 迁移模式 (MUST) | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | auto | - | active | 域数据从专属表迁通用 `setting` 表时（`scope=<域>, key=<实体>` JSON），走 app 内置… |
| db/trellis-01.md#反例（禁） | db | 反例（禁） | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | auto | - | active | - 禁在 handler 层才重试 route（只覆盖 route 路径，写连接死亡无法兜底；Db 层统一兜底全覆盖）。… |
| db/trellis-01.md#契约（MUST） | db | 契约（MUST） | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | auto | - | active | - `call_traced` / `call_read_traced` 检测 `Error::ConnectionCl… |
| db/trellis-01.md#根因（tokio_rusqlite 0.6.0 已知行为，库层不可改） | db | 根因（tokio_rusqlite 0.6.0 已知行为，库层不可改） | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | auto | - | active | - `Connection` 内部 `event_loop`（`tokio-rusqlite-0.6.0/src/lib… |
| db/trellis-01.md#验证（可 grep / 可 test） | db | 验证（可 grep / 可 test） | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | auto | - | active | - `grep -n "ConnectionClosed\/reopen_write_conn\/pool.pick" … |
| domain/bundled-models-fallback.md#关联 | domain | 关联 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active / →rule-66,time-tiers-apply-idiom | [[time-tiers-apply-idiom]] [[rule-66]] |
| domain/bundled-models-fallback.md#反例 | domain | 反例 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | ```rust // ❌ 启动 seed （版本冲突、IO 阻塞） #[init] async fn on_startu… |
| domain/bundled-models-fallback.md#触发场景 | domain | 触发场景 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | 只读配置数据（models.json 价格表、platform-presets.json）需在 DB 为空或未同步时兜底… |
| domain/bundled-models-fallback.md#路径计算 | domain | 路径计算 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | `include_str!` 相对路径**从当前 .rs 文件出发**（不是 Cargo.toml 所在目录）： - `… |
| domain/bundled-models-fallback.md#适用 | domain | 适用 | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | - 只读配置（定价表、平台预设、常量列表） - 冷启动不依赖 RPC / 版本同步 - DB 可能暂时为空、滞后同步的场… |
| domain/bundled-models-fallback.md#陷阱 ❌ vs 正解 ✅ | domain | 陷阱 ❌ vs 正解 ✅ | bundled, include_str, OnceLock, 兜底, 冷启动 | auto | - | active | **陷阱1**：启动时 seed DB - ❌ `fn seed_models()` 启动期间 INSERT bundl… |
| domain/coding-plan-utilization-calib-fix-26.md#coding plan 订阅制平台普遍无公开用量查询 API | domain | coding plan 订阅制平台普遍无公开用量查询 API | coding-plan,quota,upstream-api,degrade,custom-quota-script | auto | - | active | bailian/qianfan/xiaomi/compshare 等 coding plan 订阅制平台上游均无公开程序… |
| domain/cpa-oauth-credential-format.md#Cross-ref | domain | Cross-ref | cpa,oauth,credential,cliproxyapi,access_token,model_aliases,xai,multi-account,凭据,导入 | auto | - | active / →auto-fix-downgrade-35,parser-multi-path-format-symmetry | - `src-tauri/crates/aidog_core/src/gateway/cpa_import/parser… |
| domain/cpa-oauth-credential-format.md#OAuth 类型枚举（CpaOAuthType） | domain | OAuth 类型枚举（CpaOAuthType） | cpa,oauth,credential,cliproxyapi,access_token,model_aliases,xai,multi-account,凭据,导入 | auto | - | active | codex / claude / kimi / xai / vertex / aistudio / antigravit… |
| domain/cpa-oauth-credential-format.md#多账号语义（CLIProxyAPI） | domain | 多账号语义（CLIProxyAPI） | cpa,oauth,credential,cliproxyapi,access_token,model_aliases,xai,multi-account,凭据,导入 | auto | - | active / →auto-fix-downgrade-35 | - 同一 OAuth 类型(如 xai)可有多个凭据(各 email 不同)→ **各自独立平台**(负载均衡) - d… |
| domain/cpa-oauth-credential-format.md#格式结构 | domain | 格式结构 | cpa,oauth,credential,cliproxyapi,access_token,model_aliases,xai,multi-account,凭据,导入 | auto | - | active | CLIProxyAPI OAuth 凭据 JSON(auth-dir 文件 / 导出 zip 内): ```json {… |
| domain/cpa-oauth-credential-format.md#识别逻辑 | domain | 识别逻辑 | cpa,oauth,credential,cliproxyapi,access_token,model_aliases,xai,multi-account,凭据,导入 | auto | - | active | - `parse_oauth_json(content) -> Option<Vec<CpaProvider>>`(pa… |
| domain/rule-51.md#关联 | domain | 关联 | protocol endpoint converter platform_type | auto | - | active / →rule-05,rule-53 | [[rule-05]] [[rule-53]] |
| domain/rule-51.md#关键不变量 | domain | 关键不变量 | protocol endpoint converter platform_type | auto | - | active | endpoint 协议 = converter 模块支持的格式（convert_request + parse_sse） |
| domain/rule-51.md#反例 | domain | 反例 | protocol endpoint converter platform_type | auto | - | active | - 把 glm/kimi/sensenova 当作 endpoint 协议 → 转换时 panic/未实现 - 误以为有… |
| domain/rule-51.md#案例 | domain | 案例 | protocol endpoint converter platform_type | auto | - | active | - converter-reasoning-content task：5 协议是 N×N 互转矩阵的锚点 - glm/k… |
| domain/rule-51.md#触发场景 | domain | 触发场景 | protocol endpoint converter platform_type | auto | - | active | - endpoint 协议层只 5 种（anthropic/openai/openai_responses/openai… |
| domain/rule-51.md#适用 | domain | 适用 | protocol endpoint converter platform_type | auto | - | active | - converter 模块扩展（新增 wire protocol） - N×N 协议互转设计（真值源） - 平台接入时… |
| domain/rule-51.md#陷阱-正解 | domain | 陷阱-正解 | protocol endpoint converter platform_type | auto | - | active | - ❌ 混淆：以为所有 Protocol 枚举值都是「协议」 - ✅ 区分：仅 5 个可作为 endpoint 协议参与… |
| domain/rule-52.md#关联 | domain | 关联 | reasoning thinking anthropic signature converter | auto | - | active / →rule-52,rule-53 | [[rule-53]] [[rule-52]] |
| domain/rule-52.md#决策背景 | domain | 决策背景 | reasoning thinking anthropic signature converter | auto | - | active | - TrueFoundry/LiteLLM #8927 调研佐证：第三方 reasoning 无 signature -… |
| domain/rule-52.md#反例 | domain | 反例 | reasoning thinking anthropic signature converter | auto | - | active | - 强行出 thinking 块 → CC 多轮交互时 400/empty or malformed - 空 reaso… |
| domain/rule-52.md#实现 | domain | 实现 | reasoning thinking anthropic signature converter | auto | - | active | - openai/response.rs:13：reasoning_content 被忽略，不影响 content/to… |
| domain/rule-52.md#触发场景 | domain | 触发场景 | reasoning thinking anthropic signature converter | auto | - | active | - 第三方（deepseek/sensenova/glm）reasoning_content 纯文本无 signatur… |
| domain/rule-52.md#适用 | domain | 适用 | reasoning thinking anthropic signature converter | auto | - | active | - 所有第三方 → anthropic 跨协议转换 - reasoning 扩展字段处理（未来第三方新增非标准字段） |
| domain/rule-52.md#陷阱-正解 | domain | 陷阱-正解 | reasoning thinking anthropic signature converter | auto | - | active | - ❌ 方案 A（标准协议）：出 thinking 块 → signature 风险 - ✅ 方案 B（务实方案）：re… |
| domain/rule-53.md#关联 | domain | 关联 | converter NonStreamResponse parse render protocol | auto | - | active / →rule-52,rule-54 | [[rule-52]] [[rule-54]] |
| domain/rule-53.md#反例 | domain | 反例 | converter NonStreamResponse parse render protocol | auto | - | active | - 点对点设计：新增协议时改 N 处 → O(N²) 维护成本 - 无中间归一：无法跨协议组合（如 openai→gem… |
| domain/rule-53.md#案例 | domain | 案例 | converter NonStreamResponse parse render protocol | auto | - | active | - converter-reasoning-content：5×5 互转矩阵用 NonStreamResponse - … |
| domain/rule-53.md#覆盖范围 | domain | 覆盖范围 | converter NonStreamResponse parse render protocol | auto | - | active | - 当前：openai → anthropic 真转换（convert_response） - 其余组合：回退透传（re… |
| domain/rule-53.md#触发场景 | domain | 触发场景 | converter NonStreamResponse parse render protocol | auto | - | active | - N 协议互转设计选择：内部归一（路 A）vs 点对点（路 B） - O(N) parse + render vs O… |
| domain/rule-53.md#设计决策 | domain | 设计决策 | converter NonStreamResponse parse render protocol | auto | - | active | 路 A（内部归一）： 1. 上游响应 → parse → NonStreamResponse（归一） 2. NonStr… |
| domain/rule-53.md#适用 | domain | 适用 | converter NonStreamResponse parse render protocol | auto | - | active | - converter 模块扩展（新增协议/转换组合） - N×N 互转矩阵设计（converter-reasoning… |
| domain/rule-53.md#陷阱-正解 | domain | 陷阱-正解 | converter NonStreamResponse parse render protocol | auto | - | active | - ❌ 路 B：点对点 N×N 函数 → 新增协议需加 N 个函数 - ✅ 路A：NonStreamResponse 作… |
| domain/rule-54.md#修复方案 | domain | 修复方案 | target_protocol platform_type matched_ep preset | auto | - | active | - is_valid_wire_protocol 白名单：5 协议（anthropic/openai/openai_re… |
| domain/rule-54.md#关联 | domain | 关联 | target_protocol platform_type matched_ep preset | auto | - | active / →rule-05 | [[rule-05]] |
| domain/rule-54.md#关键不变量 | domain | 关键不变量 | target_protocol platform_type matched_ep preset | auto | - | active | matched_ep=None 的合法情况：preset 未加载（DB endpoints 空），非用户配置错误 |
| domain/rule-54.md#反例 | domain | 反例 | target_protocol platform_type matched_ep preset | auto | - | active | - ❌ 误判：endpoint 配置缺 protocol → 实际是 DB endpoints 字段空 - ❌ 误修：加… |
| domain/rule-54.md#根因分析 | domain | 根因分析 | target_protocol platform_type matched_ep preset | auto | - | active | 1. matched_ep=None 时 `unwrap_or((&route.platform.platform_ty… |
| domain/rule-54.md#案例 | domain | 案例 | target_protocol platform_type matched_ep preset | auto | - | active | - converter-reasoning-content bug1：preset 未加载致 matched_ep=No… |
| domain/rule-54.md#触发场景 | domain | 触发场景 | target_protocol platform_type matched_ep preset | auto | - | active | - proxy_log.target_protocol 落平台名（如 "glm"）而非 endpoint 协议（如 "o… |
| domain/rule-54.md#适用 | domain | 适用 | target_protocol platform_type matched_ep preset | auto | - | active | - target_protocol 异常落平台名 - 新增 wire protocol 后 route fail - p… |
| domain/rule-55.md#关联 | domain | 关联 | - | auto | - | active | - |
| domain/rule-55.md#分层不变量 | domain | 分层不变量 | - | auto | - | active | - 回退仅在普通平台生效：普通平台允许跨协议回退（降低 502 率） - coding 平台永不落非 coding：步骤… |
| domain/rule-55.md#反例 | domain | 反例 | - | auto | - | active | - ❌ 误判：coding 平台也跨协议回退 → 破坏 401 防护 - ❌ 误修：只修普通平台回退，忘了 coding… |
| domain/rule-55.md#案例 | domain | 案例 | - | auto | - | active / →rule-06,rule-07 | - endpoint-cross-protocol-fallback task：普通平台步骤 4 泛化（同协议 > op… |
| domain/rule-55.md#触发场景 | domain | 触发场景 | - | auto | - | active | - 普通平台 endpoint 选择时协议不匹配（如 anthropic 入站 + 仅 openai endpoint）… |
| domain/rule-55.md#适用 | domain | 适用 | - | auto | - | active | - endpoint.rs select_endpoint_for_protocol 修改 - 跨协议回退逻辑扩展 - … |
| domain/rule-55.md#陷阱-正解 | domain | 陷阱-正解 | - | auto | - | active | **陷阱**: 误以为跨协议回退可应用于所有平台类型，或回退优先级混乱。  **正解**: 普通平台步骤 4 泛化为三级… |
| domain/rule-66.md#关联 | domain | 关联 | - | auto | - | active / →bundled-models-fallback,time-tiers-apply-idiom | [[time-tiers-apply-idiom]] [[bundled-models-fallback]] |
| domain/rule-66.md#案例 | domain | 案例 | - | auto | - | active | 原错 (billing.rs 未传参) → 日志字段时刻定价与当前时刻定价混杂 → 审计重放价格错 修后 → creat… |
| domain/rule-66.md#硬约束 | domain | 硬约束 | - | auto | - | active | `resolve_price` 新增末位参数 `now_ms: i64`，调用点按用途选传值：  / 调用点 / 传值 … |
| domain/rule-66.md#禁用 | domain | 禁用 | - | auto | - | active | ❌ 所有调用点统一传 0（会导致时段定价形同虚设） ❌ 测试传 `now()`（会让既有基准价断言失败） ❌ 签名改动后… |
| domain/time-tiers-apply-idiom.md#关联 | domain | 关联 | time_tiers, 定价分档, 嵌套价表, 时间维度 | auto | - | active / →bundled-models-fallback,rule-66,rule-67 | [[rule-66]] [[rule-67]] [[bundled-models-fallback]] |
| domain/time-tiers-apply-idiom.md#反例 | domain | 反例 | time_tiers, 定价分档, 嵌套价表, 时间维度 | auto | - | active | ```rust // ❌ 顺序首命中 + 扁平相加 let tier = tiers.iter().find(/t/ t… |
| domain/time-tiers-apply-idiom.md#案例 | domain | 案例 | time_tiers, 定价分档, 嵌套价表, 时间维度 | auto | - | active | **glm-5-turbo 时段+长文档**： - base: 32k 档 = 2e-6 $/token（普通价） - … |
| domain/time-tiers-apply-idiom.md#触发场景 | domain | 触发场景 | time_tiers, 定价分档, 嵌套价表, 时间维度 | auto | - | active | 模型定价加入时间维度（同一个模型不同时段不同价格）。需要表达二维定价：时间 + 内容长度。 |
| domain/time-tiers-apply-idiom.md#适用 | domain | 适用 | time_tiers, 定价分档, 嵌套价表, 时间维度 | auto | - | active | - 模型单价时间分档（glm_coding 早高峰 ×3.0 + 0-24 ×2.0） - 平台级时段价（某云商服务某时… |
| domain/time-tiers-apply-idiom.md#陷阱 ❌ vs 正解 ✅ | domain | 陷阱 ❌ vs 正解 ✅ | time_tiers, 定价分档, 嵌套价表, 时间维度 | auto | - | active | **陷阱1**：time_tiers 数组用顺序首命中 - ❌ `tiers[0]` 如果 start_at 符合就用，… |
| domain/trellis-06.md#Config Carrier — extra.mock (MUST) | domain | Config Carrier — extra.mock (MUST) | mock,platform,extra,test,builder,error_mode | auto | - | active | - mock 配置载体必须为现有 `platform.extra`（TEXT JSON 列），禁新增专用 DB 列（零迁… |
| domain/trellis-06.md#Response Builders (MUST) | domain | Response Builders (MUST) | mock,platform,extra,test,builder,error_mode | auto | - | active | - 非流式: `build_response(cfg, source_protocol, model)` 按 5 协议返… |
| domain/trellis-06.md#Three-Layer Config Override (MUST) | domain | Three-Layer Config Override (MUST) | mock,platform,extra,test,builder,error_mode | auto | - | active | 最终生效值 = 逐字段按优先级取首个存在者（`resolve_mock_config(extra, chat_req, … |
| domain/trellis-06.md#Verification | domain | Verification | mock,platform,extra,test,builder,error_mode | auto | - | active | ```bash cd src-tauri && cargo test mock   # 全部通过（三层覆盖 / 5 协议… |
| domain/trellis-06.md#What & When (MUST) | domain | What & When (MUST) | mock,platform,extra,test,builder,error_mode | auto | - | active | - `Protocol::Mock`（`models.rs`，serde rename `"mock"`）是**平台主类… |
| domain/trellis-06.md#error_mode Semantics (MUST) | domain | error_mode Semantics (MUST) | mock,platform,extra,test,builder,error_mode | auto | - | active | `handle_mock`（proxy.rs）按 `error_mode` 分派，两类语义并存（delay 与 erro… |
| domain/trellis-06.md#proxy_log (MUST) | domain | proxy_log (MUST) | mock,platform,extra,test,builder,error_mode | auto | - | active | - mock 分支直接写最终生效值 `log.{input_tokens,output_tokens,cache_tok… |
| domain/trellis-07.md#Frontend (MUST) | domain | Frontend (MUST) | claude,passthrough,透传,subscription,header | auto | - | active | - `api.ts` Protocol union 含 `/ "claude_code"` - `Platforms.t… |
| domain/trellis-07.md#Intercept Point (MUST) | domain | Intercept Point (MUST) | claude,passthrough,透传,subscription,header | auto | - | active | - 拦截点：`select_platform` 之后、`convert_request` 之前（与 mock 拦截点同区… |
| domain/trellis-07.md#No Transform / No Inject (MUST) | domain | No Transform / No Inject (MUST) | claude,passthrough,透传,subscription,header | auto | - | active | - 禁 `convert_request` / 禁 `build_upstream_headers` / 禁 `appl… |
| domain/trellis-07.md#Original Request Capture (MUST) | domain | Original Request Capture (MUST) | claude,passthrough,透传,subscription,header | auto | - | active | - `proxy.rs` handle_proxy 在 `req.into_parts()` **之前**捕获原始量（对… |
| domain/trellis-07.md#Verification | domain | Verification | claude,passthrough,透传,subscription,header | auto | - | active | ```bash cd src-tauri && cargo test passthrough   # URL 拼接 / … |
| domain/trellis-07.md#What & When (MUST) | domain | What & When (MUST) | claude,passthrough,透传,subscription,header | auto | - | active | - `Protocol::ClaudeCode`（`models.rs`，serde rename `"claude_c… |
| domain/trellis-07.md#handle_passthrough Semantics (MUST) | domain | handle_passthrough Semantics (MUST) | claude,passthrough,透传,subscription,header | auto | - | active | 1. **目标 URL** = `base_url` + 客户端原始 path（+ query）。**约定 CC 平台 … |
| domain/trellis-07.md#proxy_log (MUST) | domain | proxy_log (MUST) | claude,passthrough,透传,subscription,header | auto | - | active | - 透传分支**正常记** `proxy_log`：   - `source_protocol` = `target_p… |
| domain/trellis-08.md#C1 — auto_disable 触发状态码 (MUST) | domain | C1 — auto_disable 触发状态码 (MUST) | platform,error,429,auto_disable,熔断,purge,stream,status | auto | - | active | `non_success.rs` handle_non_success 中, 上游非 2xx 仅以下触发 `set_pl… |
| domain/trellis-08.md#C2 — 429 分类只看 message 文本 (MUST NOT 按 error.type) | domain | C2 — 429 分类只看 message 文本 (MUST NOT 按 error.type) | platform,error,429,auto_disable,熔断,purge,stream,status | auto | - | active | `classify_429(message) -> bool`(retry.rs)区分 429:  - **配额耗尽**… |
| domain/trellis-08.md#C3 — 熔断与 auto_disable 解耦 (MUST) | domain | C3 — 熔断与 auto_disable 解耦 (MUST) | platform,error,429,auto_disable,熔断,purge,stream,status | auto | - | active | 熔断计数(`record_failure` vs `record_ignored`)按下表:  / 错误 / 熔断 / … |
| domain/trellis-08.md#C4 — purge 只删 401/403 或已过期 (MUST) | domain | C4 — purge 只删 401/403 或已过期 (MUST) | platform,error,429,auto_disable,熔断,purge,stream,status | auto | - | active | `purge_auto_disabled_platforms`(platform_lifecycle.rs)全局 + 分… |
| domain/trellis-08.md#C5 — last_error 优先存 message 不存完整 body (MUST) | domain | C5 — last_error 优先存 message 不存完整 body (MUST) | platform,error,429,auto_disable,熔断,purge,stream,status | auto | - | active | 写 `set_platform_last_error` 前用 `extract_error_message(body)`… |
| domain/trellis-08.md#C6 — stream 字段单向性：禁用 unwrap_or(false) 区分漏发与显式非流式 (MUST) | domain | C6 — stream 字段单向性：禁用 unwrap_or(false) 区分漏发与显式非流式 (MUST) | platform,error,429,auto_disable,熔断,purge,stream,status | auto | - | active | **背景**：DB 全库实证（2026-07-02）—— 客户端（Claude Code）stream 字段是**单向*… |
| domain/trellis-08.md#C7 — 空流/空body 失败时 response_body MUST 落上游真实首块 (MUST) | domain | C7 — 空流/空body 失败时 response_body MUST 落上游真实首块 (MUST) | platform,error,429,auto_disable,熔断,purge,stream,status | auto | - | active | **背景**：proxy 流式 peek 判 `EmptyOrError`（上游 200 但流无内容/秒断/立即[DON… |
| domain/trellis-09.md#delete_platform 契约 | domain | delete_platform 契约 | platform,delete,软删,group_platform,purge,lifecycle | auto | - | active | `delete_platform(db, id)`（`src-tauri/src/gateway/db/platform… |
| domain/trellis-09.md#purge_auto_disabled_platforms | domain | purge_auto_disabled_platforms | platform,delete,软删,group_platform,purge,lifecycle | auto | - | active | 复用 `delete_platform` 的语义，**不重写关联清理逻辑**：  - **全局（`group_id = … |
| domain/trellis-09.md#purge_old_soft_deleted_platforms | domain | purge_old_soft_deleted_platforms | platform,delete,软删,group_platform,purge,lifecycle | auto | - | active | 定时任务（每日）：物理删除 `deleted_at > 0 AND deleted_at < now() - older… |
| domain/trellis-09.md#测试契约（test_platform_lifecycle.rs） | domain | 测试契约（test_platform_lifecycle.rs） | platform,delete,软删,group_platform,purge,lifecycle | auto | - | active | - `delete_platform_preserves_groups_with_other_members`：手动组 … |
| domain/trellis-10.md#HTTP client (MUST) | domain | HTTP client (MUST) | logo,sync,favicon,simpleicons,clearbit,png | auto | - | active | - **MUST 复用 `build_http_client_system`** (非 `build_http_clie… |
| domain/trellis-10.md#presets JSON 读取 (MUST) | domain | presets JSON 读取 (MUST) | logo,sync,favicon,simpleicons,clearbit,png | auto | - | active | - `read_local_presets_json` 优先级: `~/.aidog/platform-presets.… |
| domain/trellis-10.md#三路 fallback 顺序 (MUST, 首成功即止) | domain | 三路 fallback 顺序 (MUST, 首成功即止) | logo,sync,favicon,simpleicons,clearbit,png | auto | - | active | 固定顺序, **禁重排**, 见 `sync_one_into`:  1. **simpleicons CDN** — … |
| domain/trellis-10.md#入口 | domain | 入口 | logo,sync,favicon,simpleicons,clearbit,png | auto | - | active | - `sync_all_logos(db, app_data_dir)` — 后台批量同步 (app 启动 / 手动触发… |
| domain/trellis-10.md#关联 | domain | 关联 | logo,sync,favicon,simpleicons,clearbit,png | auto | - | active | - [http-client-forward.md](./http-client-forward.md) — build… |
| domain/trellis-10.md#缓存契约 (MUST) | domain | 缓存契约 (MUST) | logo,sync,favicon,simpleicons,clearbit,png | auto | - | active | - 缓存路径 `~/.aidog/logos/<protocol_id>.png` (`logo_cache_path`… |
| domain/trellis-10.md#验收基准 (可复用) | domain | 验收基准 (可复用) | logo,sync,favicon,simpleicons,clearbit,png | auto | - | active | - [ ] 清空 `~/.aidog/logos/` 后, 有 `logo_url` 的 protocol 命中路 1;… |
| domain/trellis-10.md#验证命令 | domain | 验证命令 | logo,sync,favicon,simpleicons,clearbit,png | auto | - | active | ```bash # 三路 URL 模板存在且顺序 grep -n "cdn.simpleicons.org\//favi… |
| encoding/trellis-21.md#MUST | encoding | MUST | json,script,application/json,parse,template,embedding,序列化 | auto | - | active | `<script type="application/json">` 的 textContent 是 **raw tex… |
| encoding/trellis-21.md#MUST NOT | encoding | MUST NOT | json,script,application/json,parse,template,embedding,序列化 | auto | - | active | - 禁对嵌入 script 的 JSON payload 用任何 HTML 实体转义（`html.escape` / `… |
| encoding/trellis-21.md#Verification | encoding | Verification | json,script,application/json,parse,template,embedding,序列化 | auto | - | active | ```bash # 抽取嵌入 JSON + 校验可解析 + 无实体 python3 -c " import json, … |
| encoding/trellis-21.md#踩坑来源 | encoding | 踩坑来源 | json,script,application/json,parse,template,embedding,序列化 | auto | - | active | task `07-07-presets-html-json-escape-fix`：`scripts/presets_v… |
| frontend/auto-fix-downgrade-37.md#MUST 用 Tauri onDragDropEvent，禁 HTML5 onDrop | frontend | MUST 用 Tauri onDragDropEvent，禁 HTML5 onDrop | tauri,drag,drop,wkwebview,html5,ondragdropevent | auto | - | active | macOS WKWebView 的 HTML5 `drop` 事件不触发。Tauri `getCurrentWebvie… |
| frontend/auto-fix-downgrade-37.md#event.payload.type | frontend | event.payload.type | tauri,drag,drop,wkwebview,html5,ondragdropevent | auto | - | active | - enter/over: paths[] → 高亮判断 - drop: paths[] → 取目标文件 - leave… |
| frontend/auto-fix-downgrade-37.md#关联 | frontend | 关联 | tauri,drag,drop,wkwebview,html5,ondragdropevent | auto | - | active / →modal-state-architecture | [[modal-state-architecture]] (Tauri UI 约束) |
| frontend/auto-fix-downgrade-37.md#约束 | frontend | 约束 | tauri,drag,drop,wkwebview,html5,ondragdropevent | auto | - | active | - 禁混 HTML5 onDrop（macOS WKWebView 不触发） - MUST unlisten（clean… |
| frontend/auto-fix-downgrade-37.md#范本 | frontend | 范本 | tauri,drag,drop,wkwebview,html5,ondragdropevent | auto | - | active | ```typescript useEffect(() => {   let unlisten: (() => void)… |
| frontend/auto-fix-downgrade-37.md#触发场景 | frontend | 触发场景 | tauri,drag,drop,wkwebview,html5,ondragdropevent | auto | - | active | Tauri 前端实现文件拖拽导入时。 |
| frontend/auto-fix-downgrade-37.md#适用 | frontend | 适用 | tauri,drag,drop,wkwebview,html5,ondragdropevent | auto | - | active | Tauri 文件拖拽导入、跨平台拖拽 |
| frontend/cpa-drag-import-22.md#WKWebView 退化（best-effort） | frontend | WKWebView 退化（best-effort） | authdir,dragtarget,ondragenter,wkwebview,best-effort,退化,DOM target | auto | - | active | macOS WKWebView HTML5 `drop` 不触发，`onDragEnter` **可能同病**（未实测）… |
| frontend/cpa-drag-import-22.md#关联 | frontend | 关联 | authdir,dragtarget,ondragenter,wkwebview,best-effort,退化,DOM target | auto | - | active | - core/frontend/tauri-drag-drop-api.md（依赖） |
| frontend/cpa-drag-import-22.md#模式: HTML5 onDragEnter 标记 + Tauri drop 读 ref | frontend | 模式: HTML5 onDragEnter 标记 + Tauri drop 读 ref | authdir,dragtarget,ondragenter,wkwebview,best-effort,退化,DOM target | auto | - | active | ```typescript const dragTargetRef = useRef<"source" / "authd… |
| frontend/cpa-drag-import-22.md#问题: Tauri onDragDropEvent 无 DOM target | frontend | 问题: Tauri onDragDropEvent 无 DOM target | authdir,dragtarget,ondragenter,wkwebview,best-effort,退化,DOM target | auto | - | active | `onDragDropEvent` 是 webview 级事件，payload **不含 DOM target 信息**… |
| frontend/cpa-drag-import-23.md#模式: baseIdx 全局偏移（orderLenRef） | frontend | 模式: baseIdx 全局偏移（orderLenRef） | rowid,unique,多源,import,baseidx,偏移,batch,react key | auto | - | active | ```typescript const orderLenRef = useRef(0);  const parseAnd… |
| frontend/cpa-drag-import-23.md#清理 | frontend | 清理 | rowid,unique,多源,import,baseidx,偏移,batch,react key | auto | - | active | modal 关闭重置 `orderLenRef.current = 0`，下次打开从 0 起。 |
| frontend/cpa-drag-import-23.md#问题: 跨源 rowId 撞 id | frontend | 问题: 跨源 rowId 撞 id | rowid,unique,多源,import,baseidx,偏移,batch,react key | auto | - | active | 每源 rowId 从 `${0}::` 起递增，不同源同索引条目撞 id。 |
| frontend/cpa-drag-import-23.md#验收 | frontend | 验收 | rowid,unique,多源,import,baseidx,偏移,batch,react key | auto | - | active | - [ ] 多源 drop → 所有条目 rowId 唯一 - [ ] modal 重开 → orderLenRef 清… |
| frontend/cpa-drag-import-24.md#模式: useRef 计数（parseInFlightRef） | frontend | 模式: useRef 计数（parseInFlightRef） | parseinflight,concurrent,多源,异步,ref,计数,loading,boolean | auto | - | active | ```typescript const parseInFlightRef = useRef(0);  const par… |
| frontend/cpa-drag-import-24.md#清理 | frontend | 清理 | parseinflight,concurrent,多源,异步,ref,计数,loading,boolean | auto | - | active | modal 关闭 `parseInFlightRef.current = 0; setParsing(false)`。 |
| frontend/cpa-drag-import-24.md#问题: boolean 无法表达「任一在解析」 | frontend | 问题: boolean 无法表达「任一在解析」 | parseinflight,concurrent,多源,异步,ref,计数,loading,boolean | auto | - | active | 源 A 完成设 false，源 B 还在跑但 UI 已显示非解析态。互斥锁过重（JS 单线程无需真锁）。 |
| frontend/cpa-drag-import-24.md#验收 | frontend | 验收 | parseinflight,concurrent,多源,异步,ref,计数,loading,boolean | auto | - | active | - [ ] 快速拖 N 源 → parsing 恒 true 直到全完 - [ ] 某源失败 → 其他继续，最后完成才 … |
| frontend/dirty-float-hour-normalization.md#关联 | frontend | 关联 | 脏数据,浮点,归一,Number.isInteger,splitFraction,平台兼容性 | auto | - | active / →module-load-time-constant-test-rule,time-zone-minute-arithmetic | [[time-zone-minute-arithmetic]] (时区换算硬约束) · [[module-load-ti… |
| frontend/dirty-float-hour-normalization.md#反例 / 常见错误 | frontend | 反例 / 常见错误 | 脏数据,浮点,归一,Number.isInteger,splitFraction,平台兼容性 | auto | - | active | / 错误                          / 为什么错                        … |
| frontend/dirty-float-hour-normalization.md#案例 | frontend | 案例 | 脏数据,浮点,归一,Number.isInteger,splitFraction,平台兼容性 | auto | - | active | - time-models-timezone task (commit d5b00753) — normalizeWin… |
| frontend/dirty-float-hour-normalization.md#正解：前端读取路径归一（关键） | frontend | 正解：前端读取路径归一（关键） | 脏数据,浮点,归一,Number.isInteger,splitFraction,平台兼容性 | auto | - | active | ### MUST 单点归一（parse 层）  ```ts /** 存量非整数 start_hour/end_hour（… |
| frontend/dirty-float-hour-normalization.md#落地 checklist | frontend | 落地 checklist | 脏数据,浮点,归一,Number.isInteger,splitFraction,平台兼容性 | auto | - | active | ```bash # 1. 验证 normalizeWindow 实现 grep -A15 "export functio… |
| frontend/dirty-float-hour-normalization.md#触发场景 | frontend | 触发场景 | 脏数据,浮点,归一,Number.isInteger,splitFraction,平台兼容性 | auto | - | active | 系统升级或跨版本迁移中，存量数据可能包含不符合当前数据格式的脏数据。例如，旧版本按整小时换算时产生 `start_hou… |
| frontend/dirty-float-hour-normalization.md#适用 | frontend | 适用 | 脏数据,浮点,归一,Number.isInteger,splitFraction,平台兼容性 | auto | - | active | - 版本升级中的数据兼容性问题 - 存量脏数据前端吸收而非后端永久兼容 |
| frontend/dirty-float-hour-normalization.md#陷阱：后端 migration 改 serde 类型成本高，数据污染持久 | frontend | 陷阱：后端 migration 改 serde 类型成本高，数据污染持久 | 脏数据,浮点,归一,Number.isInteger,splitFraction,平台兼容性 | auto | - | active | > 旧版本：`peak_hours` 整小时换算，半时区用户产生 `start_hour: 8.5` 写入 JSON。后… |
| frontend/dirty-float-hour-normalization.md#验证场景 | frontend | 验证场景 | 脏数据,浮点,归一,Number.isInteger,splitFraction,平台兼容性 | auto | - | active | - 升级前存量：`{ start_hour: 8.5, end_hour: 20 }`（脏数据） - 加载时：norma… |
| frontend/form-level-tz-state-sharing.md#关联 | frontend | 关联 | 表单设计,状态管理,时区模式,prop 透传,单一真值源,多组件一致性 | auto | - | active / →rule-04,time-zone-minute-arithmetic | [[time-zone-minute-arithmetic]] · [[rule-04]] |
| frontend/form-level-tz-state-sharing.md#反例 / 常见错误 | frontend | 反例 / 常见错误 | 表单设计,状态管理,时区模式,prop 透传,单一真值源,多组件一致性 | auto | - | active | / 错误                          / 为什么错                        … |
| frontend/form-level-tz-state-sharing.md#案例 | frontend | 案例 | 表单设计,状态管理,时区模式,prop 透传,单一真值源,多组件一致性 | auto | - | active | - time-models-timezone task (commit 7f78c93e) — peakHoursTz … |
| frontend/form-level-tz-state-sharing.md#正解：表单级单一 state 透传（硬约束，关键） | frontend | 正解：表单级单一 state 透传（硬约束，关键） | 表单设计,状态管理,时区模式,prop 透传,单一真值源,多组件一致性 | auto | - | active | ### MUST 单一真值源（usePlatformForm hook）  ```ts // usePlatformFo… |
| frontend/form-level-tz-state-sharing.md#落地 checklist | frontend | 落地 checklist | 表单设计,状态管理,时区模式,prop 透传,单一真值源,多组件一致性 | auto | - | active | ```bash # 1. 验证单一真值源（usePlatformForm.ts 唯一声明） grep -n "windo… |
| frontend/form-level-tz-state-sharing.md#触发场景 | frontend | 触发场景 | 表单设计,状态管理,时区模式,prop 透传,单一真值源,多组件一致性 | auto | - | active | 同一表单内多个组件展示同一类数据的不同维度（如 peak_hours 编辑器 + time_models 编辑器，都展示… |
| frontend/form-level-tz-state-sharing.md#适用 | frontend | 适用 | 表单设计,状态管理,时区模式,prop 透传,单一真值源,多组件一致性 | auto | - | active | - 同表单内多组件展示同一维度的数据（时区、主题、排序） - 跨页面 UI state 需一致性同步 |
| frontend/form-level-tz-state-sharing.md#陷阱：各组件独立 state 导致口径漂移 | frontend | 陷阱：各组件独立 state 导致口径漂移 | 表单设计,状态管理,时区模式,prop 透传,单一真值源,多组件一致性 | auto | - | active | > `PlatformEditForm` 编辑单个平台配置。peak_hours 与 time_models 都含「时段… |
| frontend/form-level-tz-state-sharing.md#验证场景 | frontend | 验证场景 | 表单设计,状态管理,时区模式,prop 透传,单一真值源,多组件一致性 | auto | - | active | 1. 用户勾选「本地时区」→ peak_hours 显示本地、time_models 也显示本地 ✅ 2. 用户切到「U… |
| frontend/modal-state-architecture.md#两类 Modal 区分 | frontend | 两类 Modal 区分 | modal, state, architecture, PlatformEditForm, usePlatformForm, PlatformPasteCtx, CpaImportModal, SmartPasteModal | auto | - | active | ### 直接灌表单 Modal（SmartPasteModal 模式） - **State 位置**: `usePlat… |
| frontend/modal-state-architecture.md#后续新 Modal 决策树 | frontend | 后续新 Modal 决策树 | modal, state, architecture, PlatformEditForm, usePlatformForm, PlatformPasteCtx, CpaImportModal, SmartPasteModal | auto | - | active | ``` 新 Modal (如 Sub2Api) ├─ onApply 直接填表单字段？ │  └─ 是 → SmartP… |
| frontend/modal-state-architecture.md#架构原则 | frontend | 架构原则 | modal, state, architecture, PlatformEditForm, usePlatformForm, PlatformPasteCtx, CpaImportModal, SmartPasteModal | auto | - | active | 1. **Modal 直接操作表单字段 → state 放 hook，通过 PlatformPasteCtx 传 set… |
| frontend/modal-state-architecture.md#验收 | frontend | 验收 | modal, state, architecture, PlatformEditForm, usePlatformForm, PlatformPasteCtx, CpaImportModal, SmartPasteModal | auto | - | active | - [ ] grep `showCpaImport` / `showPaste` 在 PlatformEditForm … |
| frontend/platform-creation-entry-consolidation.md#cli-proxy 平台创建入口唯一性 | frontend | cli-proxy 平台创建入口唯一性 | cli-proxy,平台创建,表单设计,入口收敛 | auto | - | active | - |
| frontend/platform-creation-entry-consolidation.md#关联 | frontend | 关联 | cli-proxy,平台创建,表单设计,入口收敛 | auto | - | active / →i18n-key-deletion-safety | [[i18n-key-deletion-safety]] |
| frontend/platform-creation-entry-consolidation.md#反例 | frontend | 反例 | cli-proxy,平台创建,表单设计,入口收敛 | auto | - | active | ❌ 在 PlatformEditForm 新建态混入「从 cli-proxy 导入」选项 → 导致创建路径分裂，后续改表… |
| frontend/platform-creation-entry-consolidation.md#正解 | frontend | 正解 | cli-proxy,平台创建,表单设计,入口收敛 | auto | - | active | - 添加平台表单（PlatformEditForm）只用于编辑现有平台 - 创建新 cli-proxy 平台必须走 Cl… |
| frontend/platform-creation-entry-consolidation.md#约束 | frontend | 约束 | cli-proxy,平台创建,表单设计,入口收敛 | auto | - | active | cli-proxy 平台的唯一创建入口是 **CliProxy 页 src/pages/CliProxy/index.t… |
| frontend/platform-creation-entry-consolidation.md#触发场景 | frontend | 触发场景 | cli-proxy,平台创建,表单设计,入口收敛 | auto | - | active | cli-proxy 平台的创建路径需要统一化，避免表单旁路导致的创建入口分裂。 |
| frontend/platform-creation-entry-consolidation.md#适用 | frontend | 适用 | cli-proxy,平台创建,表单设计,入口收敛 | auto | - | active | - CLI Proxy 平台管理流程设计 - 添加平台表单重构 |
| frontend/semantic-token-foreground-pairing.md#判据 | frontend | 判据 | 语义色,token,foreground,对比度,contrast,accent,wcag,配对 | auto | - | active | 任何语义色 `bg-X` token 都必须配达标对比度的 `--X-foreground`。frontend-comp… |
| frontend/semantic-token-foreground-pairing.md#案例 | frontend | 案例 | 语义色,token,foreground,对比度,contrast,accent,wcag,配对 | auto | - | active | frontend-compositing-purge task 对比度审计：dark `--accent-foregro… |
| frontend/semantic-token-foreground-pairing.md#正解 | frontend | 正解 | 语义色,token,foreground,对比度,contrast,accent,wcag,配对 | auto | - | active | 修对比度缺陷时**禁改 `--accent` 等语义色 token 的值本身**，只能改配对的 `-foreground… |
| frontend/semantic-token-foreground-pairing.md#语义色 token 必须成对达标, --accent 本值禁改 | frontend | 语义色 token 必须成对达标, --accent 本值禁改 | 语义色,token,foreground,对比度,contrast,accent,wcag,配对 | auto | - | active | - |
| frontend/semantic-token-foreground-pairing.md#适用 | frontend | 适用 | 语义色,token,foreground,对比度,contrast,accent,wcag,配对 | auto | - | active | 本项目（aidog）任何涉及语义色 token 新增/审计对比度时；同族已有跨项目规则（通用禁写死 #fff/#000 … |
| frontend/semantic-token-foreground-pairing.md#陷阱 | frontend | 陷阱 | 语义色,token,foreground,对比度,contrast,accent,wcag,配对 | auto | - | active | 本项目 `--accent` 语义**不等于** shadcn 惯例（shadcn 里 accent 通常是低调 hov… |
| frontend/shadcn-infra-30.md#关联 | frontend | 关联 | css,var,alias,live-resolution,migration | auto | - | active / →shadcn-infra-02 | [[shadcn-infra-02]] (同任务 Tailwind 约束) |
| frontend/shadcn-infra-30.md#对比 | frontend | 对比 | css,var,alias,live-resolution,migration | auto | - | active | / 方式 / 改动量 / 误伤风险 / 回滚 / /------/--------/---------/------/ … |
| frontend/shadcn-infra-30.md#技巧 | frontend | 技巧 | css,var,alias,live-resolution,migration | auto | - | active | CSS 变量改名时，用 :root 定义别名层实现 live resolution，替代批量 sed 替换（零误伤、可回… |
| frontend/shadcn-infra-30.md#案例 | frontend | 案例 | css,var,alias,live-resolution,migration | auto | - | active | - shadcn-infra task: 主题变量改名用别名层，globals.css 加 10 行 vs sed 70… |
| frontend/shadcn-infra-30.md#正解 | frontend | 正解 | css,var,alias,live-resolution,migration | auto | - | active | 1. 在 :root 定义别名：`--legacy: var(--shadcn);` 2. 所有引用用旧名 `--leg… |
| frontend/shadcn-infra-30.md#适用 | frontend | 适用 | css,var,alias,live-resolution,migration | auto | - | active | CSS 变量迁移、主题重构、大型 CSS 重构中间状态 |
| frontend/shadcn-infra-31.md#关联 | frontend | 关联 | shadcn,theme,token,runtime,css,var | auto | - | active / →shadcn-infra-28,shadcn-infra-30 | [[shadcn-infra-30]] (同任务 CSS 技巧) [[shadcn-infra-28]] (shadcn… |
| frontend/shadcn-infra-31.md#反例 | frontend | 反例 | shadcn,theme,token,runtime,css,var | auto | - | active | ❌ 用 !important 覆盖所有 token → 优先级混乱 ❌ 依赖静态 @import → 运行时无法切换 |
| frontend/shadcn-infra-31.md#技巧 | frontend | 技巧 | shadcn,theme,token,runtime,css,var | auto | - | active | shadcn 主题 token 在运行时动态切换时，用 `applyTheme` + `setProperty` inl… |
| frontend/shadcn-infra-31.md#案例 | frontend | 案例 | shadcn,theme,token,runtime,css,var | auto | - | active | - shadcn-infra task: 运行时主题切换用 setProperty inline，避免 !importa… |
| frontend/shadcn-infra-31.md#正解 | frontend | 正解 | shadcn,theme,token,runtime,css,var | auto | - | active | 1. applyTheme 函数直接设置 CSS var：    ```ts    document.documentE… |
| frontend/shadcn-infra-31.md#适用 | frontend | 适用 | shadcn,theme,token,runtime,css,var | auto | - | active | shadcn 主题运行时切换、动态主题系统、CSS var 运行时更新 |
| frontend/shadcn-infra-31.md#陷阱 | frontend | 陷阱 | shadcn,theme,token,runtime,css,var | auto | - | active | - **陷阱**: 用 !important 强制覆盖 → 级联爆炸、难以维护 - **陷阱**: 依赖 @import… |
| frontend/tailwind-cascade-layer-unlayered.md#CSS cascade layer: 裸写规则反压 layer 内 utility | frontend | CSS cascade layer: 裸写规则反压 layer 内 utility | tailwind,cascade-layer,unlayered,layer,preflight,cascade,css | auto | - | active | - |
| frontend/tailwind-cascade-layer-unlayered.md#判据 | frontend | 判据 | tailwind,cascade-layer,unlayered,layer,preflight,cascade,css | auto | - | active | Tailwind v4 项目里若 `globals.css` 顶部声明了 `@layer theme, base, co… |
| frontend/tailwind-cascade-layer-unlayered.md#案例 | frontend | 案例 | tailwind,cascade-layer,unlayered,layer,preflight,cascade,css | auto | - | active | frontend-compositing-purge task：commit c3f9515e 裸写 UA reset … |
| frontend/tailwind-cascade-layer-unlayered.md#检查 | frontend | 检查 | tailwind,cascade-layer,unlayered,layer,preflight,cascade,css | auto | - | active | globals.css 顶部若见 `@layer <names>;` 声明 + `@import ... layer(.… |
| frontend/tailwind-cascade-layer-unlayered.md#正解 | frontend | 正解 | tailwind,cascade-layer,unlayered,layer,preflight,cascade,css | auto | - | active | 补 UA reset 规则必须包进 `@layer base { }`，与 globals.css 顶部声明的 laye… |
| frontend/tailwind-cascade-layer-unlayered.md#适用 | frontend | 适用 | tailwind,cascade-layer,unlayered,layer,preflight,cascade,css | auto | - | active | Tailwind v4 + cascade layer 项目，补 preflight/UA reset 规则、新增全局元… |
| frontend/tailwind-cascade-layer-unlayered.md#陷阱 | frontend | 陷阱 | tailwind,cascade-layer,unlayered,layer,preflight,cascade,css | auto | - | active | 补 preflight 缺失的 UA reset（如 `button,input,select,textarea { c… |
| frontend/theme-dark-class-dead-code.md#关联 | frontend | 关联 | theme,dark,applyTheme,data-mode,classList,tailwind,dark-mode,color-scheme | auto | - | active / →shadcn-infra-31 | [[shadcn-infra-31]]（同类 shadcn 主题运行时切换技巧，本条补充"本项目未用 classList… |
| frontend/theme-dark-class-dead-code.md#判据 | frontend | 判据 | theme,dark,applyTheme,data-mode,classList,tailwind,dark-mode,color-scheme | auto | - | active | 本项目主题机制：`src/themes/index.ts::applyTheme` 只做两件事——`applyTheme… |
| frontend/theme-dark-class-dead-code.md#本项目主题机制: data-mode 属性驱动, dark: utility 死代码 | frontend | 本项目主题机制: data-mode 属性驱动, dark: utility 死代码 | theme,dark,applyTheme,data-mode,classList,tailwind,dark-mode,color-scheme | auto | - | active | - |
| frontend/theme-dark-class-dead-code.md#案例 | frontend | 案例 | theme,dark,applyTheme,data-mode,classList,tailwind,dark-mode,color-scheme | auto | - | active | frontend-compositing-purge task 审计发现 `field.tsx:120`、`alert.… |
| frontend/theme-dark-class-dead-code.md#正解 | frontend | 正解 | theme,dark,applyTheme,data-mode,classList,tailwind,dark-mode,color-scheme | auto | - | active | 判本项目深色态必须看 `mono.ts` 的 `dark` 块或 `:root[data-mode="dark"]` 选… |
| frontend/theme-dark-class-dead-code.md#适用 | frontend | 适用 | theme,dark,applyTheme,data-mode,classList,tailwind,dark-mode,color-scheme | auto | - | active | 本项目（aidog）任何涉及深色态样式判断/新增组件暗色适配时；planning 阶段先查 `src/themes/in… |
| frontend/theme-dark-class-dead-code.md#陷阱 | frontend | 陷阱 | theme,dark,applyTheme,data-mode,classList,tailwind,dark-mode,color-scheme | auto | - | active | 故 globals.css 里 `@custom-variant dark (&:is(.dark *))` 与 `.d… |
| frontend/time-zone-minute-arithmetic.md#关联 | frontend | 关联 | 时区,换算,分钟精度,半时区,+5:30,澳门,DST,shiftClock,modulo | auto | - | active / →dirty-float-hour-normalization,rule-04 | [[rule-04]] (i18n key 齐平) · [[dirty-float-hour-normalization… |
| frontend/time-zone-minute-arithmetic.md#反例 / 常见错误 | frontend | 反例 / 常见错误 | 时区,换算,分钟精度,半时区,+5:30,澳门,DST,shiftClock,modulo | auto | - | active | / 错误                        / 为什么错                          … |
| frontend/time-zone-minute-arithmetic.md#案例 | frontend | 案例 | 时区,换算,分钟精度,半时区,+5:30,澳门,DST,shiftClock,modulo | auto | - | active | - time-models-timezone task (commit 7f78c93e) — peak_hours 侧… |
| frontend/time-zone-minute-arithmetic.md#正解：绝对分钟 modulo 1440（硬约束，关键） | frontend | 正解：绝对分钟 modulo 1440（硬约束，关键） | 时区,换算,分钟精度,半时区,+5:30,澳门,DST,shiftClock,modulo | auto | - | active | ### MUST 换算公式（单位：分钟）  ```ts /** 时钟平移纯函数内核 — offset 显式入参，可被单测… |
| frontend/time-zone-minute-arithmetic.md#落地 checklist | frontend | 落地 checklist | 时区,换算,分钟精度,半时区,+5:30,澳门,DST,shiftClock,modulo | auto | - | active | ```bash # 1. 验证 shiftClock 实现（必须绝对分钟） cd src && grep -A5 "fu… |
| frontend/time-zone-minute-arithmetic.md#触发场景 | frontend | 触发场景 | 时区,换算,分钟精度,半时区,+5:30,澳门,DST,shiftClock,modulo | auto | - | active | 前端时区显示/输入交互（peak_hours / time_models 的时段编辑器）需与服务端一致，半时区用户（印度… |
| frontend/time-zone-minute-arithmetic.md#适用 | frontend | 适用 | 时区,换算,分钟精度,半时区,+5:30,澳门,DST,shiftClock,modulo | auto | - | active | - 时段编辑器（peak_hours / time_models）时区展示/输入 - 任何需要精确分钟级时区换算的前端交… |
| frontend/time-zone-minute-arithmetic.md#陷阱：按整小时换算产生非整数 hour（旧错误） | frontend | 陷阱：按整小时换算产生非整数 hour（旧错误） | 时区,换算,分钟精度,半时区,+5:30,澳门,DST,shiftClock,modulo | auto | - | active | > 半时区下，UTC 时刻 `8:00` 换到本地是 `8 + 5.5 = 13.5 小时` ，被写进 JSON 为非整… |
| frontend/time-zone-minute-arithmetic.md#验证场景 | frontend | 验证场景 | 时区,换算,分钟精度,半时区,+5:30,澳门,DST,shiftClock,modulo | auto | - | active | - 北京用户（UTC+8，整时区）：UTC 14:00 → 显示 22:00（0 舍入误差） - 印度用户（UTC+5:… |
| frontend/trellis-18.md#API Layer (MUST) | frontend | API Layer (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | auto | - | active | > 违反代价: invoke 散落各文件 / 静默丢错 → 后端 command 改名时编译期不报、运行时静默失败难排查… |
| frontend/trellis-18.md#CRUD 刷新链契约 (MUST) | frontend | CRUD 刷新链契约 (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | auto | - | active | > 违反代价: 后端真删/真改的 CRUD 入口（如 `platformApi.delete`）仅刷关联 state（g… |
| frontend/trellis-18.md#Component Patterns (MUST) | frontend | Component Patterns (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | auto | - | active | > 违反代价: 引入 CSS Modules / CSS-in-JS → 样式系统割裂、主题切换失效；index 作 k… |
| frontend/trellis-18.md#Deep-Link 导入契约 (MUST) | frontend | Deep-Link 导入契约 (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | auto | - | active | > 违反代价: 缓存重放 → 用户重访页面时旧导入弹窗反复弹；URL 承载格式与接收端解析不匹配 → 唤起后导入静默失败… |
| frontend/trellis-18.md#Directory Structure (MUST) | frontend | Directory Structure (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | auto | - | active | > 违反代价: 文件放错层 → 后续 agent 按约定 grep 找不到 → 重复造同名文件 / import 路径混… |
| frontend/trellis-18.md#Hooks (MUST) | frontend | Hooks (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | auto | - | active | > 违反代价: 不用 `use` 前缀 → React lint 规则失效、依赖检查漏报；≥2 组件复用却不提取 → 逻… |
| frontend/trellis-18.md#Large File Split — facade 模式 (MUST) | frontend | Large File Split — facade 模式 (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | auto | - | active | > 违反代价: 巨石文件 (>800 行) → 增量改动成本指数增长、merge 冲突频发、agent 上下文爆炸；拆分… |
| frontend/trellis-18.md#State Management (MUST) | frontend | State Management (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | auto | - | active | > 违反代价: 新建 store / 绕过 AppContext 读写 localStorage → 状态双源不一致、持… |
| frontend/trellis-18.md#Type Safety (MUST) | frontend | Type Safety (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | auto | - | active | > 违反代价: 用 `any` / `string` 代替 union → 后端字段改动编译期不报错、运行时崩；漏同步 … |
| frontend/trellis-18.md#i18n (MUST) | frontend | i18n (MUST) | frontend,react,component,hook,state,crud,刷新链,modal,invoke | auto | - | active | - 所有用户可见文案必须用 `t("key")`，禁硬编码中/英文字面量（含 placeholder / title /… |
| git/rule-44.md#关联 | git | 关联 | git,并行,subtask,commit,竞态,staged,worktree | auto | - | active | git-worktree-parallel-isolation |
| git/rule-44.md#处理流程 | git | 处理流程 | git,并行,subtask,commit,竞态,staged,worktree | auto | - | active | ```bash # commit 前检查 staged 文件 git diff --cached --name-only… |
| git/rule-44.md#案例 | git | 案例 | git,并行,subtask,commit,竞态,staged,worktree | auto | - | active | - shadcn-pages task 并行 m-groups/m-logs/m-stats 等子任务，需 commit… |
| git/rule-44.md#触发场景 | git | 触发场景 | git,并行,subtask,commit,竞态,staged,worktree | auto | - | active | 同一 worktree 并行跑多个 subtask 时，不同 agent 可能对同一文件产生变更，导致 git inde… |
| git/rule-44.md#适用 | git | 适用 | git,并行,subtask,commit,竞态,staged,worktree | auto | - | active | - 同 worktree 并行 subtask（skein parallel 模式） - 多 agent 同时改同一文件… |
| git/rule-44.md#陷阱-正解 | git | 陷阱-正解 | git,并行,subtask,commit,竞态,staged,worktree | auto | - | active | ❌ **陷阱**：多个并行 subtask 各自 commit，兄弟 staged 文件可能被误入彼此的 commit（… |
| i18n/i18n-key-deletion-safety.md#i18n key 删除的安全规矩 | i18n | i18n key 删除的安全规矩 | i18n,key删除,grep,check-i18n.mjs,引用清零 | auto | - | active | - |
| i18n/i18n-key-deletion-safety.md#关联 | i18n | 关联 | i18n,key删除,grep,check-i18n.mjs,引用清零 | auto | - | active / →platform-creation-entry-consolidation | [[platform-creation-entry-consolidation]]（同批 task remove-cli… |
| i18n/i18n-key-deletion-safety.md#分类注意 | i18n | 分类注意 | i18n,key删除,grep,check-i18n.mjs,引用清零 | auto | - | active | 关键词 `platform.cliProxy.inherited*` 系列（如 `inheritedEndpoint`,… |
| i18n/i18n-key-deletion-safety.md#反例 | i18n | 反例 | i18n,key删除,grep,check-i18n.mjs,引用清零 | auto | - | active | ❌ 在 i18n JSON 直接删键，不检查代码里还有没有调用 → 运行时缺键报错 ❌ 只 grep 常见调用模式（如直… |
| i18n/i18n-key-deletion-safety.md#正解 | i18n | 正解 | i18n,key删除,grep,check-i18n.mjs,引用清零 | auto | - | active | 1. 确认该 key 的所有调用点    ```bash    grep -r "platform.cliProxy.i… |
| i18n/i18n-key-deletion-safety.md#约束 | i18n | 约束 | i18n,key删除,grep,check-i18n.mjs,引用清零 | auto | - | active | 删 i18n key 时必须**逐 key grep 确认引用点完全归零**。直接删文件内容是常见陷阱。 |
| i18n/i18n-key-deletion-safety.md#触发场景 | i18n | 触发场景 | i18n,key删除,grep,check-i18n.mjs,引用清零 | auto | - | active | 删除项目中的 i18n key 时，需要确保引用点已清零，避免遗留的 key 引用导致运行时错误。 |
| i18n/i18n-key-deletion-safety.md#适用 | i18n | 适用 | i18n,key删除,grep,check-i18n.mjs,引用清零 | auto | - | active | - i18n 文件清理 - 界面流程重构后的 key 梳理 - 删除冗余翻译项 |
| i18n/rule-04.md#MUST 硬约束 | i18n | MUST 硬约束 | i18n,locale,翻译,check-i18n,8语言,同步 | auto | - | active | 新增 i18n key 必须同时补齐 8 个语言文件（zh-Hans/en-US/ar-SA/fr-FR/de-DE/r… |
| i18n/rule-04.md#关联 | i18n | 关联 | i18n,locale,翻译,check-i18n,8语言,同步 | auto | - | active | i18n-flat-key-convention |
| i18n/rule-04.md#处理流程 | i18n | 处理流程 | i18n,locale,翻译,check-i18n,8语言,同步 | auto | - | active | ```bash # 新增 key 后检查 yarn check-i18n  # 自动补齐（示例：从 zh-Hans 复制… |
| i18n/rule-04.md#案例 | i18n | 案例 | i18n,locale,翻译,check-i18n,8语言,同步 | auto | - | active | - shadcn-pages m-checkfix：新增 3 key 同步补 8 locale（1db931fe） |
| i18n/rule-04.md#检查机制 | i18n | 检查机制 | i18n,locale,翻译,check-i18n,8语言,同步 | auto | - | active | - `check-i18n` 守门：跑 `yarn check-i18n` 检测 key 同步 - 缺失语言会导致对应语… |
| i18n/rule-04.md#触发场景 | i18n | 触发场景 | i18n,locale,翻译,check-i18n,8语言,同步 | auto | - | active | alert() 迁移到 toast() 等新 i18n 机制时，新增翻译 key 必须同步到所有 locale。 |
| i18n/rule-04.md#适用 | i18n | 适用 | i18n,locale,翻译,check-i18n,8语言,同步 | auto | - | active | - 所有 i18n key 新增/修改 - alert() → toast() 迁移（如 shadcn-pages ta… |
| i18n/trellis-19.md#RTL | i18n | RTL | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | auto | - | active | `ar-SA` 是唯一 RTL locale (`RTL_LOCALES`, `index.ts:28`); `isRT… |
| i18n/trellis-19.md#三层一致 (MUST) | i18n | 三层一致 (MUST) | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | auto | - | active | 应用 i18n locale 标签跨三层必须**字面同一集合**:  1. **i18next** (`src/loca… |
| i18n/trellis-19.md#关联 | i18n | 关联 | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | auto | - | active | - [backend/index.md](../backend/index.md) — presets JSON (后端… |
| i18n/trellis-19.md#多 locale 命名空间共存, 禁统一 (MUST NOT) | i18n | 多 locale 命名空间共存, 禁统一 (MUST NOT) | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | auto | - | active | 应用内存在 **4 套独立 locale 命名空间**, 各服务不同消费者, 标签约定不同是有意设计, **禁强行统一*… |
| i18n/trellis-19.md#应用 i18n locale 标签 = BCP 47 script 子标签 (MUST) | i18n | 应用 i18n locale 标签 = BCP 47 script 子标签 (MUST) | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | auto | - | active | - **MUST `zh-Hans`** (script 子标签), **禁 `zh-CN`** (region 子标签… |
| i18n/trellis-19.md#持久化迁移 (MUST, 单向) | i18n | 持久化迁移 (MUST, 单向) | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | auto | - | active | - `src/context/AppContext.tsx:98` 启动读用户设置时: `raw.locale === … |
| i18n/trellis-19.md#测试 fixture / 文档 URL (合法残留, 非命名空间) | i18n | 测试 fixture / 文档 URL (合法残留, 非命名空间) | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | auto | - | active | - 测试用 `zh-CN` fixture (`test_sync_settings.rs` / `test_apply… |
| i18n/trellis-19.md#验收基准 (可复用) | i18n | 验收基准 (可复用) | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | auto | - | active | - [ ] `ALL_LOCALES` 集合 == presets JSON 任一 protocol 的 `name` … |
| i18n/trellis-19.md#验证命令 | i18n | 验证命令 | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | auto | - | active | ```bash # zh-CN 残留审计 (合法点见上 "测试 fixture / 文档 URL" + 4 命名空间表;… |
| ops/idle-wakeup-sources-inventory.md#空闲期唤醒源 6 分类清单 | ops | 空闲期唤醒源 6 分类清单 | wakeup,timers,scheduler,sources,profiling,static-analysis,cpu | auto | - | active / →idle-cpu-baseline-xctrace,measure-window-exclusive-env | 空闲期 CPU 唤醒源分 6 类，静态 rg 检索无遗漏（src-tauri + src）。  / 分类 / 频率 / … |
| ops/stack-attribution-profiling-methodology.md#栈归因用法 | ops | 栈归因用法 | profiling,stack-trace,attribution,instruments,xctrace,methodology,cpu | auto | - | active / →idle-cpu-baseline-xctrace,measure-window-exclusive-env,webkit-jit-warmup-trap | **定理**：静态检索定时器只能估出量级（因周期、触发条件、执行成本都是猜），无法判断是否真在稳态 CPU 占比中命中。… |
| ops/test-data-isolation-constraint.md#性能测试数据隔离约束 | ops | 性能测试数据隔离约束 | testing,data,isolation,database,measurement,real-data | auto | - | active | - |
| ops/test-data-isolation-constraint.md#测试数据隔离硬约束 | ops | 测试数据隔离硬约束 | testing,data,isolation,database,measurement,real-data | auto | - | active | 性能量测或功能验证时需要用特定数据库（如缩小库、污染库等）。  ### 硬约束  - **禁移动/重命名用户的真实库文件… |
| ops/trellis-17.md#Cross-reference | ops | Cross-reference | sync,defaults,json,jsdelivr,remote,validate,presets,hash | auto | - | active | - 先例代码: `crates/aidog_core/src/gateway/defaults_sync.rs`（pla… |
| ops/trellis-17.md#实例 | ops | 实例 | sync,defaults,json,jsdelivr,remote,validate,presets,hash | auto | - | active | - task 07-09-*（platform-presets 同步首次落地，`defaults_sync.rs` 先例… |
| ops/trellis-17.md#数据流架构 (MUST，禁前端直读 github) | ops | 数据流架构 (MUST，禁前端直读 github) | sync,defaults,json,jsdelivr,remote,validate,presets,hash | auto | - | active | ``` github (master) ──rust sync (<x>_sync.rs)──▶ ~/.aidog/<f… |
| ops/trellis-17.md#范式 (MUST，照抄先例 `gateway/defaults_sync.rs`) | ops | 范式 (MUST，照抄先例 `gateway/defaults_sync.rs`) | sync,defaults,json,jsdelivr,remote,validate,presets,hash | auto | - | active | `defaults/*.json` 远端同步**MUST** 实现完整 7 件套，缺一致命。先例 `crates/aid… |
| ops/trellis-17.md#验收断言（可复用） | ops | 验收断言（可复用） | sync,defaults,json,jsdelivr,remote,validate,presets,hash | auto | - | active | ```bash # 7 件套齐全（双源 / last_updated / 24h / 三路触发 / schema gat… |
| optimization/api-payload-optimization.md#后端 DISTINCT 替代前端集合去重降低 IPC payload | optimization | 后端 DISTINCT 替代前端集合去重降低 IPC payload | api,payload,ipc,distinct,set-deduplication,query-optimization | auto | - | active | 后端改为返回去重后的单列（如 DISTINCT model），而非拉全字段摘要行数组到前端，再用集合去重。  **收益*… |
| optimization/idle-cpu-baseline-xctrace.md#空闲 CPU 基线数据 | optimization | 空闲 CPU 基线数据 | baseline,measurement,xctrace,process,webkit,profiling,cpu | auto | - | active / →idle-wakeup-sources-inventory,measure-window-exclusive-env,webkit-jit-warmup-trap | 基于 xctrace Time Profiler 实测（2026-07-31，30s 采样窗口）。四进程占比： - **… |
| optimization/idle-cpu-stack-sampling.md#反例（错误模式） | optimization | 反例（错误模式） | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / 仅 grep 定时器列表 / grep 列表 + `sample`… |
| optimization/idle-cpu-stack-sampling.md#案例 | optimization | 案例 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | grep 找到 5 个定时器，工作量推算应占 CPU 1-1.5%。但实测 3.0% 稳态，缺口 1.5% 无法追溯。用… |
| optimization/idle-cpu-stack-sampling.md#空闲 CPU 归因必须靠栈采样 | optimization | 空闲 CPU 归因必须靠栈采样 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | - |
| optimization/idle-cpu-stack-sampling.md#触发场景 | optimization | 触发场景 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | 性能分析中发现应用稳态 CPU 占用 3.0%，但静态代码检索只能找到 60s×1 + 300s×1 + 24h×3 共… |
| optimization/idle-cpu-stack-sampling.md#适用 | optimization | 适用 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | - 稳态 CPU 3% 以上但代码检索无法解释的场景 - 长时间后台进程 CPU 诊断 - 定时任务链效应分析（A 定时… |
| optimization/idle-cpu-stack-sampling.md#陷阱 & 正解 | optimization | 陷阱 & 正解 | cpu,profiling,sample,timer,instruments,time-profiler | auto | - | active | ❌ **陷阱**：仅用静态代码检索（grep）列举定时器  ```bash # 搜索所有定时器 grep -r "set… |
| optimization/manual-budget-empty-shortcircuit.md#manual_budget 零配额短路：进写连接前预检 | optimization | manual_budget 零配额短路：进写连接前预检 | manual-budget,optimization,db-write,shortcircuit,loadgen | auto | - | active | - |
| optimization/manual-budget-empty-shortcircuit.md#关键点 | optimization | 关键点 | manual-budget,optimization,db-write,shortcircuit,loadgen | auto | - | active | - **硬约束**：配额存在时行为不变，短路仅对「零配额」分支生效 - **非 mock 专属**：真实转发路径共用同一… |
| optimization/manual-budget-empty-shortcircuit.md#方案 | optimization | 方案 | manual-budget,optimization,db-write,shortcircuit,loadgen | auto | - | active | **分两阶段：**  1. **只读池预检**（`has_any_budget`，line:189-203）：用只读池（… |
| optimization/manual-budget-empty-shortcircuit.md#用途 | optimization | 用途 | manual-budget,optimization,db-write,shortcircuit,loadgen | auto | - | active | 高频转发路径的每请求冷路径优化，减少单线程 DB 写锁争。适用于： - mock/真实平台混用的压测 - 用户未配额时的… |
| optimization/manual-budget-empty-shortcircuit.md#问题 | optimization | 问题 | manual-budget,optimization,db-write,shortcircuit,loadgen | auto | - | active | `apply_manual_budgets`（`manual_budget.rs:211-246`）处理用户手动配额时，… |
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
| proxy/router.md#判定口径契约 | proxy | 判定口径契约 | router,platform,sole_platform,单启用平台,短路,路由优化 | auto | - | active | ### 定义  **单启用平台分组** (`sole_platform`) 判定唯一真值源位于 `src-tauri/c… |
| proxy/router.md#单启用平台分组判定 (sole_platform) | proxy | 单启用平台分组判定 (sole_platform) | router,platform,sole_platform,单启用平台,短路,路由优化 | auto | - | active | - |
| proxy/rule-50.md#关联 | proxy | 关联 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | auto | - | active / →trellis-00,trellis-11 | [[trellis-11]] （proxy 统计不污染） · [[trellis-00]] （DB 表设计） |
| proxy/rule-50.md#反例 / 常见错误 | proxy | 反例 / 常见错误 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | auto | - | active | / 错误                          / 为什么错                        … |
| proxy/rule-50.md#案例 | proxy | 案例 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | auto | - | active | - log-async-write task (commit 529e571b) — proxy_log 改为单 wri… |
| proxy/rule-50.md#正解：方案 B（单 writer + 有界 queue + 分级背压 + 串行快照） | proxy | 正解：方案 B（单 writer + 有界 queue + 分级背压 + 串行快照） | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | auto | - | active | ### 架构骨架 ``` 热路径 (request handler)       后台 writer task ────… |
| proxy/rule-50.md#落库路径升级 checklist | proxy | 落库路径升级 checklist | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | auto | - | active | ```rust // 新增高频异步操作时参考此模式： // 1. 定义枚举消息类型 pub(crate) enum Yo… |
| proxy/rule-50.md#触发场景 | proxy | 触发场景 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | auto | - | active | 高频热路径中需要异步写入数据库（如 proxy_log upsert），不能阻塞请求处理；需要保证最终结果不丢且落库顺序… |
| proxy/rule-50.md#适用 | proxy | 适用 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | auto | - | active | - proxy_log 异步写入（已实现 s1） - 其他高频日志 / 统计 / 聚合表的异步更新（future 可参考… |
| proxy/rule-50.md#陷阱：同步写会阻塞热路径 + 异步不保证持久性 | proxy | 陷阱：同步写会阻塞热路径 + 异步不保证持久性 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | auto | - | active | > proxy_log 原先热路径内同步调 `upsert_log(db).await` → 所有请求必须等 DB 写入… |
| proxy/rule-50.md#验证 | proxy | 验证 | proxy,async,queue,mpsc,背压,背压策略,writer,snapshot,upsert,流式,中间态,终态 | auto | - | active | ```bash # 背压分级（中间态 try_send vs 终态 send） cd src-tauri && grep… |
| proxy/trellis-11.md#CONNECT target 多源解析 (MUST) | proxy | CONNECT target 多源解析 (MUST) | proxy,connect,tunnel,axum,hyper,TcpStream | auto | - | active | > 违反代价: `req.uri().path()` 对 authority-form URI 返空 → `target… |
| proxy/trellis-11.md#CONNECT 路由契约 (MUST) | proxy | CONNECT 路由契约 (MUST) | proxy,connect,tunnel,axum,hyper,TcpStream | auto | - | active | > 违反代价: `.route()` 注册 CONNECT → authority-form URI `host:por… |
| proxy/trellis-11.md#MITM CA 信任库安装 (MUST — 三 OS 原生提权) | proxy | MITM CA 信任库安装 (MUST — 三 OS 原生提权) | proxy,connect,tunnel,axum,hyper,TcpStream | auto | - | active | > 违反代价: 假 CA 装不进系统信任库 → 客户端不信任 AirDog 签的 host 证书 → MITM 解密全挂… |
| proxy/trellis-11.md#TCP 双向隧道 (MUST) | proxy | TCP 双向隧道 (MUST) | proxy,connect,tunnel,axum,hyper,TcpStream | auto | - | active | - `tokio::io::copy` 双向 + `tokio::join!` 同时转发两向 - 字节 u64 返回值:… |
| proxy/trellis-11.md#hyper-util upgrade downcast 类型 (MUST) | proxy | hyper-util upgrade downcast 类型 (MUST) | proxy,connect,tunnel,axum,hyper,TcpStream | auto | - | active | > 违反代价: downcast 类型错 → 取不到底层流 → 隧道空转 / panic。research 说 `dow… |
| proxy/trellis-11.md#proxy_log 写入契约 (MUST — 不污染 stats_agg) | proxy | proxy_log 写入契约 (MUST — 不污染 stats_agg) | proxy,connect,tunnel,axum,hyper,TcpStream | auto | - | active | > 违反代价: CONNECT 流量走 `upsert_log` → 触发 `upsert_stats_agg` + `… |
| proxy/trellis-11.md#前端筛选 sentinel (MUST) | proxy | 前端筛选 sentinel (MUST) | proxy,connect,tunnel,axum,hyper,TcpStream | auto | - | active | - Logs/Stats 平台筛选「无平台」: value `"0"` → `Number("0")=0` → `pla… |
| proxy/trellis-11.md#平台 host 匹配 (MUST) | proxy | 平台 host 匹配 (MUST) | proxy,connect,tunnel,axum,hyper,TcpStream | auto | - | active | - `match_platform_by_host` (新增, `endpoint.rs`) — CONNECT tar… |
| proxy/trellis-11.md#验证 | proxy | 验证 | proxy,connect,tunnel,axum,hyper,TcpStream | auto | - | active | ```bash # CONNECT 分流 early return, 非 CONNECT 原 fallthrough g… |
| proxy/trellis-12.md#host self 判定分支 (复用, 不变) | proxy | host self 判定分支 (复用, 不变) | proxy,fallback,host,route,mitm,path | auto | - | active | loopback 名 (`localhost`/`127.0.0.1`/`0.0.0.0`) + listen ip 字… |
| proxy/trellis-12.md#关联 | proxy | 关联 | proxy,fallback,host,route,mitm,path | auto | - | active | - CONNECT 隧道 / relay 层: [proxy-connect-relay.md](proxy-conne… |
| proxy/trellis-12.md#核心契约 (MUST) | proxy | 核心契约 (MUST) | proxy,fallback,host,route,mitm,path | auto | - | active | - **`should_fallback_passthrough` host 判定 MUST 前置于 path/is_a… |
| proxy/trellis-12.md#验收基准 (复用断言) | proxy | 验收基准 (复用断言) | proxy,fallback,host,route,mitm,path | auto | - | active | - MITM 灌入: host=`open.bigmodel.cn` + path=`/api/anthropic/v1… |
| proxy/trellis-13.md#absolute-form URI 路由契约 (MUST) | proxy | absolute-form URI 路由契约 (MUST) | proxy,forward,absolute,scheme,relay,host | auto | - | active | > 违反代价: axum 按 `Request::uri().path()` 匹配路由，absolute-form `G… |
| proxy/trellis-13.md#forward URL scheme 自适应 (MUST) | proxy | forward URL scheme 自适应 (MUST) | proxy,forward,absolute,scheme,relay,host | auto | - | active | > 违反代价: `forward_passthrough_to_orig_host` 硬编码 `https://{hos… |
| proxy/trellis-13.md#proxy_log 落虚拟桶 (MUST — 与 MITM fallback 同语义) | proxy | proxy_log 落虚拟桶 (MUST — 与 MITM fallback 同语义) | proxy,forward,absolute,scheme,relay,host | auto | - | active | > 违反代价: forward 流量走独立 upsert 路径 / 单独统计 → 与 MITM 解密 fallback … |
| proxy/trellis-13.md#跨层 / 关联 spec | proxy | 跨层 / 关联 spec | proxy,forward,absolute,scheme,relay,host | auto | - | active | - [Proxy CONNECT Relay](./proxy-connect-relay.md) — CONNECT … |
| proxy/trellis-13.md#路由层契约 (MUST) | proxy | 路由层契约 (MUST) | proxy,forward,absolute,scheme,relay,host | auto | - | active | - **`build_router(state: Arc<ProxyState>) -> Router`** — Rou… |
| proxy/trellis-13.md#验证 | proxy | 验证 | proxy,forward,absolute,scheme,relay,host | auto | - | active | ```bash # absolute-form middleware 存在 + 路由顶层包装 grep -n "abso… |
| proxy/trellis-14.md#为何 502 路径不触发 / 200 路径触发 | proxy | 为何 502 路径不触发 / 200 路径触发 | reqwest,no_proxy,http_client,forward,env,递归 | auto | - | active | - **502 路径** (上游 `nonexistent.invalid`): reqwest 走 env proxy… |
| proxy/trellis-14.md#禁 env proxy 契约 (MUST) | proxy | 禁 env proxy 契约 (MUST) | reqwest,no_proxy,http_client,forward,env,递归 | auto | - | active | > 违反代价: AirDog 自身是代理 (监听 :9892), 转发上游时若 reqwest 读 `HTTPS_PRO… |
| proxy/trellis-14.md#验证 | proxy | 验证 | reqwest,no_proxy,http_client,forward,env,递归 | auto | - | active | ```bash # use_proxy=false 分支有 .no_proxy() grep -n "no_proxy"… |
| proxy/trellis-15.md#Helper 复用契约 (MUST) | proxy | Helper 复用契约 (MUST) | proxy,header,diagnostic,trace,blind_relay,debug | auto | - | active | > 违反代价: 各响应构造点重复实现 `cfg!(debug_assertions)` gate, 新加诊断 heade… |
| proxy/trellis-15.md#blind_relay 物理豁免 (MUST NOT) | proxy | blind_relay 物理豁免 (MUST NOT) | proxy,header,diagnostic,trace,blind_relay,debug | auto | - | active | > 违反代价: blind_relay 是 CONNECT 隧道建好后 TCP 字节透传, AirDog 看见的是加密 … |
| proxy/trellis-15.md#header 名规范 (MUST) | proxy | header 名规范 (MUST) | proxy,header,diagnostic,trace,blind_relay,debug | auto | - | active | - **header 名 MUST 小写** (`x-aidog-trace` 等), 用 `HeaderName::f… |
| proxy/trellis-15.md#id 取值链 (MUST) | proxy | id 取值链 (MUST) | proxy,header,diagnostic,trace,blind_relay,debug | auto | - | active | > 违反代价: 各处自造 id 失去与 proxy_log / span 的关联, 诊断时无法客户端报错 → AirDo… |
| proxy/trellis-15.md#release build 行为 (MUST) | proxy | release build 行为 (MUST) | proxy,header,diagnostic,trace,blind_relay,debug | auto | - | active | - **release build MUST 不注入** —— helper 内 `if cfg!(debug_asse… |
| proxy/trellis-15.md#跨协议注入选址参考 | proxy | 跨协议注入选址参考 | proxy,header,diagnostic,trace,blind_relay,debug | auto | - | active | `07-05-proxy-trace-id-header` 实施时枚举的 47 调用点分布: - `handler.rs… |
| proxy/trellis-15.md#验收基准 (可复用) | proxy | 验收基准 (可复用) | proxy,header,diagnostic,trace,blind_relay,debug | auto | - | active | - [ ] debug build: 所有 AirDog **直构**响应含诊断 header (grep `injec… |
| proxy/trellis-15.md#验证命令 | proxy | 验证命令 | proxy,header,diagnostic,trace,blind_relay,debug | auto | - | active | ```bash # helper 调用计数 (debug 注入点) grep -rn "inject_trace_hea… |
| reuse/auto-fix-downgrade-36.md#Abstract Threshold | reuse | Abstract Threshold | grep,reuse,复用,组件,utility,抽象,dry | auto | - | active | - ≥ 3 处相同逻辑 → 必须 abstract - 2 处相同逻辑 → 必须 grep 确认，commit mess… |
| reuse/auto-fix-downgrade-36.md#MUST | reuse | MUST | grep,reuse,复用,组件,utility,抽象,dry | auto | - | active | - 写新函数前必须 `grep -rE '<关键词>' src/` 查已有实现；命中则复用，禁重写 - 新增平台协议必须… |
| reuse/auto-fix-downgrade-36.md#MUST NOT | reuse | MUST NOT | grep,reuse,复用,组件,utility,抽象,dry | auto | - | active | - 禁止为新页面复制已有页面的 CRUD 模板代码而不提取公共组件 - 禁止定义与 `api.ts` 中已有 names… |
| reuse/auto-fix-downgrade-36.md#关联 | reuse | 关联 | grep,reuse,复用,组件,utility,抽象,dry | auto | - | active / →shadcn-infra-28 | [[shadcn-infra-28]] (shadcn 依赖复用) |
| reuse/auto-fix-downgrade-36.md#触发场景 | reuse | 触发场景 | grep,reuse,复用,组件,utility,抽象,dry | auto | - | active | 写新函数 / 新组件 / 新 utility 前。 |
| reuse/auto-fix-downgrade-36.md#适用 | reuse | 适用 | grep,reuse,复用,组件,utility,抽象,dry | auto | - | active | 写新代码前查复用、防止重复实现 |
| build/shadcn/shadcn-primitives-39.md#关联 | shadcn | 关联 | shadcn,add,dependencies,yarn,tailwind,verification | auto | - | active / →shadcn-infra-02 | [[shadcn-infra-02]] |
| build/shadcn/shadcn-primitives-39.md#正解 | shadcn | 正解 | shadcn,add,dependencies,yarn,tailwind,verification | auto | - | active | add 后必 grep package.json 验证依赖在，缺则 `yarn add <pkg>` 补。 |
| build/shadcn/shadcn-primitives-39.md#规则 | shadcn | 规则 | shadcn,add,dependencies,yarn,tailwind,verification | auto | - | active | 不预设必漏也不预设必装，每次 add 后验证。 |
| build/shadcn/shadcn-primitives-39.md#证据 | shadcn | 证据 | shadcn,add,dependencies,yarn,tailwind,verification | auto | - | active | commit 2b79767a "补 class-variance-authority 依赖 (shadcn add 漏… |
| build/shadcn/shadcn-primitives-39.md#适用 | shadcn | 适用 | shadcn,add,dependencies,yarn,tailwind,verification | auto | - | active | yarn 4+ + tailwind 4 + shadcn add 操作 |
| build/shadcn/shadcn-primitives-39.md#问题 | shadcn | 问题 | shadcn,add,dependencies,yarn,tailwind,verification | auto | - | active | shadcn add 在 yarn4+tailwind4 下 "Installing dependencies" 阶段不… |
| shadcn/rule-03.md#MUST 硬约束 | shadcn | MUST 硬约束 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | auto | - | active | Radix Dialog **必须包含 DialogTitle**，否则会触发 a11y 警告。 |
| shadcn/rule-03.md#关联 | shadcn | 关联 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | auto | - | active / →rule-43 | [[rule-43]] |
| shadcn/rule-03.md#实现模式 | shadcn | 实现模式 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | auto | - | active | ❌ **陷阱**：自定义 header 时完全省略 DialogTitle，破坏 a11y。 ✅ **正解**：用 `s… |
| shadcn/rule-03.md#案例 | shadcn | 案例 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | auto | - | active | - `src/components/settings/editors/StatusLineSection/Segment… |
| shadcn/rule-03.md#模式模板 | shadcn | 模式模板 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | auto | - | active | ```tsx import { Dialog, DialogContent, DialogTitle } from "@… |
| shadcn/rule-03.md#触发场景 | shadcn | 触发场景 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | auto | - | active | 使用 Radix Dialog 组件时，必须满足无障碍（a11y）要求。 |
| shadcn/rule-03.md#适用 | shadcn | 适用 | Radix,Dialog,DialogTitle,a11y,sr-only,无障碍 | auto | - | active | - 所有 Radix Dialog 用法（@/components/ui/dialog） - 需要完全自定义 heade… |
| shadcn/rule-41.md#关联 | shadcn | 关联 | radix,Select,空值,哨兵,__none__ | auto | - | active / →rule-42 | [[rule-42]] |
| shadcn/rule-41.md#案例 | shadcn | 案例 | radix,Select,空值,哨兵,__none__ | auto | - | active | - `src/pages/Logs/primitives.tsx:12-13` 定义 NONE 常量 + 注释说明 - … |
| shadcn/rule-41.md#模式模板 | shadcn | 模式模板 | radix,Select,空值,哨兵,__none__ | auto | - | active | ```tsx // 定义哨兵常量 const NONE = "__none__";  // 组件使用 <Select  … |
| shadcn/rule-41.md#触发场景 | shadcn | 触发场景 | radix,Select,空值,哨兵,__none__ | auto | - | active | 使用 radix Select 组件时，value 属性需要处理空值/undefined 状态。 |
| shadcn/rule-41.md#适用 | shadcn | 适用 | radix,Select,空值,哨兵,__none__ | auto | - | active | - radix Select 组件（@/components/ui/select） - 需要空值占位符的下拉选择场景 |
| shadcn/rule-41.md#陷阱-正解 | shadcn | 陷阱-正解 | radix,Select,空值,哨兵,__none__ | auto | - | active | ❌ **陷阱**：直接使用 `value=""` 会触发 radix Select 内部验证错误（SelectItem … |
| shadcn/rule-42.md#关联 | shadcn | 关联 | radix,Select,number,String,Number,双向映射 | auto | - | active / →rule-41 | [[rule-41]] |
| shadcn/rule-42.md#案例 | shadcn | 案例 | radix,Select,number,String,Number,双向映射 | auto | - | active | - `src/pages/Logs/primitives.tsx:374` Pagination pageSize: `… |
| shadcn/rule-42.md#模式模板 | shadcn | 模式模板 | radix,Select,number,String,Number,双向映射 | auto | - | active | ```tsx <Select   value={String(numberValue)}  // 存储/显示：numbe… |
| shadcn/rule-42.md#触发场景 | shadcn | 触发场景 | radix,Select,number,String,Number,双向映射 | auto | - | active | radix Select 的 value 属性只接受 string 类型，需要处理 number 类型数据。 |
| shadcn/rule-42.md#适用 | shadcn | 适用 | radix,Select,number,String,Number,双向映射 | auto | - | active | - radix Select value 仅收 string（类型约束） - 需要处理 number 选项的分页器/数值… |
| shadcn/rule-42.md#陷阱-正解 | shadcn | 陷阱-正解 | radix,Select,number,String,Number,双向映射 | auto | - | active | ❌ **陷阱**：直接传 number 会触发类型错误或运行时异常。 ✅ **正解**双向映射：存储/显示时 Strin… |
| shadcn/rule-43.md#关联 | shadcn | 关联 | Dialog,open,null,Promise,resolve,bool | auto | - | active / →rule-41 | [[rule-41]] |
| shadcn/rule-43.md#案例 | shadcn | 案例 | Dialog,open,null,Promise,resolve,bool | auto | - | active | - 通用模式：shadcn-pages 迁移中所有 Dialog 均用 `open={state !== null}` |
| shadcn/rule-43.md#模式模板 | shadcn | 模式模板 | Dialog,open,null,Promise,resolve,bool | auto | - | active | ```tsx const [modalState, setModalState] = useState<{resolve… |
| shadcn/rule-43.md#触发场景 | shadcn | 触发场景 | Dialog,open,null,Promise,resolve,bool | auto | - | active | Dialog.open 属性需要 bool 类型，但实际控制常来自 Promise resolve 型 state（如 … |
| shadcn/rule-43.md#适用 | shadcn | 适用 | Dialog,open,null,Promise,resolve,bool | auto | - | active | - 任何 Promise resolve 型 state 控制弹窗开关的场景（如 async confirm/自定义 M… |
| shadcn/rule-43.md#陷阱-正解 | shadcn | 陷阱-正解 | Dialog,open,null,Promise,resolve,bool | auto | - | active | ❌ **陷阱**：直接用 `open={modalState}` 会将 null/对象转为 bool，无法正确反映「有 … |
| shadcn/rule-45.md#关联 | shadcn | 关联 | popover,只读,shadcn,迁移,预筛,grep | auto | - | active / →rule-41 | [[rule-41]] |
| shadcn/rule-45.md#案例 | shadcn | 案例 | popover,只读,shadcn,迁移,预筛,grep | auto | - | active | - shadcn-pages task：PopoverConfigTab 经 grep 命中 0，确认无需迁移 |
| shadcn/rule-45.md#触发场景 | shadcn | 触发场景 | popover,只读,shadcn,迁移,预筛,grep | auto | - | active | popover 独立窗口（TrayConfigTab）是只读展示域，无表单控件，不适用通用 shadcn 迁移模板。 |
| shadcn/rule-45.md#适用 | shadcn | 适用 | popover,只读,shadcn,迁移,预筛,grep | auto | - | active | - popover 独立窗口（TrayConfigTab）等只读域 - planning 阶段 shadcn 迁移范围判… |
| shadcn/rule-45.md#陷阱-正解 | shadcn | 陷阱-正解 | popover,只读,shadcn,迁移,预筛,grep | auto | - | active | ❌ **陷阱**：planning 阶段未预筛，按通用模板对所有页面跑 shadcn 迁移，对只读域产生误判（实际无 b… |
| shadcn/rule-45.md#预筛命令 | shadcn | 预筛命令 | popover,只读,shadcn,迁移,预筛,grep | auto | - | active | ```bash # 检查目标域是否有可迁组件 grep -c "<button\/<input\/<select\/<t… |
| shadcn/rule-46.md#MUST 硬约束 | shadcn | MUST 硬约束 | shadcn,Button,cva,svg,16px,size-4 | auto | - | active | shadcn Button 内的 svg 图标会被强制压至 16px（`size-4` = 1rem = 16px），自… |
| shadcn/rule-46.md#关联 | shadcn | 关联 | shadcn,Button,cva,svg,16px,size-4 | auto | - | active / →rule-43 | [[rule-43]] |
| shadcn/rule-46.md#实现模式 | shadcn | 实现模式 | shadcn,Button,cva,svg,16px,size-4 | auto | - | active | ```tsx // Button cva 基类（shadcn/ui/button.tsx） variants: {   … |
| shadcn/rule-46.md#案例 | shadcn | 案例 | shadcn,Button,cva,svg,16px,size-4 | auto | - | active | - shadcn-pages task：Sidebar nav icon 迁移至 Button，接受 16px 默认 |
| shadcn/rule-46.md#触发场景 | shadcn | 触发场景 | shadcn,Button,cva,svg,16px,size-4 | auto | - | active | shadcn Button 组件 cva 基类含 `[&_svg]:size-4` 规则，统一压内部 svg 至 16p… |
| shadcn/rule-46.md#适用 | shadcn | 适用 | shadcn,Button,cva,svg,16px,size-4 | auto | - | active | - 所有 shadcn Button 用法（@/components/ui/button） - nav icon 等小图… |
| shadcn/rule-47.md#关联 | shadcn | 关联 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | auto | - | active / →rule-41 | [[rule-41]] |
| shadcn/rule-47.md#案例 | shadcn | 案例 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | auto | - | active | - shadcn-pages task：Groups/GroupListItem SortableList 迁移，保留拖… |
| shadcn/rule-47.md#模式模板 | shadcn | 模式模板 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | auto | - | active | ```tsx // 保留：拖拽逻辑 const { attributes, listeners, setNodeRef,… |
| shadcn/rule-47.md#触发场景 | shadcn | 触发场景 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | auto | - | active | dnd-kit SortableList 组件迁移时，只需替换内部 button/视觉组件，拖拽逻辑保持不变。 |
| shadcn/rule-47.md#适用 | shadcn | 适用 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | auto | - | active | - dnd-kit SortableList 迁移至 shadcn - 保留拖拽逻辑仅换视觉的场景 |
| shadcn/rule-47.md#陷阱-正解 | shadcn | 陷阱-正解 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | auto | - | active | ❌ **陷阱**：重写整个拖拽逻辑，破坏已有行为。 ✅ **正解**：保留 dnd-kit 的 useSortable/… |
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
| style/css-reset-layer.md#CSS reset 必须写进 @layer base | style | CSS reset 必须写进 @layer base | tailwind,v4,reset,layer,css-precedence,shadcn | auto | - | active | - |
| style/css-reset-layer.md#关联 | style | 关联 | tailwind,v4,reset,layer,css-precedence,shadcn | auto | - | active / →shadcn-infra-02,shadcn-infra-30,tailwind-cascade-layer-unlayered | [[shadcn-infra-30]] [[shadcn-infra-02]] [[tailwind-cascade-l… |
| style/css-reset-layer.md#反例（错误模式） | style | 反例（错误模式） | tailwind,v4,reset,layer,css-precedence,shadcn | auto | - | active | / ❌ 错 / ✅ 改为 / /---/---/ / `* { padding: 0; }` unlayered / `… |
| style/css-reset-layer.md#案例 | style | 案例 | tailwind,v4,reset,layer,css-precedence,shadcn | auto | - | active | commit `2b14131e`：git diff 展示 `src/styles/globals.css` 把旧 `*… |
| style/css-reset-layer.md#触发场景 | style | 触发场景 | tailwind,v4,reset,layer,css-precedence,shadcn | auto | - | active | Tailwind v4 迁移到新组件库（如 shadcn）后，CSS reset 声明失效，导致按钮/输入框文字贴边。症… |
| style/css-reset-layer.md#适用 | style | 适用 | tailwind,v4,reset,layer,css-precedence,shadcn | auto | - | active | - Tailwind v3 → v4 迁移 - 新项目用 v4 + shadcn - 任何 CSS reset 失效症状… |
| style/css-reset-layer.md#陷阱 & 正解 | style | 陷阱 & 正解 | tailwind,v4,reset,layer,css-precedence,shadcn | auto | - | active | ❌ **陷阱**：在 `src/styles/globals.css` 裸写 CSS reset  ```css * {… |
| style/trellis-16.md#ANSI 着色 (MUST) | style | ANSI 着色 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | auto | - | active | - **console MUST ANSI on** (`AidogFormat { ansi: true }`), f… |
| style/trellis-16.md#id 双轨映射 (MUST) | style | id 双轨映射 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | auto | - | active | > 违反代价: proxy 请求 header id 不能反查 proxy_log 行; 或全局统一随机失去诊断关联。 … |
| style/trellis-16.md#id 格式规范 (MUST) | style | id 格式规范 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | auto | - | active | - **每级 id MUST 6 位 `[0-9a-z]`** (36^6 ≈ 2.2B 空间) - **多级 MUST… |
| style/trellis-16.md#thread-local 栈角色 (MUST) | style | thread-local 栈角色 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | auto | - | active | - **thread-local `TRACE_ID_STACK` 仅同步业务代码 fallback** (inject… |
| style/trellis-16.md#traceid 取值链 (MUST) | style | traceid 取值链 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | auto | - | active | > 违反代价: 日志行无 id 可 grep = 诊断 header 设计目的 (header↔日志映射) 失效。  -… |
| style/trellis-16.md#健康端点 span (MUST) | style | 健康端点 span (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | auto | - | active | > 违反代价: 健康端点无 span → inject_trace_header 兜底现场造孤儿 id, header↔… |
| style/trellis-16.md#异步分支 id 传播 (MUST) | style | 异步分支 id 传播 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | auto | - | active | > 违反代价: thread-local 栈在 tokio spawn 后失效 (跨线程执行不继承), 子任务内 tra… |
| style/trellis-16.md#日志字段顺序 (MUST) | style | 日志字段顺序 (MUST) | log,trace,traceid,ansi,format,spawn_traced,span | auto | - | active | > 违反代价: 用户诊断时按位置 grep 失败, dev/release 字段顺序不一致需两套解析。  - **MUS… |
| style/trellis-16.md#跨层 / 关联 spec | style | 跨层 / 关联 spec | log,trace,traceid,ansi,format,spawn_traced,span | auto | - | active | - [Proxy Diagnostic Headers](./proxy-diagnostic-headers.md) … |
| style/trellis-16.md#验收基准 (可复用) | style | 验收基准 (可复用) | log,trace,traceid,ansi,format,spawn_traced,span | auto | - | active | - [ ] debug build: header `x-aidog-trace` id grep 日志命中 ≥1 行 … |
| style/trellis-16.md#验证命令 | style | 验证命令 | log,trace,traceid,ansi,format,spawn_traced,span | auto | - | active | ```bash # 格式器装在 console + file 两层 grep -n "AidogFormat\/even… |
| test/rule-48.md#MUST 硬约束 | test | MUST 硬约束 | shadcn,测试,snapshot,行为断言,className | auto | - | active | 测试改测行为而非 className；shadcn 迁移后 snapshot 应改为行为断言。 |
| test/rule-48.md#关联 | test | 关联 | shadcn,测试,snapshot,行为断言,className | auto | - | active / →rule-41 | [[rule-41]] |
| test/rule-48.md#案例 | test | 案例 | shadcn,测试,snapshot,行为断言,className | auto | - | active | - shadcn-pages task：PlatformCard.test.tsx snapshot → 行为断言（删除… |
| test/rule-48.md#触发场景 | test | 触发场景 | shadcn,测试,snapshot,行为断言,className | auto | - | active | shadcn 迁移导致组件 className/结构变化，现有 snapshot 测试会因视觉差异失败。 |
| test/rule-48.md#迁移模式 | test | 迁移模式 | shadcn,测试,snapshot,行为断言,className | auto | - | active | ```tsx // ❌ 旧：测试 className（脆弱） expect(screen.getByTestId("ca… |
| test/rule-48.md#适用 | test | 适用 | shadcn,测试,snapshot,行为断言,className | auto | - | active | - PlatformCard/BalanceBar 等组件测试 - shadcn 迁移导致 className/结构变化… |
| test/rule-65.md#关联 | test | 关联 | test,migration,module,internal,path | auto | - | active / →rule-60 | [[rule-60]] |
| test/rule-65.md#案例 | test | 案例 | test,migration,module,internal,path | auto | - | active | - arch-deepen-2 c3-commands batch 3：迁 commands_*::src/test_*… |
| test/rule-65.md#正解 | test | 正解 | test,migration,module,internal,path | auto | - | active | 将所有 `aidog_core::` 前缀改为 `crate::`（当前 crate 的自引用）： ```rust //… |
| test/rule-65.md#触发场景 | test | 触发场景 | test,migration,module,internal,path | auto | - | active | 测试代码从外部 crate 迁移进 aidog_core 内部时。 |
| test/rule-65.md#适用 | test | 适用 | test,migration,module,internal,path | auto | - | active | - 跨 crate 迁移测试文件 - 模块合并时 - 测试代码路径清理 |
| test/rule-65.md#陷阱 | test | 陷阱 | test,migration,module,internal,path | auto | - | active | 保持原外部 crate 的全限定路径 `aidog_core::xxx::yyy`，但新位置是 aidog_core 内… |
| testing/deterministic-pseudorandom-loadgen.md#关键点 | testing | 关键点 | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | auto | - | active | - **确定性**：给定 error_rate 的序列完全由进程启动顺序决定，重复压测结果稳定 - **分布均匀**：s… |
| testing/deterministic-pseudorandom-loadgen.md#压测可复现的确定性伪随机（原子计数器+哈希） | testing | 压测可复现的确定性伪随机（原子计数器+哈希） | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | auto | - | active | - |
| testing/deterministic-pseudorandom-loadgen.md#方案 | testing | 方案 | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | auto | - | active | **进程级原子计数器 + 乘法哈希** (`proxy/mock.rs:2-16`)：  ```rust static … |
| testing/deterministic-pseudorandom-loadgen.md#用途 | testing | 用途 | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | auto | - | active | - mock 平台的 error_rate 注入 - 压测场景的确定性故障模拟 - 内存/CPU 基准测试（需要重复压测… |
| testing/deterministic-pseudorandom-loadgen.md#问题 | testing | 问题 | testing,loadgen,deterministic,pseudorandom,splitmix64,atomic,error_rate | auto | - | active | 压测场景（尤其是性能/内存压测）需要可复现的伪随机行为，用于注入 `error_rate=0.05`（5% 请求返回 4… |
| testing/module-load-time-constant-test-rule.md#关联 | testing | 关联 | 单测,模块加载,常数,时区偏移,getTimezoneOffset,mock,spyOn,纯函数,参数化 | auto | - | active / →time-zone-minute-arithmetic | [[time-zone-minute-arithmetic]] (时区换算硬约束) |
| testing/module-load-time-constant-test-rule.md#反例 / 常见错误 | testing | 反例 / 常见错误 | 单测,模块加载,常数,时区偏移,getTimezoneOffset,mock,spyOn,纯函数,参数化 | auto | - | active | / 错误                          / 为什么错                        … |
| testing/module-load-time-constant-test-rule.md#案例 | testing | 案例 | 单测,模块加载,常数,时区偏移,getTimezoneOffset,mock,spyOn,纯函数,参数化 | auto | - | active | - time-models-timezone task (commit d5b00753) — peakHours.ts… |
| testing/module-load-time-constant-test-rule.md#正解：纯函数内核参数化（硬约束，关键） | testing | 正解：纯函数内核参数化（硬约束，关键） | 单测,模块加载,常数,时区偏移,getTimezoneOffset,mock,spyOn,纯函数,参数化 | auto | - | active | ### MUST 两层函数分离（参数化内核 + 便捷包装）  ```ts /** 公开常数：模块加载时求值，用于默认行为… |
| testing/module-load-time-constant-test-rule.md#落地 checklist | testing | 落地 checklist | 单测,模块加载,常数,时区偏移,getTimezoneOffset,mock,spyOn,纯函数,参数化 | auto | - | active | ```bash # 1. 验证纯函数内核（offset 参数显式） grep -A5 "export function … |
| testing/module-load-time-constant-test-rule.md#触发场景 | testing | 触发场景 | 单测,模块加载,常数,时区偏移,getTimezoneOffset,mock,spyOn,纯函数,参数化 | auto | - | active | 模块在加载时求值的常数（如本地时区偏移 `LOCAL_OFFSET_MINUTES`），需要在单测中覆盖不同时区场景。 |
| testing/module-load-time-constant-test-rule.md#适用 | testing | 适用 | 单测,模块加载,常数,时区偏移,getTimezoneOffset,mock,spyOn,纯函数,参数化 | auto | - | active | - 任何模块加载时求值的常数（时区、配置、初始化状态）需参数化单测的场景 - 纯函数测试（数学函数、格式转换、换算） |
| testing/module-load-time-constant-test-rule.md#陷阱：vi.spyOn(Date.prototype, "getTimezoneOffset") 对模块常数无效 | testing | 陷阱：vi.spyOn(Date.prototype, "getTimezoneOffset") 对模块常数无效 | 单测,模块加载,常数,时区偏移,getTimezoneOffset,mock,spyOn,纯函数,参数化 | auto | - | active | > 时区常数 `LOCAL_OFFSET_MINUTES = -new Date().getTimezoneOffset… |
| frontend/theme/shadcn-primitives-40.md#关联 | theme | 关联 | next-themes,theme,conflict,shadcn,sonner | auto | - | active / →modal-state-architecture | [[modal-state-architecture]] (同 task Modal 保留策略) |
| frontend/theme/shadcn-primitives-40.md#待决策 | theme | 待决策 | next-themes,theme,conflict,shadcn,sonner | auto | - | active | - 留待 pages 层评估：是否切换到 next-themes 统一，或隔离 Sonner 主题逻辑 - 当前：保留冲… |
| frontend/theme/shadcn-primitives-40.md#证据 | theme | 证据 | next-themes,theme,conflict,shadcn,sonner | auto | - | active | - src/components/ui/sonner.tsx line 3: `import { useTheme } … |
| frontend/theme/shadcn-primitives-40.md#适用 | theme | 适用 | next-themes,theme,conflict,shadcn,sonner | auto | - | active | shadcn 组件集成 + 主题体系迁移 |
| frontend/theme/shadcn-primitives-40.md#问题 | theme | 问题 | next-themes,theme,conflict,shadcn,sonner | auto | - | active | shadcn Sonner 组件导入 next-themes 的 `useTheme`，与本项目自有主题体系（`src/… |
| ts-rust-boundary/mock-config-4layer-consistency.md#mock 配置四层覆盖的字段一致性检查 | ts-rust-boundary | mock 配置四层覆盖的字段一致性检查 | ts-rust-boundary,mock-config,consistency,serde,json-boundary | auto | - | active | - |
| ts-rust-boundary/mock-config-4layer-consistency.md#失配场景 | ts-rust-boundary | 失配场景 | ts-rust-boundary,mock-config,consistency,serde,json-boundary | auto | - | active | / 症状 / 原因 / /---/---/ / TS 编辑器赋值后无效 / `serializeMockConfig` … |
| ts-rust-boundary/mock-config-4layer-consistency.md#检查表（四处同步） | ts-rust-boundary | 检查表（四处同步） | ts-rust-boundary,mock-config,consistency,serde,json-boundary | auto | - | active | ### 1. Rust struct 定义 (`config.rs:11-25`) - [ ] 新字段声明的类型：`Op… |
| ts-rust-boundary/mock-config-4layer-consistency.md#用途 | ts-rust-boundary | 用途 | ts-rust-boundary,mock-config,consistency,serde,json-boundary | auto | - | active | Rust↔TS 跨边界的配置字段迭代通用检查表。适用于： - 平台/插件配置扩展 - 新增可选设置 - 配置升级 mig… |
| ts-rust-boundary/mock-config-4layer-consistency.md#问题 | ts-rust-boundary | 问题 | ts-rust-boundary,mock-config,consistency,serde,json-boundary | auto | - | active | mock 配置在四层跨 Rust↔TS 边界流转，任一处字段定义/序列化不一致都导致静默失配：  1. **Rust s… |
| ts-rust-boundary/optional-config-backward-compat.md#Option<T> 可选字段的向后兼容方案 | ts-rust-boundary | Option<T> 可选字段的向后兼容方案 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | auto | - | active | - |
| ts-rust-boundary/optional-config-backward-compat.md#关键点 | ts-rust-boundary | 关键点 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | auto | - | active | - **旧字段保留**：必须保留兼容入口，不删不改 - **Option/undefined 对应**：Rust `Op… |
| ts-rust-boundary/optional-config-backward-compat.md#方案 | ts-rust-boundary | 方案 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | auto | - | active | **Rust 端** (`config.rs:11-25`)： ```rust pub struct MockConfi… |
| ts-rust-boundary/optional-config-backward-compat.md#用途 | ts-rust-boundary | 用途 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | auto | - | active | 配置迭代的通用方案，适用于： - 新增可选旋钮 - 旧版本平台配置升级 - 分阶段特性开关（旧特性先 disable，新… |
| ts-rust-boundary/optional-config-backward-compat.md#问题 | ts-rust-boundary | 问题 | ts-rust-boundary,option,backward-compat,unwrap_or,config-migration | auto | - | active | 新旋钮常需跨 Rust↔TS 边界，并与旧配置字段共存以确保向后兼容。  例：`mock` 配置新增 `ttft_ms`… |
