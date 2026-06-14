# 导入导出纳入主题设置 (theme style/color/mode)

## Goal

主题设置（themeStyle/themeColor/themeMode）+ locale 当前纯 localStorage（`aidog-settings` key），不在 DB。导入导出只读写 DB，故换机器 / `.aidogx` 导入时主题与语言偏好丢失。本任务把 theme 三字段（+ locale 读路径）迁移到 DB setting 表（scope=`app`），纳入现有 `setting` 导入导出 scope，实现跨机器同步。

## What I already know（现状锚点）

- **theme 持久化**：`src/context/AppContext.tsx`
  - `STORAGE_KEY = "aidog-settings"`，值 `{locale, themeStyle, themeColor, themeMode}`（+ 旧 `themeName` 迁移字段）
  - `loadSettings()`（L69）：**只读 localStorage**，含旧 `themeName` → {style,color} 迁移映射 `LEGACY_THEME_MAP`
  - `saveSettings()`（L96）：**只写 localStorage**
  - `update()`（L130）/ `toggleMode()`（L141）：调 `saveSettings`（localStorage）
  - 启动 useEffect（L107）：写回 localStorage 物化迁移结果
- **locale 已半迁移 DB**（L114-123 useEffect）：
  - 写：`settingsApi.set("app", "locale", { locale })`（**已写 DB**，供后端 proxy 错误消息用）
  - 读：仍只 localStorage（**双写单读**，DB 的 locale 未被前端读回）
- **settingsApi 封装**（`src/services/api.ts:993`）：`get/set/delete/list(scope,key)` → invoke `settings_get/set/delete/list`，通用 KV，scope/key 任意字符串
- **后端 setting 表**（`db.rs`）：scope 现有值 `notification/popover/proxy/tray` + 前端写的 `app`（locale）；通用 CRUD，无 schema 约束
- **导入导出后端零改即可覆盖**：
  - `collect.rs:57` `list_all_settings_raw(db)` → 导出 setting 表**全部行**（scope+key+value 三元组），含 scope=`app`
  - `apply.rs:105-113` 导入时按 `scope::key` 查冲突 → 写回 setting 表（通用，不区分 scope）
  - 即：theme 写入 DB scope=`app` 后，**现有 `setting` scope 导出/导入自动覆盖，后端无需任何改动**
- **ImportExport.tsx**：`ALL_SCOPES` 列 8 scope（`src/components/settings/ImportExport.tsx:19`），`setting` scope label = "代理全局设置"（含 notification/popover/proxy/tray + 将含 app）

## Decision (ADR)

**Context**: theme/locale 纯 localStorage，Tauri webview 数据目录重置/迁移易丢；导入导出无法同步用户偏好。
**Decision**:
- theme 三字段仿 locale 模式写入 DB setting 表（scope=`app`）：
  - key=`theme`，value `{style, color, mode}`（单 key 聚合三字段，而非三个 key，减少 setting 行数 + 原子读写）
  - key=`locale` 已存在（value `{locale}`），保持
- `loadSettings()` 改 async：优先读 DB（scope=`app` key=`theme`/`locale`），DB 缺失回退 localStorage（兼容旧用户 + 首启迁移），再回退默认
- `update()`/`toggleMode()` 写 DB（scope=`app`）+ **保留 localStorage 写**（过渡期双写，防 DB 写失败丢设置；未来可移除 localStorage）
- 首启迁移：DB 无 theme 记录 + localStorage 有 → 写 DB 物化（一次性）
- 导入应用后刷新：`ImportExport.tsx` `handleApply` 成功后，重读 DB scope=`app` 的 theme/locale，更新 AppContext（触发 applyTheme + i18n）
**Consequences**:
- 后端导入导出零改动（setting 通用收集/应用已覆盖）
- locale 从"双写单读"升级为"双写双读"（DB 为权威源，localStorage 过渡兜底）
- theme 从"纯 localStorage"升级为"双写双读"
- 导入 .aidogx 后主题/语言即时生效（无需重启）

## 契约：DB setting 行

```
scope="app"  key="theme"   value={"style":<ThemeStyle>,"color":<ThemeColor>,"mode":<"light"|"dark">}
scope="app"  key="locale"  value={"locale":<Locale>}   # 已存在，保持
```

> theme 用单 key 聚合（非 style/color/mode 三 key）：原子读写 + 减少 setting 行 + apply 冲突粒度合理（整组覆盖或跳过）。

## 实施步骤（前端 AppContext + ImportExport）

### 1. AppContext — 读改 DB 优先
- `loadSettings()` → `async function loadSettings(): Promise<Settings>`
  - 并行 `settingsApi.get("app","theme")` + `settingsApi.get("app","locale")`
  - DB theme 存在 → 用 DB 值；否则 localStorage（含旧 themeName 迁移）；再否则默认
  - DB locale 存在 → 用 DB 值；否则 localStorage；否则 "zh-CN"
  - 兼容：DB 缺失 + localStorage 有 → 标记需迁移
- `AppProvider` 初始化：`useState<Settings>(() => fallbackSyncFromLocalStorage())`（同步默认避免白屏）+ `useEffect` 异步 `loadSettings()` 读 DB 后 `setSettings`（DB 权威覆盖）
- 首启迁移 useEffect：DB 无 theme + localStorage 有 → `settingsApi.set("app","theme", migrated)` 一次性物化

### 2. AppContext — 写改 DB（双写）
- `saveSettings(s)` 内部追加 `settingsApi.set("app","theme",{style,color,mode})` + 保留 `localStorage.setItem`（过渡双写）
- locale 写 DB 已有（L121），保留
- `update()`/`toggleMode()` 调 `saveSettings` 自动覆盖

### 3. AppContext — 暴露 reloadFromDB
- 新增 `reloadFromDB()`：重读 DB scope=`app` theme/locale → `setSettings`（供导入后调用）
- `AppContextValue` 加 `reloadFromDB: () => Promise<void>`

### 4. ImportExport.tsx — 导入后刷新偏好
- `handleApply` 成功（拿到 report）后：调 `useApp().reloadFromDB()`
- 失败/无 setting scope 跳过

### 5. ImportExport.tsx — scope label 更新
- `ALL_SCOPES` `setting` label：从"代理全局设置"扩为含主题/语言（如"全局设置（含主题/语言/代理/通知）"）+ i18n key 8 语言更新

## 验收

- [ ] theme 三字段写 DB（scope=`app` key=`theme`），`settings_list "app"` 可见
- [ ] 首启旧 localStorage 用户自动迁移到 DB（DB 出现 theme 行）
- [ ] DB 有 theme 时读 DB（删 localStorage 仍正确）；DB 无时回退 localStorage
- [ ] 导出含 `setting` scope → `.aidogx` 内含 theme/locale 行
- [ ] 另一机器导入 → theme/locale 即时生效（apply 后 reloadFromDB 触发 applyTheme + i18n.changeLanguage）
- [ ] locale 从 DB 读（修复原双写单读）
- [ ] `yarn build` 通过（tsc 无错）
- [ ] `cargo clippy` + `cargo test` 无新 warning/fail（后端零改，应无影响）

## 风险

- **异步初始化白屏**：loadSettings 改 async，AppProvider 首渲染用同步 fallback（localStorage/默认），DB 读回后覆盖。避免主题闪烁：fallback 用 localStorage（已有值则不闪）。
- **DB 写失败**：保留 localStorage 双写兜底；DB 写 catch 后不阻断 UI（log warn）。
- **导入冲突粒度**：theme 单 key，冲突时整组 overwrite/skip/rename（用户决策），符合预期。
- **locale 双向**：locale 已写 DB（L121），本任务补读路径；确保不重复写或循环。

## Cross-reference

- memory [[import-export-module]]：AES 容器 + 7 scope + 逐项冲突
- memory [[floating-bg-variable]] / [[theme-css-var-names]]：theme 变量体系
- locale DB 写先例：`AppContext.tsx:121`
