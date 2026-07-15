# CPA 独立模块重构 — PRD (主入口)

## 目标
推翻现有 cpa-* 协议寄生 platform 表的架构, 新建独立 `cli-proxy` 模块(独立数据表 + 独立菜单 + 独立 CRUD)管理 CLIProxyAPI 上游 provider 配置。AI 平台页通过新内部平台类型 `cli_proxy` 引用本模块 provider 建平台, 模型从 provider 继承不可选, 路由走新模块拉真实配置。删旧 cpa-* 协议 + 旧 CpaImportModal + 旧 apply 链, 代码层零残留。

用户价值: CPA provider 配置与 platform 解耦 — provider 是可复用上游库(一个 provider 可被多平台引用 / 独立测试 / 独立统计), 平台只管引用与分组; 删旧 cpa-* 寄生协议后路由/统计层干净。

成功长什么样:
- 侧栏新菜单 `cli-proxy`(proxy section), 进去是 provider 列表(增删改 + 测试 + 导入 CLIProxyAPI 配置)
- AI 平台页新建态可"从 cli-proxy 添加" → 选 provider → 建 `cli_proxy` 类型平台(模型区只读, 显示"继承自 provider X")
- 代理转发: `cli_proxy` 平台请求 → 路由从 provider 表拉真实 wire/base_url/key/models → 转发
- 旧 cpa-grok/aistudio/antigravity/vertex 协议 + CpaImportModal + apply 链全删, 代码 grep `cpa-` 零命中(新模块 cli-proxy 不含旧 cpa 字面)

- [x] 目标已定

## 边界

**范围内**:
1. 新独立表 `cli_proxy_provider`(schema migration + db 模块)
2. 新 Protocol 变体 `CliProxy`(serde `cli-proxy`), 平台类型 = 引用 provider 的内部类型
3. 路由扩展: candidate resolve 时 `CliProxy` 平台 → join provider 表拉 wire/base_url/key/models 注入 endpoint
4. 新 crate `commands_cli_proxy`: provider CRUD + test + create_cli_proxy_platform(建引用平台) + 解析导入(复用旧 parser 逻辑迁入)
5. 前端新 `CliProxy.tsx` 页(侧栏菜单, proxy section) + provider 管理 UI
6. 前端 PlatformEditForm 新建态加"从 cli-proxy 添加"入口
7. 删旧 cpa-*: 后端(cpa_import 模块 + 4 Protocol 变体 + converter 4 arm + presets 4 条目 + 3 Tauri command) + 前端(CpaImportModal + applyCpaToForm/runBatchCreateFromCpa + cpaImportApi + MappedPlatform 等类型 + Protocol union 4 cpa-* + i18n cpaImport.*)
8. 数据 migration: 删旧 `platform_type LIKE 'cpa-%'` 平台行(防 enum 删除 panic)

**范围外(非目标)**:
- 不改 group / quota / pricing / peak_hours / model_test 核心逻辑(新 protocol 走通用路径)
- 不改 proxy_log schema(provider 不独立落库, 仍按 platform_id)
- 不做旧 cpa-* 平台 → 新 provider 迁移(用户重新配置; 符合"完全推翻")
- 不改 SmartPasteModal / MultiKeyPreview(独立路径)

**已知约束**:
- 路由层当前零 cpa 特判(research A4), wire 由 endpoint.protocol 决定 → 新 CliProxy 路由在 candidate resolve 注入 endpoint, 不改 selection/ordering
- DB 无 cpa 专属列(research A5), 新表是净新增
- 删 Protocol enum 变体前必须先 migration 清 `platform_type LIKE 'cpa-%'` 行, 否则 from_str unwrap panic(research C2)
- 前端 PROTOCOLS 动态派生自 presets(research B5), 删 JSON 条目自动消失
- parser.rs(解析 CLIProxyAPI config.yaml/auth-dir)逻辑有价值, 迁新模块作"导入"入口

- [x] 边界已定

## 验收标准
- [ ] 侧栏新菜单 `cli-proxy`(proxy section, 与 platforms 同段), 8 locale i18n 补全
- [ ] CliProxy 页: provider 列表 + 新增/编辑/删除/测试 + 导入 CLIProxyAPI 配置(复用旧 parser)
- [ ] 新表 `cli_proxy_provider` schema migration 幂等
- [ ] Protocol::CliProxy 变体就位, serde roundtrip
- [ ] PlatformEditForm 新建态"从 cli-proxy 添加" → 选 provider → 建 cli_proxy 平台
- [ ] cli_proxy 平台模型区只读(显示"继承自 provider X"), 路由从 provider 拉 models
- [ ] 代理转发 cli_proxy 平台 → 路由拉 provider 配置 → 成功转发(走通用 endpoint wire)
- [ ] 旧 cpa-grok/aistudio/antigravity/vertex 4 协议删除, 代码 grep `CpaGrok\|CpaAistudio\|CpaAntigravity\|CpaVertex\|cpa-grok\|cpa-aistudio\|cpa-antigravity\|cpa-vertex` 零命中
- [ ] 旧 CpaImportModal + applyCpaToForm + runBatchCreateFromCpa + cpaImportApi 删除
- [ ] platform-presets.json 4 cpa-* 条目删除
- [ ] DB migration 删旧 `platform_type LIKE 'cpa-%'` 平台行 + 同步清 proxy_log/stats_agg_hourly 含这些 platform_id 的历史行(防 panic + 干净统计, 用户授权破坏性)
- [ ] cargo clippy --workspace 无新增; cargo test --workspace 过
- [ ] yarn build 过; check:i18n 全绿

## 索引
- 详细设计: [design.md](design.md)
- 调研收敛: [research/](research/) (4 笔记: backend/frontend/cleanup-impact/menu-and-stats)
- 任务/子任务/调度: task.json (脚本真值, `skein.py subtask list cpa-standalone-module`)
