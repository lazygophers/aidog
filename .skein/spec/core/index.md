# SKEIN core 规则索引

类目: arch(2), frontend(1), reuse(1)

| file | category | title | keywords | summary |
|---|---|---|---|---|
| arch/dedup-empty-field-key.md | arch | dedup 禁用设计为空的字段作 key | dedup,空字段,base_url,key,静默丢失,合并,数据丢失,去重 | # dedup 禁用设计为空的字段作 key  何时被读: 写任何 dedup / 去重 / 合并逻辑(HashSet … |
| arch/db-handle-ownership-audit-three-forms.md | arch | DB 拆库访问点归属审计三形式 | db,sqlite,拆库,handle,审计,call_traced,write_conn,read_conn,漏网,归属 | # DB 拆库访问点归属审计三形式  何时被读: 表从一个 SQLite 库拆到另一个库（主库→log.db / platform.db），需把该表所有访问点切到新 handle 时 谁读: trellis-imple… |
| frontend/cpa-drag-import-01.md | frontend | Tauri 拖拽事件 API（macOS WKWebView 限制） | tauri,drag,drop,wkwebview,html5,ondragdropevent,跨平台,onDrop | # Tauri 拖拽事件 API（macOS WKWebView 限制）  何时被读: Tauri 前端实现文件拖拽导入… |
| reuse/trellis-00.md | reuse | 写代码前查复用 (grep 已有实现) | grep,reuse,复用,组件,utility,抽象,dry,新函数 | # Code Reuse Rules  何时被读: 写新函数 / 新组件 / 新 utility 前 谁读: trell… |
