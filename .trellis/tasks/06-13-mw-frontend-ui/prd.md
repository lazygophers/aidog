# 中间件 C5 — 前端 UI + i18n

Parent: `06-13-request-response-middleware` — 8 类请求/响应中间件规则引擎。共享契约见 `../06-13-request-response-middleware/design.md`。

## Goal

在 AppSettings 新增"中间件"tab 管理全局规则（总开关默认 ON + 按 rule_type 子开关 + 规则增删改查），并在 group/platform 编辑页内嵌该作用域规则管理；消费 C1 冻结的 api.ts 契约；7 语言 i18n 全覆盖。完成后：`yarn build` 通过，中间件 tab 可见可操作，7 语言无缺键、ar-SA RTL 正常。

## What I already know
- 依赖 **C1 的 api.ts 契约**（仅契约，C1 冻结后即可并行，无需等 C2/C3/C4）。
- AppSettings.tsx tab 式；设置子组件在 components/settings/（editors.tsx 等）。
- 数值格式化走 utils/formatters.ts；无 react-router，导航本地 state；离页拦截 navGuard。
- i18n 7 语言（zh-CN/en-US/ar-SA/fr-FR/de-DE/ru-RU/ja-JP），ar-SA RTL；UI 风格 Liquid Glass。

## Deliverable 矩阵
| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| D5.1 | AppSettings 中间件 tab（总开关+子开关+全局规则 CRUD UI） | UI | tab 可见可增删改 | P0 |
| D5.2 | group/platform 编辑页内嵌作用域规则管理 | UI | 各页可管理本作用域规则 | P0 |
| D5.3 | i18n 7 语言 key | diff | 无缺键 + RTL 正常 | P0 |

## Requirements
- R12 中间件 tab：总开关默认 ON + rule_type 子开关 + 规则列表 + 增删改表单（rule_type/scope/match_type/pattern/action/config/enabled）。
- R13 group/platform 编辑页内嵌对应作用域规则管理。
- R14 7 语言全覆盖，新增 key 无缺失，ar-SA RTL 正常。
- 内置规则在 UI 标记 builtin，禁删按钮（与 C4 约定），仅可禁用。
- 遵循 Liquid Glass 风格 + 现有设置组件模式。

## Acceptance Criteria
- [ ] `yarn build`（tsc && vite build）通过。
- [ ] 中间件 tab 总开关默认 ON，可增删改规则。
- [ ] group/platform 页可管理本作用域规则。
- [ ] 7 语言无缺键，ar-SA RTL 布局正常。

## Definition of Done
- Requirements 全实现 + AC 勾选；变更自动暂存提交。

## Out of Scope
- 后端逻辑（C1-C4）；规则命中实时统计面板（后续）。

## Technical Notes
- 改 services/api.ts(消费契约) + AppSettings.tsx + components/settings/** + group/platform 编辑页 + i18n。
- **C1 冻结 api.ts 契约后即可并行开工**（与 C2/C3/C4 无文件交集）。
- 改前端前读 guides/frontend conventions（组件/状态/类型/i18n）。
