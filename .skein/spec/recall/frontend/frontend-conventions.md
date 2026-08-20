---
title: frontend-conventions
name: frontend-conventions
description: 前端 conventions 强制规则
layer: recall
keywords: [前端,约定,conventions,强制规则]
created: 1785516136
inclusion: auto
---

## frontend-conventions

## 前端 conventions 强制规则

前端代码变更必须遵循约定，确保与现有模式一致，减少增量成本。

## Directory Structure (MUST)

- 新页面必须放 `src/pages/<PascalCase>.tsx`
- 共享组件放 `src/components/`，分为 `shared/`（跨页）、`ui/`（shadcn 原语）、`settings/`（设置域）、`platforms/`（平台域）
- **业务派生逻辑放 `src/domains/<domain>/`** 各带 `index.ts` barrel（如 `domains/platforms`、`domains/groups`、`domains/shared`）
- 服务层 API 放 `src/services/api/` 目录，每个 resource 一个 namespace 文件（`platforms.ts`、`groups.ts` 等），由 `src/services/api/index.ts` barrel 统一导出
- i18n JSON 放 `src/locales/<locale>.json`
- Context provider 放 `src/context/`

## Component Patterns (MUST)

- 页面组件必须 `export function <PascalCase>()`，用 named export
- 共享组件同理: `export function <PascalCase>(props: <Name>Props)`
- Props interface 必须紧跟组件定义之后、函数签名之前
- 组件样式必须用 inline `style={{}}` + CSS class（glass/glass-surface/btn/input）
- 禁 CSS Modules / styled-components / CSS-in-JS — 本项目仅用 inline style + 全局 CSS class
- 列表渲染必须带 `key={item.id}`，禁用 index 作 key

## State Management (MUST)

- 全局设置（locale / theme）必须走 `AppContext` + `useApp()` hook
- 禁新建全局 store / Zustand / Redux
- 组件本地状态用 `useState`
- 设置持久化必须走 `localStorage` key `"aidog-settings"`
- 异步数据获取必须用 `useEffect(() => { load() }, [])` + `useState<boolean>` loading pattern

## CRUD 刷新链契约 (MUST)

- **全入口扫描**: `platformApi.delete` 等后端真删的 CRUD 入口的全调用点 MUST grep 扫齐，确认无遗漏
- **受影响 state 必刷**: 每入口 MUST 触发受影响 state 全量刷新链（platforms + epoch）
- **禁仅刷关联 state** 致被删实体 stale 残留
- **独立信号优先**: 跨 hook 协作刷新链 MUST 用独立 callback（如 `onPlatformDeleted`），禁复用宽语义 callback
- **hook 级回归测试**: 每入口写 hook 级 renderHook 回归测试

## API Layer (MUST)

- invoke 契约需严格遵守（泛型标注 / 集中 api/ 目录 / 字段名 snake_case）
- API namespace 必须按 resource 拆分（`platformApi` / `groupApi` / `proxyApi` 等）
- 入参类型必须用独立 interface
- 错误处理: try/catch 包裹，禁静默丢弃

## i18n (MUST)

- 所有用户可见文案必须用 `t("key")`
- 新增任一 key 必须 8 locale 全补（zh-Hans/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP/es-ES）
- `t(变量)` 路径（labelKey/group 属性）新增时同步补 key
- **check 前必须跑 `node scripts/check-i18n.mjs` exit 0**

### Protocol Metadata 多语言 (MUST)

- metadata (name/desc) 多语言放 `platform-presets.json` 内嵌 `{name: {<locale>: "..."}, desc: {...}}`，非 `src/locales/`
- `src/domains/platforms/constants.ts` 的 `PROTOCOLS[].label` 硬编码保留为 fallback
- 用户可见 UI 派生本地化 label via `getProtocolLabel` 等 async helper

## Large File Split — facade 模式 (MUST)

>800 行文件统一走 facade + 子目录模式：

- **facade 保留同名 export**: 拆后 `<Xxx>.tsx` 退化为编排 facade（仅 mount hook + 子组件），MUST 保留原 `export function <PascalCase>` 签名
- **子目录 `<Xxx>/`**: 拆出的 hook + JSX 子组件放 `src/pages/<Xxx>/` 或 `src/components/.../<Xxx>/`
- **单 hook 抽 state+actions**: 抽一个 `use<Xxx>Data` hook 收全部 state
- **纯 .ts 数据表外迁用 re-export barrel（唯一例外）**

## 关联

[[tauri-drag-drop-api]]、[[modal-state-architecture]]、[[i18n-key-eight-locales]]
