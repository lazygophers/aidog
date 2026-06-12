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

- 新页面必须放 `src/pages/<PascalCase>.tsx`
- 共享组件放 `src/components/`，禁嵌套 >1 层子目录
- 主题文件放 `src/themes/<name>.ts`，必须导出 `ThemeDefinition`
- 服务层 API 放 `src/services/api.ts`，按 resource 分 namespace
- i18n JSON 放 `src/locales/<locale>.json`
- Context provider 放 `src/context/`
- 验证: `find src -type f -name '*.tsx' -o -name '*.ts' | sort` 必须与上述结构一致

## Component Patterns (MUST)

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

- 全局设置（locale / theme / mode）必须走 `AppContext` + `useApp()` hook
- 禁新建全局 store / Zustand / Redux — 扩展 `AppContext` 即可
- 组件本地状态（表单 / loading / 编辑态）用 `useState`，禁提升到全局
- 设置持久化必须走 `localStorage` key `"aidog-settings"`
- 禁在 Context 外部直接读写 localStorage — 必经 `loadSettings` / `saveSettings`
- 异步数据获取必须用 `useEffect(() => { load() }, [])` + `useState<boolean>` loading pattern
- 验证: `grep -rn 'localStorage' src/ | grep -v 'AppContext'` 必须 0 行

## API Layer (MUST)

- invoke 契约 (泛型标注 / 集中 api.ts / 字段名 snake_case / 新 command 必同步前端) 见 [Cross-Layer Rules](../guides/cross-layer-rules.md#taurireact-boundary-must)，本节不重复
- API namespace 必须按 resource 拆分 (`platformApi` / `groupApi` / `mappingApi` / `proxyApi` / `configApi`)
- 入参类型必须用独立 `interface` 定义，禁 inline `{ [key]: string }`
- 错误处理: `try/catch` 包裹，`catch` 至少 `console.error`，禁静默丢弃

## Type Safety (MUST)

- 可枚举字符串类型必须用 union type (`"anthropic" | "openai" | "glm" | "kimi"`)，禁用 `string`
- 共享类型必须定义在 `src/services/api.ts`（业务）或 `src/themes/types.ts`（主题）
- 组件 Props type 必须用 `interface`，禁 `type` alias for props
- 禁 `any` / 禁 `as unknown as X` 断言链
- 可选字段必须标 `?`，禁用 `| undefined`
- 新增后端数据结构必须同步在 `api.ts` 添加对应 interface
- 验证: `grep -rn 'any' src/ --include='*.ts' --include='*.tsx'` 必须 0 行

## Hooks (MUST)

- 自定义 hook 必须以 `use` 前缀命名，放 `src/hooks/` 或组件文件内
- 获取全局设置必须用 `useApp()`，禁直接 `useContext(AppContext)`
- i18n 翻译必须用 `const { t } = useTranslation()`
- 数据获取必须用 `useEffect + useState<loading>` pattern（见 State Management）
- 新增 hook 若被 ≥ 2 组件使用，必须提取到独立文件
