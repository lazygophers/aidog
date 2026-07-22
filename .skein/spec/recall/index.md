# SKEIN recall 规则索引

类目: arch(12), build(3), cross-layer(1), db(3), domain(7), encoding(1), frontend(8), git(1), i18n(1), ops(1), proxy(5), reuse(1), shadcn(7), skein(1), style(1), test(1), theme(1)

| file | category | title | keywords | status | summary |
|---|---|---|---|---|---|
| arch/auto-fix-downgrade-33.md | arch | agent-as-LLM 平台 handler 分支接入范式 | agent,handler,branch,platform,wire,sse | active | # agent-as-LLM 平台 handler 分支接入范式  ## 触发场景 新增「agent-as-LLM」类平… |
| arch/auto-fix-downgrade-34.md | arch | DB 拆库访问点归属审计三形式 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn | active | # DB 拆库访问点归属审计三形式  ## 触发场景 表从一个 SQLite 库拆到另一个库（主库→log.db / p… |
| arch/auto-fix-downgrade-35.md | arch | dedup 禁用设计为空的字段作 key | dedup,空字段,key,数据丢失,合并 | active | # dedup 禁用设计为空的字段作 key  ## 触发场景 写任何 dedup / 去重 / 合并逻辑(HashSe… |
| arch/auto-fix-downgrade-38.md | arch | 删 enum 变体前先 migration DB | enum,serde,db,migration,rust,panic | active | # 删 enum 变体前先 migration DB  ## 触发场景 删 serde 落库的 enum 变体时。  #… |
| arch/coding-plan-utilization-calib-fix-25.md | arch | coding plan 校准链路 base_url 真值源 = endpoint 级 | coding-plan,base_url,quota,calibration,finish,est_coding_plan | active | coding plan 平台 preset 平台级 base_url 恒为 None (真 base_url 在 end… |
| arch/cross-db-subquery-handle-selection.md | arch | 跨库补查闭包 handle 按补查表归属 | db,sqlite,跨库,补查,handle,闭包,cpp,平台名,N+1 | active | # 跨库补查闭包 handle 按补查表归属  何时被读: 跨库查询（主表查 A 库 + 补查 B 库表，如 proxy… |
| arch/non-typical-sql-audit-pattern.md | arch | 非典型 SQL 形态易漏审计 | db,sqlite,sql,审计,helper,裸sql,grep,易漏,访问点 | active | # 非典型 SQL 形态易漏审计  何时被读: 拆库审计某表访问点时 谁读: trellis-implement sub… |
| arch/parser-multi-path-format-symmetry.md | arch | parser 多路径格式识别必须对称 | parser,多路径,symmetry,对称,格式识别,抽函数,复用,入口分裂,oauth | active | # parser 多路径格式识别必须对称  何时被读: 写 / 改 parser 有多个入口识别同一格式时(如「单文件导… |
| arch/shadcn-infra-32.md | arch | locale 死键清理归属 | locale,dead-key,cleanup,responsibility,theme | active | # locale 死键清理归属  ## 流程约定 **删除主题/功能导致的 locale 死键，由删该主题/功能的 ta… |
| arch/trellis-03.md | arch | Workspace crate 边界契约 | crate,boundary,边界,commands,aidog_core,event,依赖 | active | # Workspace Crate 边界契约  何时被读: commands_* crate 内改源码 / 迁移 com… |
| arch/trellis-04.md | arch | Protocol 枚举变体扩展范式 | protocol,enum,变体,grep,serde,match,union | active | # Protocol 枚举变体扩展范式  何时被读: 新增 `Protocol` 枚举变体时（新协议 / 新 cp 变体… |
| arch/trellis-05.md | arch | 前端常量派生自后端 JSON 真值源 | derived,constants,docpromise,defaults,派生,presets,async | active | # 前端派生层（常量 → 后端 JSON 派生）  何时被读: 前端硬编码常量（协议列表 / label 映射 / 颜色… |
| build/shadcn-infra-28.md | build | shadcn add 漏装 cva 依赖 | shadcn,cva,yarn,dependency,class-variance-authority | active | # shadcn add 漏装 cva 依赖  ## 触发场景 运行 `npx shadcn add` 批量添加组件后，… |
| build/shadcn-infra-29.md | build | vite @ alias 手动配置 | vite,alias,resolve,shadcn,tsconfig | active | # vite @ alias 手动配置  ## 触发场景 使用 shadcn/ui 或其他假设存在 `@` 别名的库时，… |
| build/trellis-02.md | build | Cargo workspace 重构门禁 | cargo,workspace,crate,build.rs,重构,门禁,下沉 | active | # Cargo Workspace 重构门禁  何时被读: 单 crate → cargo workspace 多 cr… |
| cross-layer/trellis-20.md | cross-layer | 跨 Rust TS 边界契约 | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | active | # Cross-Layer Rules  何时被读: 改动跨越 Rust↔TypeScript 边界的功能时 谁读: t… |
| db/crash-safe-db-split-migration.md | db | 拆库 crash-safe 四阶段迁移模式 | db,sqlite,拆库,迁移,crash-safe,INSERT OR IGNORE,DROP,保id,幂等 | active | # 拆库 crash-safe 四阶段迁移模式  何时被读: 表从一个 SQLite 库迁移到另一个库（主库→log.d… |
| db/trellis-00.md | db | DB 表设计强制规范 | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | active | # DB Conventions  何时被读: 新增 / 修改任何数据库表、字段、模型、CRUD、迁移时 谁读: tre… |
| db/trellis-01.md | db | tokio_rusqlite 连接韧性契约 | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | active | # DB Connection Resilience  何时被读: 改 `Db` 结构 / DB 调用路径（`call_… |
| domain/coding-plan-utilization-calib-fix-26.md | domain | coding plan 订阅制平台普遍无公开用量查询 API | coding-plan,quota,upstream-api,degrade,custom-quota-script | active | bailian/qianfan/xiaomi/compshare 等 coding plan 订阅制平台上游均无公开程序… |
| domain/cpa-oauth-credential-format.md | domain | CPA OAuth 凭据格式（CLIProxyAPI） | cpa,oauth,credential,cliproxyapi,access_token,model_aliases,xai,multi-account,凭据,导入 | active | # CPA OAuth 凭据格式（CLIProxyAPI）  何时被读: 改 CPA 导入解析器 / 加新 OAuth … |
| domain/trellis-06.md | domain | mock 平台类型规范 | mock,platform,extra,test,builder,error_mode | active | # Mock Platform Type  何时被读: 改 mock 平台逻辑（adapter/mock.rs / pr… |
| domain/trellis-07.md | domain | Claude Code 订阅透传平台 | claude,passthrough,透传,subscription,header | active | # Claude Code Passthrough Platform Type  何时被读: 改 Claude Code… |
| domain/trellis-08.md | domain | 平台失败处理契约 | platform,error,429,auto_disable,熔断,purge,stream,status | active | # Platform Error Handling  何时被读: 改 proxy 失败处理 / 加平台 / 调 auto… |
| domain/trellis-09.md | domain | 平台生命周期契约 | platform,delete,软删,group_platform,purge,lifecycle | active | # Platform Lifecycle  何时被读: 任何改动 `delete_platform` / `purge_… |
| domain/trellis-10.md | domain | 协议 logo 三路 fallback | logo,sync,favicon,simpleicons,clearbit,png | active | # Platform Logo Sync (三路 fallback)  何时被读: 改 `src-tauri/src/g… |
| encoding/trellis-21.md | encoding | JSON 嵌入 script 标签契约 | json,script,application/json,parse,template,embedding,序列化 | active | # HTML JSON Embedding  何时被读: server-side / build-time 模板（Pyt… |
| frontend/auto-fix-downgrade-37.md | frontend | Tauri 拖拽事件 API（macOS WKWebView 限制） | tauri,drag,drop,wkwebview,html5,ondragdropevent | active | # Tauri 拖拽事件 API（macOS WKWebView 限制）  ## 触发场景 Tauri 前端实现文件拖拽… |
| frontend/cpa-drag-import-22.md | frontend | auth-dir 拖拽目标识别（WKWebView 不可靠 best-effort） | authdir,dragtarget,ondragenter,wkwebview,best-effort,退化,DOM target | active | # auth-dir 拖拽目标识别（WKWebView 不可靠 best-effort）  何时被读: 需区分拖入落到 … |
| frontend/cpa-drag-import-23.md | frontend | 多源批量导入 rowId 唯一性模式（baseIdx 偏移） | rowid,unique,多源,import,baseidx,偏移,batch,react key | active | # 多源批量导入 rowId 唯一性模式  何时被读: 多源批量导入/聚合，每条记录需全局唯一 rowId 时 不遵守代… |
| frontend/cpa-drag-import-24.md | frontend | 多源异步解析并发控制（parseInFlightRef 计数） | parseinflight,concurrent,多源,异步,ref,计数,loading,boolean | active | # 多源异步解析并发控制模式（parseInFlightRef 计数）  何时被读: 多源异步操作共享单一 loadin… |
| frontend/modal-state-architecture.md | frontend | PlatformEditForm Modal 架构模式 | modal, state, architecture, PlatformEditForm, usePlatformForm, PlatformPasteCtx, CpaImportModal, SmartPasteModal | active | # PlatformEditForm Modal 架构模式  何时被读: 在 PlatformEditForm 加新 m… |
| frontend/shadcn-infra-30.md | frontend | CSS var live resolution 别名层 | css,var,alias,live-resolution,migration | active | # CSS var live resolution 别名层  ## 技巧 CSS 变量改名时，用 :root 定义别名层… |
| frontend/shadcn-infra-31.md | frontend | shadcn token 运行时切换 | shadcn,theme,token,runtime,css,var | active | # shadcn token 运行时切换  ## 技巧 shadcn 主题 token 在运行时动态切换时，用 `app… |
| frontend/trellis-18.md | frontend | 前端 conventions 强制规则 | frontend,react,component,hook,state,crud,刷新链,modal,invoke | active | # Frontend Conventions  何时被读: sub-agent 改前端代码 (`src/`) 时 谁读:… |
| git/rule-44.md | git | 并行 subtask commit 竞态防护 | git,并行,subtask,commit,竞态,staged,worktree | active | ## 触发场景 同一 worktree 并行跑多个 subtask 时，不同 agent 可能对同一文件产生变更，导致 … |
| i18n/trellis-19.md | i18n | locale 标签跨层一致性 | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | active | # Locale 标签跨层一致性 (zh-Hans BCP47 script)  何时被读: 改 i18n locale… |
| ops/trellis-17.md | ops | 远端 defaults JSON 同步链 | sync,defaults,json,jsdelivr,remote,validate,presets,hash | active | # 远端 defaults JSON 同步链范式  何时被读: 新增 `src-tauri/defaults/*.jso… |
| proxy/trellis-11.md | proxy | HTTP CONNECT 隧道契约 | proxy,connect,tunnel,axum,hyper,TcpStream | active | # Proxy CONNECT 隧道 (HTTP Relay)  何时被读: 改 `src-tauri/src/gate… |
| proxy/trellis-12.md | proxy | handler fallback 路由判定 | proxy,fallback,host,route,mitm,path | active | # Proxy Fallback Host Routing  何时被读: 改 `src-tauri/src/gatewa… |
| proxy/trellis-13.md | proxy | forward proxy absolute-form | proxy,forward,absolute,scheme,relay,host | active | # Forward Proxy Absolute-Form HTTP 转发  何时被读: 改 `src-tauri/sr… |
| proxy/trellis-14.md | proxy | 上游转发 reqwest client 契约 | reqwest,no_proxy,http_client,forward,env,递归 | active | # HTTP Client Forward (上游转发)  何时被读: 改 `src-tauri/src/gateway… |
| proxy/trellis-15.md | proxy | 诊断 header 注入契约 | proxy,header,diagnostic,trace,blind_relay,debug | active | # Proxy 诊断响应 Header (debug build)  何时被读: 改 `src-tauri/src/ga… |
| reuse/auto-fix-downgrade-36.md | reuse | 写代码前查复用 (grep 已有实现) | grep,reuse,复用,组件,utility,抽象,dry | active | # 写代码前查复用 (grep 已有实现)  ## 触发场景 写新函数 / 新组件 / 新 utility 前。  ##… |
| build/shadcn/shadcn-primitives-39.md | shadcn | shadcn add 依赖验证需补装检查 | shadcn,add,dependencies,yarn,tailwind,verification | active | # shadcn add 依赖验证需补装检查  ## 问题 shadcn add 在 yarn4+tailwind4 下… |
| shadcn/rule-41.md | shadcn | radix Select 空值哨兵模式 | radix,Select,空值,哨兵,__none__ | active | ## 触发场景 使用 radix Select 组件时，value 属性需要处理空值/undefined 状态。  ##… |
| shadcn/rule-42.md | shadcn | radix Select number 双向映射 | radix,Select,number,String,Number,双向映射 | active | ## 触发场景 radix Select 的 value 属性只接受 string 类型，需要处理 number 类型数… |
| shadcn/rule-43.md | shadcn | Dialog.open 需显式 null 判断 | Dialog,open,null,Promise,resolve,bool | active | ## 触发场景 Dialog.open 属性需要 bool 类型，但实际控制常来自 Promise resolve 型 … |
| shadcn/rule-45.md | shadcn | popover 独立窗口只读域跳过 shadcn 迁移 | popover,只读,shadcn,迁移,预筛,grep | active | ## 触发场景 popover 独立窗口（TrayConfigTab）是只读展示域，无表单控件，不适用通用 shadcn… |
| shadcn/rule-46.md | shadcn | shadcn Button cva 基类压 svg 16px | shadcn,Button,cva,svg,16px,size-4 | active | ## 触发场景 shadcn Button 组件 cva 基类含 `[&_svg]:size-4` 规则，统一压内部 s… |
| shadcn/rule-47.md | shadcn | dnd-kit SortableList 迁移保留拖拽逻辑 | dnd-kit,SortableList,拖拽,迁移,shadcn,Button | active | ## 触发场景 dnd-kit SortableList 组件迁移时，只需替换内部 button/视觉组件，拖拽逻辑保持… |
| skein/coding-plan-utilization-calib-fix-27.md | skein | task 查重: 同模块非重复, 先看 PRD 边界互引 | skein,dedup,task-boundary,prd | active | dedup/查重判定重叠维度前, MUST 先看两 task 的 PRD 边界条款是否已显式互相引用切割 (如双向标注对… |
| style/trellis-16.md | style | 日志格式 + traceid 契约 | log,trace,traceid,ansi,format,spawn_traced,span | active | # 日志格式 + traceid 取值链  何时被读: 改 `src-tauri/src/logging.rs` 的格式… |
| test/rule-48.md | test | shadcn 迁移测试改行为断言 | shadcn,测试,snapshot,行为断言,className | active | ## 触发场景 shadcn 迁移导致组件 className/结构变化，现有 snapshot 测试会因视觉差异失败。… |
| frontend/theme/shadcn-primitives-40.md | theme | next-themes 与自有主题体系冲突 | next-themes,theme,conflict,shadcn,sonner | active | # next-themes 与自有主题体系冲突  ## 问题 shadcn Sonner 组件导入 next-theme… |
