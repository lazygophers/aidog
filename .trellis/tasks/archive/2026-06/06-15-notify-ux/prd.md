# 通知模块易用性增强

## Goal

两个独立但同模块的小优化：
1. **每类型默认模板**：`error`/`custom` 当前无默认模板，补齐；render body 兜底从 `default_title` 改为 `default_template`（项目名存在时），让用户留空 template 时也能看到带项目名的 body。
2. **删除冗余单 group 注入按钮区**：「默认为所有分组注入通知 Hook」总开关（`_aidog_hooks.enabled`）已实现且默认 ON（`defaults/settings.json`）。下方 L392-437 「按 client 单 group 注入/移除」按钮区与总开关语义重叠且误导，删除以简化 UI。

## What I already know

- **总开关已存在**：`set_default_hooks_enabled` command + UI（NotificationSettings.tsx L371-390） + `do_sync_group_settings`（lib.rs L1101+）marker 读取 + `defaults/settings.json` 默认 `_aidog_hooks.enabled=true`。
- **现有 default_template**（models.rs L1677-1683）：仅 `TaskComplete="{project} 完成"` / `WaitingInput="{project} 等待用户输入"`，Error/Custom 返回空字符串。
- **render body 兜底链**（notification.rs L131-140）：template (setting) > content > `substitute_vars(default_title)`。**未走 default_template** → setting.template 空且 content 空时不会用 default_template。
- **单 group 注入命令**（lib.rs `inject_hooks`/`remove_hooks` ~L2250-L2317）：API 保留（防破坏），仅删 UI 入口。

## Decision (ADR-lite)

**Context**：用户两个独立请求合并到一个 task：一键注入应该默认全分组（已实现，删冗余 UI 即可）、每类型默认模板（补齐 + render 走 default_template）。

**Decisions**：
- 后端 `models.rs::default_template()` 补 `Error → "{project} 出错"`, `Custom → "{project} 通知"`。
- 后端 `notification.rs::render()` body 兜底链改为：template (setting) > content > **default_template (项目名存在时)** > default_title。无 project 时仍用 default_title（避免 `{project}` 字面残留）。
- 前端 `NotificationSettings.tsx` 删 hookGroup/hookBusy state + handleInject + groups state (若仅此处用) + HOOK_CLIENTS 渲染区 + i18n key（保留 key 不删，向后兼容）。
- 后端 `inject_hooks`/`remove_hooks` command + `notificationApi.injectHooks/removeHooks` API **保留**（向后兼容；导入导出/外部脚本可能调）。
- 文案与「title 字段语义=项目名」（[[notification-title-project-name]]）协调：body 自带项目名，title 也含项目名 → 重复？inbox 渲染 `${title} · ${typeLabel}\n${body}` 主标题项目名 + 类型；body 再次含项目名是 redundant 但仍可读，作小代价。

**Consequences**：
- 用户少一个困惑入口；总开关成为唯一治理路径。
- 用户自定义 template 不动；只对 template=空时新行为生效。
- 历史 inbox 旧数据不变。

## Requirements

- `models.rs::default_template()` Error/Custom 文案补齐。
- `notification.rs::render()` body 兜底链注入 default_template 分支（仅 vars 含非空 project 时启用）。
- `NotificationSettings.tsx` 删 L392-437 按钮区 + 相关 state/handler/imports（清未用引用避免 TS 警告）。
- 测试：
  - 旧 `render_template_priority` body3 期望 "aidog 通知"（vars 含 project="aidog"，custom 走 default_template）。
  - 新 `render_default_template_fallback` × 4 类型 × 含/无 project 路径。
- `cargo clippy -D warnings` / `cargo test` / `yarn build` / `check-i18n` 全绿。

## Acceptance Criteria

- [ ] 设置页通知 tab 不再有「分组 + 注入/移除」按钮组；只剩总开关 + 每类型设置 + 测试。
- [ ] 通知类型设置中 template 留空时，task_complete/waiting_input/error/custom 四类 body 都自动展示「`<项目名> <类型动词>`」（如 "aidog 出错"）。
- [ ] vars 无 project 时（curl 直接调 /api/notify 不传），body 退化为类型默认名（"Task Complete" 等），不出现字面 `{project}`。
- [ ] cargo / tsc / clippy / check-i18n 全绿。
- [ ] 后端 `inject_hooks`/`remove_hooks` command 仍可调（保留 API）。

## Definition of Done

- 代码 + 测试更新
- worktree commit + merge master + archive
- 自检通过 + cortex 更新关联记忆

## Out of Scope

- 删 `inject_hooks`/`remove_hooks` 后端 command（保留 API）。
- 重做总开关 UI 文案 / 摆位。
- 通知历史 inbox 数据迁移。
- 用户自定义 template 含 {project} 但 vars 无 project 时的字面化处理（用户自己写的，按字面渲染）。

## Files

- `src-tauri/src/gateway/models.rs` — default_template Error/Custom 文案
- `src-tauri/src/gateway/notification.rs` — render body 兜底链 + 新测试
- `src/components/settings/NotificationSettings.tsx` — 删按钮区 + 相关 state/handler/imports
- 不动：lib.rs (inject_hooks/remove_hooks 命令保留)、api.ts (notificationApi.injectHooks/removeHooks 保留)、locales（i18n key 保留）

## Research References

无外部研究 — 现状内部审查 + 用户拍板。
