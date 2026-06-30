# Pending Task: 导出 UX + i18n — 自动展开 + 条目级展示 + label 本地化

> 排队等 `06-30-group-env-vars` finish（共享 `api.ts` Group 类型 + `locales×7`，强冲突）。
> 与 Task A（编程工具 tab）也共享 `locales×7`，须串行。顺序 group-env-vars finish 后裁定。

## 用户原话
「导出不需要点击预览导出项就应该直接展开展示导出的相关的实际内容。导出的范围应该稍微更细一点，比如 mcp/skills等等」

## brainstorm 决策（已用户确认）
- **时机**：排队等 group-env-vars finish
- **展开触发**：勾 scope 后自动拉展开（debounce，去掉「预览导出项」按钮）
- **细化语义**：条目级展示 + 单独勾选（每 mcp/skill 单独行 label 可见可控）

## 现状（已 grep）
- 前端：`src/components/settings/ImportExport.tsx`（1453 行）
  - `ALL_SCOPES`（:38-56）已 10 个：platform/group/group_platform/setting/codex/claude_code/model_price/mcp/middleware/skills
  - 默认勾 4（:124-125）：platform/group/group_platform/setting
  - 导出流程：勾 scope → 点「预览导出项」按钮 `loadExportPreview`（:151）→ 调 `importExportApi.exportPreview`（:160）拉 `ImportPreview` → 展示逐项勾选 → 导出
  - 条目状态：`exportPreview`（:130）/ `exportSelected`（Set）/ `previewing`（:132 loading）
- 后端：`src-tauri/src/gateway/import_export/mod.rs`
  - `SCOPE_*` 常量 10 个（:31-40）含 mcp/skills/middleware/model_price
  - `ImportPreview{items: Vec<ImportItem>}`（:163-173），`ImportItem{scope,key,label,conflict}`（:152-160）—— **已条目级**
  - `exportPreview` 已返全量条目，**后端无需改**

## 实现（纯前端，不动后端）
- `ImportExport.tsx`：
  1. 去掉「预览导出项」按钮 + `loadExportPreview` 手动调用
  2. `useEffect` 监听 `scopes` 变化 → debounce（~300ms）→ 自动调 `importExportApi.exportPreview` → `setExportPreview`
  3. preview 展示：按 scope 分组，每条 `ImportItem` 一行（label + checkbox），复用现有 `exportSelected` Set 支持单独勾选/取消
  4. loading 态（`previewing`）+ 空态（scope 无条目）+ error 态
  5. debounce 防抖：连续勾多个 scope 不每次都拉
- `locales/*.json` × 7：新增条目展开/折叠/加载态文案 key（`importExport.*`）
- 不碰后端、不新增 scope

## 验收
1. 勾选 scope 后无需点按钮，debounce 自动拉条目并展开
2. 每个 scope 的实际条目可见（mcp 服务器名 / skill 名 / 平台名等 label），可单独勾选/取消
3. `yarn build` 零 error / 零新 warning
4. 7 语言 i18n 全覆盖（`check-i18n.mjs` 绿）
5. 性能：skills/mcp 多时拉取有 loading 态，不卡 UI；debounce 不抖
6. UI 风格遵 Liquid Glass

## 交付项 2: 导出 i18n — setting 条目 label 本地化（用户 06-30 第二批请求）

### 用户原话
「导出还存在更多未适配多语言的地方，比如 app:theme 等等」

### 根因（已 grep 实证）
- `app:theme` = setting 条目 key（`<settingScope>:<settingKey>` 格式，`ImportExport.tsx:70` 注释）
- `src-tauri/src/gateway/import_export/apply/mod.rs:144-153` build_items **setting scope**：`label: k.clone()` = **裸 key**（`app:theme` / `app:language` / `proxy:*` / `notify:*` 等）
- 其他 scope label 都人类可读：platform/group=name、group_platform=`g ↔ p`、codex/claude_code=文件路径（`~/.codex/config.toml`）
- **唯独 setting 条目 label 是裸 key** → 导出预览展示 `app:theme` 而非「主题」

### 修复方向（前端，不动后端 — 后端无 locale）
- `ImportExport.tsx`：setting item 展示时，按 settingScope（key 前缀）+ settingKey → i18n label 映射
  - 复用现有 `SETTING_SCOPE_GROUP`（:96，settingScope → 菜单组）架构，扩展加 settingScope/settingKey → 本地化 label 表
  - setting scope 前缀已知（app/group/proxy/notify/ui 等，见 SETTING_SCOPE_GROUP），各前缀的 key 有限可枚举
- `locales/*.json` × 7：新增 setting 条目 label key（`importExport.settingLabel.*`）
- 未命中映射的 setting key → fallback 显示裸 key（不崩）

### 验收
1. 导出预览 setting 条目显示本地化 label（`app:theme` → 「主题」/「Theme」等），7 语言全覆盖
2. 未命中映射的 key fallback 裸 key（不崩，不空）
3. `check-i18n.mjs` 绿
4. 不改后端 build_items（label 仍存裸 key，前端展示层映射）

### 非目标（i18n）
- 不改后端 ImportItem.label（保持裸 key 稳定标识，前端映射展示层解耦）
- 不改其他 scope label（已可读）

---

## 非目标
- 不新增 scope（mcp/skills/middleware/model_price 已独立）
- 不改后端 collect/apply/encrypt
- 不改导入侧 UX（仅导出方向）

## 风险
- 自动拉替代按需：每次 scope 变化都拉，skills/mcp 全量时需 debounce + loading 兜底
- 与 Task A（编程工具 tab）共享 locales×7 → 须串行（group-env-vars → Task A 或本 task → 另一）
