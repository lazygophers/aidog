# PRD — 导入导出: 补全模块覆盖 + 对齐菜单 IA + 导出逐项细粒度

## 背景

导入导出子系统 (`src-tauri/src/gateway/import_export/`, UI `src/components/settings/ImportExport.tsx`) 现状:
- 导出 7 scope: platform / group / group_platform / setting / codex / claude_code / skills
- **导入侧已有逐条目细粒度勾选** (build_items + Selection 白名单, `apply/mod.rs`), **导出侧仅 scope 级勾选**
- 平台/分组/分组关联拆成 3 个独立 scope 卡片, 与侧栏菜单组织方式不一致
- model_price 后端表存在但 collect/scope 完全没接; mcp/middleware 表存在未接

## 目标 (用户需求, 三件事)

1. **支持更多模块**: 新增 mcp / middleware / model_price 三个导出 scope
2. **对齐菜单 IA**: 导出/导入界面整体按侧栏菜单分组重组; 平台+分组+分组关联**合并为一个「平台」模块**(不再拆三个独立卡片)
3. **导出逐项细粒度**: 每个导出项允许用户选择具体内容, **默认全选**, 可移除部分 / 选中部分 (对称导入侧)

## 关键事实 (调研已确认, 引用)

- scheduling 配置存 **setting 表 scope=scheduling** (`db/schema_early.rs:237` 注释) → 已随 SCOPE_SETTING 导出, **无需新后端 scope**。tray/popover/notification 模板同在 setting 表, 同理已覆盖。故"调度/UI偏好"模块在 IA 上由 setting items 按其 `scope` 字段二次归类呈现, 底层仍 SCOPE_SETTING。
- 真正缺后端 scope 的只有 **mcp** (`mcp_server` 表, `db/mcp.rs`) / **middleware** (`middleware_rule` 表, `db/middleware.rs`) / **model_price** (`model_price` 表, `db/model_price.rs`)。
- `export_to_file(scopes, path)` @ `commands/backup.rs:18`, 注册 `startup.rs:149`。**当前无 selection 参数**。
- `import_apply(path, decisions, selection)` 已有 selection (`backup.rs:103`); `is_selected(selection, scope, key)` @ `apply/mod.rs:210`; `build_items` @ `apply/mod.rs:81`; `Selection = BTreeSet<(String,String)>`, **None = 全选**。
- TS 类型 `ImportExportScope` @ `services/api.ts:1836`; `exportToFile` @ `api.ts:1900`。
- **build_items key 铁律** (见记忆 import-export-module): build_items 造的 (scope,key) 必须与 apply/collect 迭代时构造的 key **逐字一致**。platform 用 `idx:N` (name 非唯一), group 用 group_key/name, group_platform 复合 `<g>::<p>`, setting 用 `<scope>:<key>`。新增 scope 沿用此约定 (mcp/middleware/model_price 用稳定主键, 如 `idx:N` 或唯一 name/model)。

## 设计

### 后端

1. **3 新 scope** (mcp/middleware/model_price):
   - `mod.rs`: 加 SCOPE_MCP/SCOPE_MIDDLEWARE/SCOPE_MODEL_PRICE 常量; Payload 加 3 字段 (`Vec<serde_json::Value>`, `#[serde(default)]`)
   - `collect.rs`: 加 3 收集分支 (调 `db/mcp.rs` / `middleware.rs` / `model_price.rs` 现有 list 全量函数, 取原始行/完整 JSON)
   - `apply/mod.rs`: build_items 加 3 scope 条目枚举 (稳定 key); apply_db 加 3 写入分支 (含冲突检测)
   - 测试: collect→serialize→from_bytes_verified 往返; build_items key 与 apply 一致性

2. **导出逐项 (核心新能力)**:
   - 新 command `export_preview(scopes) -> ImportPreview` (或复用 ImportItem 列表结构): 内部 `collect(全量)` → `build_items(payload, &[])` (conflicts 空) → 返回 items 给前端勾选
   - `export_to_file` 加 `selection: Option<Vec<(String,String)>>` 参数: collect 后用 `is_selected` 过滤 payload 各字段再序列化。**None = 全部导出** (向后兼容)
   - selection 透传链: api.ts → command → collect/filter

### 前端 (`ImportExport.tsx` + `api.ts`)

1. ALL_SCOPES 补 mcp/middleware/model_price (labelKey + icon + 默认 label)
2. **IA 重组**: 导出项按侧栏菜单分组呈现:
   - 代理: 「平台」(合并 platform+group+group_platform 三 scope 为一可展开模块)
   - 扩展: Skills, MCP
   - 规则: 中间件(middleware), 调度(setting scope=scheduling 子集)
   - 系统: 全局设置(其余 setting), Codex, ClaudeCode, model_price 价格, UI偏好(tray/popover/notification setting 子集)
   - setting items 按其 `scope` 字段映射到菜单组 (前端映射表 settingScope→menuGroup)
3. **导出逐项 UI**: 导出前调 export_preview 列出 items → 复用导入侧逐条目勾选组件 (默认全选, scope级+全局全选/反选) → exportToFile(selection)
4. api.ts: ImportExportScope 加 3 值; exportToFile 加 selection 参数; 新增 exportPreview 封装

### i18n

- `src/locales/*.json` 7 语言补新 scope label + IA 分组标题 + 导出逐项 UI 文案 key
- 跑 `scripts/check-i18n.mjs` 验证 4 类对齐
- docs 站点若有导入导出页, 7 语言补说明 (按需)

## 验收标准

- [ ] 导出可选 mcp/middleware/model_price 三新模块, 导入能正确还原 (往返一致)
- [ ] 平台+分组+分组关联在 UI 合并为一个「平台」模块, 不再三个独立卡片
- [ ] 导出/导入界面整体按侧栏菜单分组组织
- [ ] 导出支持逐项勾选, 默认全选, 可部分移除/选中, 导出文件只含选中项
- [ ] export_to_file selection=None 时全量导出 (向后兼容)
- [ ] `cargo test` (import_export 相关) + `cargo clippy` 零 warning
- [ ] `yarn build` 通过 + `check-i18n.mjs` 零缺口
- [ ] build_items (scope,key) 与 apply/collect 迭代 key 逐字一致 (单测断言)

## 非目标

- 不导出 stats/logs 历史数据 (运行时数据, 跨机无意义)
- 不改 .aidogx 容器格式/加密 (format_version 仅 Payload 加字段, serde default 向后兼容旧文件)
- 不新建 scheduling/tray/popover 后端 scope (已在 setting 表)
