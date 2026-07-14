# SKEIN recall 规则索引

类目: arch(3), build(1), cross-layer(1), db(2), domain(5), encoding(1), frontend(4), i18n(1), ops(1), proxy(5), style(1)

| file | category | title | keywords | summary |
|---|---|---|---|---|
| arch/trellis-03.md | arch | Workspace crate 边界契约 | crate,boundary,边界,commands,aidog_core,event,依赖 | # Workspace Crate 边界契约  何时被读: commands_* crate 内改源码 / 迁移 com… |
| arch/trellis-04.md | arch | Protocol 枚举变体扩展范式 | protocol,enum,变体,grep,serde,match,union | # Protocol 枚举变体扩展范式  何时被读: 新增 `Protocol` 枚举变体时（新协议 / 新 cp 变体… |
| arch/trellis-05.md | arch | 前端常量派生自后端 JSON 真值源 | derived,constants,docpromise,defaults,派生,presets,async | # 前端派生层（常量 → 后端 JSON 派生）  何时被读: 前端硬编码常量（协议列表 / label 映射 / 颜色… |
| build/trellis-02.md | build | Cargo workspace 重构门禁 | cargo,workspace,crate,build.rs,重构,门禁,下沉 | # Cargo Workspace 重构门禁  何时被读: 单 crate → cargo workspace 多 cr… |
| cross-layer/trellis-20.md | cross-layer | 跨 Rust TS 边界契约 | cross-layer,边界,字段名,类型,rust,typescript,契约,invoke | # Cross-Layer Rules  何时被读: 改动跨越 Rust↔TypeScript 边界的功能时 谁读: t… |
| db/trellis-00.md | db | DB 表设计强制规范 | db,sqlite,schema,表,主键,命名,软删除,setting,迁移,crud | # DB Conventions  何时被读: 新增 / 修改任何数据库表、字段、模型、CRUD、迁移时 谁读: tre… |
| db/trellis-01.md | db | tokio_rusqlite 连接韧性契约 | db,connection,call_traced,reconnect,pool,ConnectionClosed,rusqlite | # DB Connection Resilience  何时被读: 改 `Db` 结构 / DB 调用路径（`call_… |
| domain/trellis-06.md | domain | mock 平台类型规范 | mock,platform,extra,test,builder,error_mode | # Mock Platform Type  何时被读: 改 mock 平台逻辑（adapter/mock.rs / pr… |
| domain/trellis-07.md | domain | Claude Code 订阅透传平台 | claude,passthrough,透传,subscription,header | # Claude Code Passthrough Platform Type  何时被读: 改 Claude Code… |
| domain/trellis-08.md | domain | 平台失败处理契约 | platform,error,429,auto_disable,熔断,purge,stream,status | # Platform Error Handling  何时被读: 改 proxy 失败处理 / 加平台 / 调 auto… |
| domain/trellis-09.md | domain | 平台生命周期契约 | platform,delete,软删,group_platform,purge,lifecycle | # Platform Lifecycle  何时被读: 任何改动 `delete_platform` / `purge_… |
| domain/trellis-10.md | domain | 协议 logo 三路 fallback | logo,sync,favicon,simpleicons,clearbit,png | # Platform Logo Sync (三路 fallback)  何时被读: 改 `src-tauri/src/g… |
| encoding/trellis-21.md | encoding | JSON 嵌入 script 标签契约 | json,script,application/json,parse,template,embedding,序列化 | # HTML JSON Embedding  何时被读: server-side / build-time 模板（Pyt… |
| frontend/cpa-drag-import-22.md | frontend | auth-dir 拖拽目标识别（WKWebView 不可靠 best-effort） | authdir,dragtarget,ondragenter,wkwebview,best-effort,退化,DOM target | # auth-dir 拖拽目标识别（WKWebView 不可靠 best-effort）  何时被读: 需区分拖入落到 … |
| frontend/cpa-drag-import-23.md | frontend | 多源批量导入 rowId 唯一性模式（baseIdx 偏移） | rowid,unique,多源,import,baseidx,偏移,batch,react key | # 多源批量导入 rowId 唯一性模式  何时被读: 多源批量导入/聚合，每条记录需全局唯一 rowId 时 不遵守代… |
| frontend/cpa-drag-import-24.md | frontend | 多源异步解析并发控制（parseInFlightRef 计数） | parseinflight,concurrent,多源,异步,ref,计数,loading,boolean | # 多源异步解析并发控制模式（parseInFlightRef 计数）  何时被读: 多源异步操作共享单一 loadin… |
| frontend/trellis-18.md | frontend | 前端 conventions 强制规则 | frontend,react,component,hook,state,crud,刷新链,modal,invoke | # Frontend Conventions  何时被读: sub-agent 改前端代码 (`src/`) 时 谁读:… |
| i18n/trellis-19.md | i18n | locale 标签跨层一致性 | locale,i18n,zh-hans,bcp47,i18next,presets,rtl | # Locale 标签跨层一致性 (zh-Hans BCP47 script)  何时被读: 改 i18n locale… |
| ops/trellis-17.md | ops | 远端 defaults JSON 同步链 | sync,defaults,json,jsdelivr,remote,validate,presets,hash | # 远端 defaults JSON 同步链范式  何时被读: 新增 `src-tauri/defaults/*.jso… |
| proxy/trellis-11.md | proxy | HTTP CONNECT 隧道契约 | proxy,connect,tunnel,axum,hyper,TcpStream | # Proxy CONNECT 隧道 (HTTP Relay)  何时被读: 改 `src-tauri/src/gate… |
| proxy/trellis-12.md | proxy | handler fallback 路由判定 | proxy,fallback,host,route,mitm,path | # Proxy Fallback Host Routing  何时被读: 改 `src-tauri/src/gatewa… |
| proxy/trellis-13.md | proxy | forward proxy absolute-form | proxy,forward,absolute,scheme,relay,host | # Forward Proxy Absolute-Form HTTP 转发  何时被读: 改 `src-tauri/sr… |
| proxy/trellis-14.md | proxy | 上游转发 reqwest client 契约 | reqwest,no_proxy,http_client,forward,env,递归 | # HTTP Client Forward (上游转发)  何时被读: 改 `src-tauri/src/gateway… |
| proxy/trellis-15.md | proxy | 诊断 header 注入契约 | proxy,header,diagnostic,trace,blind_relay,debug | # Proxy 诊断响应 Header (debug build)  何时被读: 改 `src-tauri/src/ga… |
| style/trellis-16.md | style | 日志格式 + traceid 契约 | log,trace,traceid,ansi,format,spawn_traced,span | # 日志格式 + traceid 取值链  何时被读: 改 `src-tauri/src/logging.rs` 的格式… |
