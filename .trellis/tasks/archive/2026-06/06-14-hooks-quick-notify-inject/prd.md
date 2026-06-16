# Hooks 区加快速注入/移除通知 hook 入口（编辑器可见）

## Goal

在 Claude Code 设置页 **Hooks 区**（`HooksSectionInline`）顶部加「注入通知 hook / 移除」快捷按钮：点注入 → 把 notify 的 `Stop`(complete) + `Notification`(waiting) hooks 填进**草稿** `config.hooks`（编辑器立即可见）+ 置 `_aidog_hooks.enabled=true`；正常保存 → sync 物化全分组 CC hooks + Codex 全局 notify。解决「Hooks 区永远空、用户以为 notify 没配」+「Hooks 区无快速 notify 入口」两个 UX 痛点。

## What I already know（诊断结论 / 精确锚点）

- **地面真相**：notify hooks 实际已生效 —— 查 `~/.aidog/settings.*.json` 全 7 分组都有 `Stop`→complete.py / `Notification`→waiting.py，Codex `config.toml` 有 notify，脚本在 `~/.aidog/scripts/`。**后端物化链路正常**。
- **"还是没有" 真因 = UI 可见性**：设置页 Hooks 区读 base config `hooks` 字段（空）。当前分组 hooks 由「默认注入总开关」`set_default_hooks_enabled`（lib.rs:2279）来 —— 它**只写 `_aidog_hooks` 标记、不填 base config.hooks**，物化只发生在 sync→分组文件。故 base config.hooks 空 → Hooks 编辑器空。
- **可复用原语**：
  - `gateway::hooks::build_hook_script(notif_type)` 生成脚本内容；`generate_hook_scripts(invoker)`（lib.rs:~2119）写脚本到 `~/.aidog/scripts/` + 返回 `ScriptPaths{complete, waiting}`（含 invoker 命令串 `uv run --script <path>` / `python3 <path>`）。
  - `gateway::hooks::inject_claude_code_hooks(&mut config, &scripts)`（hooks.rs:150）把 `{Stop:[...], Notification:[...]}` 写进 config["hooks"] + 打 `_aidog_hooks` 标记，按脚本文件名去重（幂等）。
  - `resolve_script_invoker(&db)`（uv/python3 探测）。
- **接入点**：`src/components/settings/editors.tsx` `HooksSectionInline`（Settings.tsx:468 渲染，props = `hooksValue` + `updateField` + `t`）。注意 editors.tsx 内 `HooksSection`(3227) 与 `HooksSectionInline`(3773) 有重复 JSX —— 快捷按钮加在 Inline 即可（Settings 用的是 Inline），如能低成本共享则共享。
- `notificationApi.injectHooks/removeHooks`（api.ts:948）是**即时副作用**命令（直写 DB + sync），**不用**于本任务（本任务走草稿+正常保存，保持 Hooks 编辑器 draft 模型一致）。
- 8 locale：ar-SA/de-DE/en-US/es-ES/fr-FR/ja-JP/ru-RU/zh-CN。`yarn check:i18n` 强制全覆盖。

## Decision (ADR-lite)

**Context**: 需要 Hooks 区可见 + 快速 notify 入口，且不破坏 Hooks 编辑器的「加载→改草稿→保存」模型。
**Decision**: 新增**只读式后端命令** `build_notify_hooks_fragment(db) -> serde_json::Value`（返回 `{Stop:[...],Notification:[...]}`，内部 `resolve_script_invoker` + `generate_hook_scripts` + 空对象走 `inject_claude_code_hooks` 后取 `config["hooks"]`，**不写 DB、不 sync**，仅确保脚本文件存在）。前端 Hooks 区按钮把 fragment 合并进**草稿** `config.hooks` + 置 `_aidog_hooks.enabled=true`，由用户正常保存触发 sync（既有链路）物化 CC 全分组 + Codex notify。
**Consequences**: Codex notify 经 `_aidog_hooks` 标记在保存 sync 时物化（不需 Codex 专属前端）。注入即时可见于编辑器，行为与其它字段一致（dirty→保存）。与既有标记物化幂等去重，无双注入。

## Requirements

- **后端**（`src-tauri/src/lib.rs` + 复用 `gateway/hooks.rs`）：
  - 新 command `build_notify_hooks_fragment(db: State<Db>) -> Result<serde_json::Value, String>`：`resolve_script_invoker` → `generate_hook_scripts`（确保脚本落盘）→ 构造空 `json!({})` → `inject_claude_code_hooks` → 返回其中 `hooks` 子对象（即 `{Stop,Notification}`）。不写 DB、不 sync。
  - 注册到 `invoke_handler`（lib.rs:~3419 列表）。
- **前端**：
  - `src/services/api.ts` `notificationApi` 加 `buildNotifyHooksFragment(): Promise<Record<string, any>>`（invoke `build_notify_hooks_fragment`）。
  - `HooksSectionInline`（必要时 `HooksSection`）顶部加快捷区：
    - 「注入通知 hook」按钮：`await notificationApi.buildNotifyHooksFragment()` → 把返回的 Stop/Notification matcher 组**深合并/并入**当前 `hooksValue`（按脚本文件名去重，避免重复项）→ `updateField("hooks", merged)` + `updateField("_aidog_hooks", {enabled:true})`。
    - 「移除」按钮：从 `hooksValue` 删除 command 含 `aidog-notify-complete` / `aidog-notify-waiting` 的 handler（空 matcher 组/空 event 一并清）→ `updateField("hooks", cleaned||undefined)` + `updateField("_aidog_hooks", {enabled:false})`。
    - 已注入态检测：`hooksValue` 中存在 aidog-notify command → 显示「已注入」+ 移除按钮；否则显示注入按钮。busy 期间禁并发。失败 toast/inline 提示。
  - 文案走 i18n（8 locale 全加），key 复用/新增：`notif.hookInject`(已有"注入")、`notif.hookRemove`(已有"移除") 可复用；新增 `settings.hooksQuickTitle`、`settings.hooksQuickDesc`、`settings.hooksNotifyInjected`（已注入态）等所需 key。品牌名（Claude Code/Codex）保留原文。

## Acceptance Criteria

- [ ] `build_notify_hooks_fragment` 返回合法 `{Stop:[{hooks:[{type:command,command:<invoker complete>}]}], Notification:[{...waiting}]}`，脚本文件已写 `~/.aidog/scripts/`，**未触碰 DB / 未 sync**。
- [ ] Hooks 区点「注入通知 hook」→ 编辑器 Stop + Notification 立即出现 aidog-notify 项，页面进入 dirty。
- [ ] 重复点注入不产生重复 handler（按脚本文件名去重）。
- [ ] 点「移除」→ aidog-notify 项消失，`_aidog_hooks.enabled=false`，用户其它自定义 hook 不受影响。
- [ ] 保存后 sync：分组 `settings.{group}.json` 仍有 hooks、Codex `config.toml` 有 notify（标记驱动，幂等）。
- [ ] `cd src-tauri && cargo build && cargo clippy`（warning 清）+ `cargo test` 通过；新增 command 有单测覆盖 fragment 结构。
- [ ] `yarn build` + `yarn check:i18n` 通过（8 locale 无缺失 key）。

## Out of Scope

- 改既有「默认注入总开关」/ NotificationSettings tab 注入逻辑（保留）。
- 改后端 `do_sync_group_settings` 物化链路（已正确）。
- Codex 设置页独立 notify UI（Codex notify 经标记 sync 物化，不需专属前端）。
- 上一任务的 deepMerge（已合并归档，不回滚）。

## Technical Notes

- cross-layer：新 command 名 / 返回结构是 Rust↔TS 契约，前端 api.ts 类型对齐。读 `.trellis/spec/guides/cross-layer-rules.md`。
- code-reuse：去重/合并逻辑复用 HooksSection 既有 `syncHooks`/matcher 结构，禁另起一套；HooksSection 与 Inline 若共享按钮逻辑则抽公共。读 `.trellis/spec/guides/code-reuse-rules.md`。
- URL 构造规则与本任务无关（脚本内自行推导 /api/notify，既有逻辑不动）。
- 验证手动：dev → 设置页 Hooks 区点注入 → 见编辑器出现 → 保存 → 查 `~/.aidog/settings.<group>.json`。
