# GSB 前端 — Platform/Group/全局 UI + i18n

Parent: `06-13-group-scheduling-breaker` — Group 智能调度与熔断器。共享契约见 `../06-13-group-scheduling-breaker/design.md`。

## Goal

前端落地：Platform 编辑页熔断配置（failure_threshold/open_secs/half_open_max，空=继承全局默认）+ Group 编辑页调度策略选择（含新增 HealthAware/LeastLatency/Sticky）+ 系统设置全局默认（SchedulingBreakerSettings）+ 7 语言 i18n。完成后：三处可配，空值显示"继承默认"，yarn build 过，7 语言无缺键 RTL 正常。

## What I already know
- 依赖 **GA 的 api.ts 契约**（Platform breaker 字段 + RoutingMode 新变体 + SchedulingBreakerSettings + schedulingApi）。GA 冻结后开工。
- 与 **C5（middleware 前端）同改 api.ts/AppSettings/Platform·Group 编辑页 → 前端串行**（C5 完成后再开工，或确认无文件冲突）。
- 前端约定见 spec/frontend/conventions.md；Liquid Glass；i18n 7 语言 ar-SA RTL；formatters/navGuard。

## Deliverable 矩阵
| ID | 交付物 | 类型 | 独立验收 | 优先级 |
| --- | --- | --- | --- | --- |
| GB.1 | Platform 编辑页熔断配置区 | UI | 可配 + 空=继承默认 | P0 |
| GB.2 | Group 编辑页调度策略下拉（含新策略） | UI | 可选 + 默认值 | P0 |
| GB.3 | 系统设置全局默认 SchedulingBreakerSettings | UI | 可配默认值 | P0 |
| GB.4 | i18n 7 语言 | diff | 无缺键 RTL 正常 | P0 |

## Requirements
- GR7 Platform 编辑页熔断字段（空显示"继承默认 N"）；Group 编辑页 routing_mode 下拉补全部策略（现有 + 新增）；系统设置全局默认面板。
- GR8 i18n 7 语言全覆盖，ar-SA RTL 正常。
- 遵循现有编辑页/设置组件模式 + Liquid Glass。

## Acceptance Criteria
- [ ] yarn build（tsc && vite build）通过。
- [ ] Platform/Group/全局 三处可配；空值继承默认生效（UI 显示继承提示）。
- [ ] 7 语言无缺键；ar-SA RTL 正常。

## Definition of Done
- Requirements 全实现 + AC 勾选；变更自动暂存提交。

## Out of Scope
- 后端逻辑（GA）；中间件 UI（C5）。

## Technical Notes
- 改 Platform 编辑页（grep Platforms.tsx + components）、Group 编辑页（Groups.tsx + components）、AppSettings.tsx（全局默认 tab/区）、i18n 资源。
- 只消费 GA 契约，不改 api.ts 契约。
- **前端串行**：开工前合入含 GA 契约的最新 master + 确认 C5 前端改动已合（避免 api.ts/AppSettings 冲突）。
