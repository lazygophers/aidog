---
updated: 2026-06-12
rewrite-version: 1
supersedes:
  - component-guidelines.md
  - directory-structure.md
  - hook-guidelines.md
  - quality-guidelines.md
  - state-management.md
  - type-safety.md
authored-by: trellisx-spec
mode: optimize
---

# Frontend Conventions

何时被读: sub-agent 改前端代码 (`src/`) 时
谁读: trellis-implement sub-agent / main
不遵守的代价: 与现有模式不一致 → 增量变更成本指数增长

---

## Directory Structure (MUST)

> 违反代价: 文件放错层 → 后续 agent 按约定 grep 找不到 → 重复造同名文件 / import 路径混乱。

- 新页面必须放 `src/pages/<PascalCase>.tsx`
- 共享组件放 `src/components/`，禁嵌套 >1 层子目录
- 主题文件放 `src/themes/<name>.ts`，必须导出 `ThemeDefinition`
- 服务层 API 放 `src/services/api.ts`，按 resource 分 namespace
- i18n JSON 放 `src/locales/<locale>.json`
- Context provider 放 `src/context/`
- 验证: `find src -type f -name '*.tsx' -o -name '*.ts' | sort` 必须与上述结构一致

## Component Patterns (MUST)

> 违反代价: 引入 CSS Modules / CSS-in-JS → 样式系统割裂、主题切换失效；index 作 key → 列表重排时状态错位。

- 页面组件必须 `export function <PascalCase>()`，用 named export
- 共享组件同理: `export function <PascalCase>(props: <Name>Props)`
- Props interface 必须紧跟组件定义之后、函数签名之前
- 组件样式必须用 inline `style={{}}` + CSS class (`glass` / `glass-surface` / `glass-elevated` / `btn` / `btn-primary` / `input`)
- 禁 CSS Modules / styled-components / CSS-in-JS — 本项目仅用 inline style + 全局 CSS class
- 导航项必须遵循 `NavItem` 接口 (`{ id: string; icon: string; labelKey: string }`)
- 条件渲染用 `{condition && <Component />}`，禁 ternary 返回 null
- 列表渲染必须带 `key={item.id}`，禁用 index 作 key
- 验证: `grep -rn 'className=' src/ | grep -vE 'glass|btn|input|text-|surface'` 命中量必须 ≤ 5（仅允许少量自定义 class）

## State Management (MUST)

> 违反代价: 新建 store / 绕过 AppContext 读写 localStorage → 状态双源不一致、持久化漏写、主题/语言切换不生效。

- 全局设置（locale / theme / mode）必须走 `AppContext` + `useApp()` hook
- 禁新建全局 store / Zustand / Redux — 扩展 `AppContext` 即可
- 组件本地状态（表单 / loading / 编辑态）用 `useState`，禁提升到全局
- 设置持久化必须走 `localStorage` key `"aidog-settings"`
- 禁在 Context 外部直接读写 localStorage — 必经 `loadSettings` / `saveSettings`
- 异步数据获取必须用 `useEffect(() => { load() }, [])` + `useState<boolean>` loading pattern
- 验证: `grep -rn 'localStorage' src/ | grep -v 'AppContext'` 必须 0 行

## API Layer (MUST)

> 违反代价: invoke 散落各文件 / 静默丢错 → 后端 command 改名时编译期不报、运行时静默失败难排查。

- invoke 契约 (泛型标注 / 集中 api.ts / 字段名 snake_case / 新 command 必同步前端) 见 [Cross-Layer Rules](../guides/cross-layer-rules.md#taurireact-boundary-must)，本节不重复
- API namespace 必须按 resource 拆分 (`platformApi` / `groupApi` / `mappingApi` / `proxyApi` / `configApi`)
- 入参类型必须用独立 `interface` 定义，禁 inline `{ [key]: string }`
- 错误处理: `try/catch` 包裹，`catch` 至少 `console.error`，禁静默丢弃

## Type Safety (MUST)

> 违反代价: 用 `any` / `string` 代替 union → 后端字段改动编译期不报错、运行时崩；漏同步 interface → 前后端字段静默错位。

- 可枚举字符串类型必须用 union type (`"anthropic" | "openai" | "glm" | "kimi"`)，禁用 `string`
- 共享类型必须定义在 `src/services/api.ts`（业务）或 `src/themes/types.ts`（主题）
- 组件 Props type 必须用 `interface`，禁 `type` alias for props
- 禁 `any` / 禁 `as unknown as X` 断言链
- 可选字段必须标 `?`，禁用 `| undefined`
- 新增后端数据结构必须同步在 `api.ts` 添加对应 interface
- 验证: `grep -rn 'any' src/ --include='*.ts' --include='*.tsx'` 必须 0 行

## Hooks (MUST)

> 违反代价: 不用 `use` 前缀 → React lint 规则失效、依赖检查漏报；≥2 组件复用却不提取 → 逻辑分叉、bug fix 不传播。

- 自定义 hook 必须以 `use` 前缀命名，放 `src/hooks/` 或组件文件内
- 获取全局设置必须用 `useApp()`，禁直接 `useContext(AppContext)`
- i18n 翻译必须用 `const { t } = useTranslation()`
- 数据获取必须用 `useEffect + useState<loading>` pattern（见 State Management）
- 新增 hook 若被 ≥ 2 组件使用，必须提取到独立文件

## Deep-Link 导入契约 (MUST)

> 违反代价: 缓存重放 → 用户重访页面时旧导入弹窗反复弹；URL 承载格式与接收端解析不匹配 → 唤起后导入静默失败。D2 (`20fc5f42` 修热路径重放) + D3 实证，D4 复用。

`aidog://<entity>/import?data=<base64>` (entity = platform|mcp|skill) 唤起链路契约：

- **App.tsx 分发**: deep-link handler 解析 entity/action/data → 写 `window.__aidogDeepLink[entity] = {action, data}` 缓存 → `if(entity==="...") setActiveNav(entity)` 条件挂载目标页 → dispatch `aidog:${entity}` CustomEvent
- **目标页双路单次消费 (MUST)**: 目标组件 useEffect 两路都消费 deep-link，**两路都 MUST `delete window.__aidogDeepLink[entity]` 防重放**：
  - mount 路径: 挂载时读 `window.__aidogDeepLink[entity]` → 先 `delete` 再用本地引用处理（禁 use-after-delete，存局部变量）
  - 运行时路径: `addEventListener('aidog:${entity}', ...)` handler 也 `delete` 同键
  - 两路互斥: 同一 deep-link 仅产生 1 cache + 1 event，任一路消费后 delete，另一路读不到 → 单次消费
- **URL base64 恒取接收端可解析格式**: URL 承载的可导入契约恒用**接收端严格解析器接受的格式**。例: mcp `mcp_import_json` 走 `serde_json` 严格解析 → URL base64 恒 `JSON.stringify`（YAML 会被拒）；与 ShareModal 弹窗展示格式（用户可切 yaml/json/base64）解耦——弹窗展示 ≠ URL 承载
- **SmartPasteModal 粘贴预填**: 粘贴导入复用 SmartPasteModal `initialText` prop（预填 + 跳过自动读剪贴板）
- **ShareModal 泛化**: 分享弹窗 `<T extends object>` 泛型 + `title`（替代原 `platformName`）+ 可选 i18n keys (`titleKey`/`warningKey`/`urlScheme`/`copyUrlKey`)，向后兼容平台/mcp/skill 调用点
- 验证: `grep -n "__aidogDeepLink" src/App.tsx src/pages/*.tsx` — 写入点(App.tsx) + 消费点(目标页 mount + addEventListener) + delete 锚点(双路) 齐全；重访页面不重播导入

## i18n (MUST)

- 所有用户可见文案必须用 `t("key")`，禁硬编码中/英文字面量（含 placeholder / title / aria-label / 错误提示）
- i18n JSON 放 `src/locales/<locale>.json`，flat dot-notation key（`section.subsection`）
- 支持语言：zh-CN / en-US / ar-SA / fr-FR / de-DE / ru-RU / ja-JP / es-ES（8 locale，ar-SA 走 RTL）
- 新增 `t("key")` 时该 key **必须 8 locale 同步补全**——只补 zh-CN 或部分 locale 会导致其他 locale 显示裸 key（如 `env.CLAUDE_CODE_MAX_OUTPUT_TOKENS`）。这是反复出现的高频遗漏，**新增任一 key 必须 grep 全部 8 个 locale 文件确认存在**
- 动态模板 `t(\`prefix.${var}\`)` 必须枚举所有变量取值，每个值对应 key 8 locale 全补；变量取值新增时同步补 key
- **t(变量) 路径**（第三类，最易漏）: `t(item.labelKey)` / `t(section.labelKey)` / `t(g.key)` / `t(m.labelKey)` — i18n key 作为对象属性字面量定义、运行时变量传入 `t()`。数据源 = 配置数组（`App.tsx` NAV_ITEMS / Settings sections / Platforms MODES 等）的 `labelKey` / `group` 属性。**新增配置项时该属性值的 key 必须 8 locale 同步补全**（只改配置不补 locale → 菜单/tab 显示裸 key，如 `appSettings.systemTab`）。这是 `App.tsx` NAV_ITEMS 加 tab 时反复遗漏的高频点
- 翻译约定：品牌（AiDog/Claude Code/Anthropic）/协议名/技术常量（env key 名如 `ANTHROPIC_API_KEY`）保留原文不译；插值 `{{var}}` 保留；仅 label/desc 本地化
- 仿函数场景（非组件内）用纯函数 `TFunction` 参数模式注入 `t`，禁直接 import 全局实例
- **check 前必须跑 `node scripts/check-i18n.mjs`，exit 0（零缺失）才可 finish**。脚本检查四类：(A) `t()` 静态 key 8 locale 覆盖 (B) locale 间 key 集合对齐（并集 = 每个 locale）(C) 动态模板清单人工审计 (D) `labelKey`/`group` 属性字面量覆盖（堵 t(变量) 盲区）
- 验证: `node scripts/check-i18n.mjs` exit 0；新 key 落地后 `git diff src/locales/` 应见 8 文件同步改动

### Protocol Metadata 多语言 (MUST)

> 违反代价: protocol metadata (name/desc/source_urls/homepage/logo_url) 字段散乱 → 后续 task 重复决策 + check-i18n 漏校 + 跨层 Rust↔TS 边界 struct 死代码。来源: 07-07-protocols-i18n-name-desc-search。

- **(a) metadata 多语言位置**: protocol name/desc 多语言放 `platform-presets.json` 内嵌 `{name: {<locale>: "..."}, desc: {...}}`, **非** `src/locales/<locale>.json` (后者仅 app UI 文案, dot-notation key)。JSON locale key 与 i18next locale 统一用 BCP47 (`zh-Hans` script 子标签)。
- **(b) check-i18n.mjs 第 5 类 (E 段)**: 校验 `platform-presets.json` 每 protocol name/desc 8 locale 完整性 (零空 string)。**新增 protocol metadata 字段 (source_urls/homepage/logo_url 等含 locale 维度的字段) MUST 扩展 E 段校验**。
- **(c) PROTOCOLS[].label 硬编码保留**: `src/domains/platforms/constants.ts` 的 `PROTOCOLS[].label` 硬编码**禁删**, 作内部匹配 fallback (`platformPaste`/`ccswitchMatch`/`usePlatformForm`/`PlatformCard` 用, 非用户直见)。仅用户可见 UI (`SearchableProtocolSelect`/`Sub2ApiImport`/未来 logo 展示) 派生本地化 label via `getProtocolLabel`/`getProtocolLabelMap`/`getProtocolDesc` async helper (复用 `defaults.ts` `docPromise` 单次 RPC 模式, fallback locale→en-US→protocol key)。
- **(d) 跨层边界 — 禁加 Rust struct**: protocol metadata 字段加 `platform-presets.json` + TS 类型 `src/domains/platforms/defaults.ts DefaultsDoc` 协议条目加可选字段; **禁加 Rust `ProtocolPreset` struct** (`get_defaults_json` 透传 raw String, serde 仅校验 `last_updated` 不解析 protocol 字段, 加 struct=死代码)。
- **(e) 已移除**: locale-rename (07-06) 前的 BCP47 (`zh-Hans`) ↔ i18next (`zh-CN`) locale 桥接映射 `LOCALE_TO_DEFAULTS` 已随 rename 一并删除——两端 locale 现统一为 `zh-Hans`, 直接用 i18next locale 作 `DefaultsLocale` 查 name/desc 即可。

## Large File Split — facade 模式 (MUST)

> 违反代价: 巨石文件 (>800 行) → 增量改动成本指数增长、merge 冲突频发、agent 上下文爆炸；拆分不守契约 → 外部 import 路径 churn + 业务逻辑迁移丢块。Groups/Platforms/AppSettings/Logs/Skills/Mcp/PopoverConfigTab/StatusLineSection 均已用此模式。

拆分 >800 行文件统一走 facade + 子目录模式：

- **facade 保留同名 export**: 拆后 `<Xxx>.tsx` 退化为编排 facade（仅 mount hook + 子组件，行数 <60），MUST 保留原 `export function <PascalCase>` 签名 → 外部 import 路径零 churn（`grep -rn "from.*pages/Xxx"` 消费点不改）
- **子目录 `<Xxx>/`**: 拆出的 hook + JSX 子组件放 `src/pages/<Xxx>/` 或 `src/components/.../<Xxx>/`，**禁建 `index.ts` barrel**（facade 同名 export 已保兼容，barrel 多余且违反"唯一入口"原则）
- **单 hook 抽 state+actions**: 抽一个 `use<Xxx>Data` hook 收全部 state + derived + effect + handler，返回一对象；**禁拆双 hook**（useState + useActions），单 hook 少一层 props 传递更简（useLogsData/useSkillsData/useMcpData 先例）
- **JSX 按区块抽子组件**: 大 JSX 区块（列表/弹窗/预览/编辑器/面板）抽独立 `.tsx`，通过 props 传 hook 返回值
- **纯 .ts 数据表外迁用 re-export barrel（唯一例外）**: 纯 .ts 文件（无 React）的大常量数据表（如 `statusline-gen.ts` 的 SEGMENT_DEFS ~540 行）外迁到 `<name>-segments.ts`，原文件用 `export { ... } from "./<name>-segments"` re-export 保导出兼容。**仅此场景允许 barrel**（非组件无法用 facade 模式，数据/函数分离）
- **逐行外迁零业务变更**: 拆分 = 纯结构搬运，禁顺手改逻辑 / 去"重复" / 简化（标注的 bugfix 注释 / 特殊处理注释原样保留）。逻辑改动另开 task
- **验收基准**:
  - 全仓零 >800 行（`find src \( -name '*.tsx' -o -name '*.ts' \) | xargs wc -l | awk '$1>800 && $2!="total"'` 为空）
  - facade 行数 <60（仅编排）
  - `yarn build` 绿 + `node scripts/check-i18n.mjs` exit 0
  - **t() 调用数前后一致**（`grep -oE '\bt\(' <拆前文件>` vs 拆后 sum 相等 = i18n 零 churn 验证）
  - 外部 import 路径零 churn（消费点 grep 不变）
