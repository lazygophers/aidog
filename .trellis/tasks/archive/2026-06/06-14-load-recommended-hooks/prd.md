# 修复加载推荐配置时通知 hooks 未自动添加/更新

## Goal

点「加载推荐配置」后，推荐配置里的 `_aidog_hooks:{enabled:true}` 标记（及其它推荐基线项）能正确**添加/更新**进当前草稿，从而保存+sync 后通知 hooks 真正物化到每分组 `settings.{group}.json`（CC hooks）与 Codex 全局 `config.toml`（notify）。当前因浅合并方向错误导致推荐项被现有 config 覆盖、标记进不来。

## What I already know

- `handleLoadRecommended`（`src/pages/Settings.tsx:215`、`src/pages/CodexSettings.tsx:103`）当前 = `{ ...RECOMMENDED_CONFIG, ...config }` —— **浅合并，current 覆盖 recommended**。
  - 后果 1（更新失败）：现有 config 已有的键（如 `_aidog_hooks`、`env`、`permissions`）不会被推荐值刷新。
  - 后果 2（嵌套丢失）：嵌套对象（env/permissions/enabledPlugins）整体被 current 替换，推荐新增的子键进不来。
- `RECOMMENDED_CONFIG` ← `src-tauri/defaults/settings.json`，含 `_aidog_hooks:{enabled:true}` / `_aidog_statusline` / `_aidog_subagent_statusline` 标记。
- hooks 物化链路（后端，**本任务不改**）：保存 → `do_sync_group_settings`（`src-tauri/src/lib.rs:1073`）读 `hooks_marker_enabled(&base_config)` → 为每分组注入 CC hooks + 一次性注入 Codex notify。标记为 true 才物化。
- `materializeStatuslineFields`（Settings.tsx:54）保留 `_aidog_hooks`，不丢标记。
- 无现成 deepMerge 工具（grep 验证）。

## Decision (ADR-lite)

**Context**: 「加载推荐配置」应能 add（推荐新增键）+ update（已有键刷新为推荐值），同时保留用户独有的自定义键。
**Decision**: 把 `handleLoadRecommended` 的浅合并改为**深合并，recommended 优先覆盖 current**，并保留用户独有键（recommended 没有的键不动）。数组（如 `permissions.deny`）按 recommended 替换。**不**改 merge 之外的行为（仍 draft-only，用户手动保存，不 auto persist+sync）。两页（Claude + Codex）同步修，行为一致。
**Consequences**: 加载推荐 = 用推荐基线刷新草稿（hooks 标记必为 true），用户自定义的额外键保留；但用户对推荐键的自定义修改会被推荐值覆盖（符合「加载推荐」语义）。后端零改动。

## Requirements

- 新增通用 `deepMerge(base, override)` 工具（override 优先；嵌套 object 递归合并；数组/标量 override 直接替换；override 没有的键保留 base 值）。放 `src/utils/`（两页共享，遵 code-reuse）。
- `src/pages/Settings.tsx` `handleLoadRecommended` → `deepMerge(config, RECOMMENDED_CONFIG)`（current 作 base，recommended 作 override 优先）。
- `src/pages/CodexSettings.tsx` `handleLoadRecommended` → `deepMerge(config, CODEX_RECOMMENDED_CONFIG)`。
- 合并后 `_aidog_hooks` = `{enabled:true}`（推荐值），setConfig + setEditJson + toast 行为不变。
- 不动 `materializeStatuslineFields` / `handleSave` / 后端 `do_sync_group_settings`。

## Acceptance Criteria

- [ ] `deepMerge` 单元语义正确：override 优先、嵌套递归、base 独有键保留、数组替换。
- [ ] 已有 config（含 `_aidog_hooks:{enabled:false}` 或缺标记）点加载推荐后，草稿中 `_aidog_hooks.enabled === true`。
- [ ] 用户在 config 加的额外 env 键 / 额外 deny 项之外的自定义顶层键，加载推荐后仍在（仅 recommended 覆盖的键被刷新）。
- [ ] 保存后 sync，分组 `settings.{group}.json` 出现 CC hooks（Stop/Notification）、Codex `config.toml` 出现 aidog notify（手动验证一次）。
- [ ] `yarn build` 通过；`yarn check:i18n` 通过（无新增裸 key —— 本任务不新增文案）。

## Out of Scope

- 后端 `do_sync_group_settings` / hooks.rs 物化逻辑（已正确，标记驱动）。
- auto persist+sync on load-recommended（用户明确否决，仍手动保存）。
- 「默认为所有分组注入通知 hook」总开关（NotificationSettings，独立路径）。
- skills-source-grouping 任务（无关，停留 planning）。

## Technical Notes

- code-reuse：合并工具单一实现，两页复用，禁页内各写一份。
- cross-layer：纯前端 TS，无 Rust↔TS 契约改动。
- 数组合并取舍：recommended 替换（非 union）—— permissions.deny/ask、enabledPlugins 等以推荐为准刷新，符合「加载推荐」。
- 验证手动步骤：dev 跑起 → Settings 点加载推荐 → 保存 → 查 `~/.aidog/settings.<group>.json` 含 hooks。
