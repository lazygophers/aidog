# middleware 一键导入默认规则

## Goal

middleware_rule 内置规则（`builtin_rule_specs()` 10 条）首启由 migration 015 `seed_builtin_middleware_rules` 自动 seed。但用户删除内置规则后**无法手动恢复**（仅重启 app 不重 seed——seed 已跑过 migration 不重跑）。

需在 MiddlewareRules 页加"导入默认规则"按钮，复用 seed 幂等逻辑，已存在跳过、不存在补入，返 `{imported, skipped}` 计数反馈用户。

## What I already know

- 默认规则源 `builtin_rule_specs()`（`schema.rs:42-165`）：10 条（3 脱敏 mask + 1 日期改写 + 6 error 分类），is_builtin=1。
- seed 逻辑（`schema.rs:166` `seed_builtin_middleware_rules`）：按 `(name, is_builtin=1)` 幂等判定，已存在跳过（不重新启用，尊重用户禁用）；`pub(crate)` 仅 migration 调。
- 完整先例：`mitm_whitelist_import_defaults`（`mitm.rs:302`）—— command 包裹 INSERT OR IGNORE 逻辑，返 `{imported, skipped}`；前端 `MitmConfig.tsx:377` 按钮 + `mitmApi.importDefaults()` 封装 + i18n `mitm.importDefaults/Done`。
- middleware 命令文件 `commands/middleware.rs`（6 命令：list/create/update/delete/settings_get/settings_set），无 import_defaults。
- 前端 `MiddlewareRules.tsx` 顶部已有「新建规则」按钮（615 行），无"导入默认"。
- startup.rs:248 注册 mitm_whitelist_import_defaults（同模式参考）。

## Requirements

1. **后端 command**：`middleware_import_default_rules(db) -> { imported: u32, skipped: u32 }`
   - 复用 `builtin_rule_specs()` + seed 幂等逻辑（按 name+is_builtin=1 判定，已存在跳过）。
   - **禁重新启用用户禁用的内置规则**（seed 现语义：exists 即 skip，不 update enabled）。
   - 提取 seed 逻辑为可复用 pub fn（返计数），migration 015 与新 command 共用（禁抄第二份）。
2. **注册**：startup.rs `generate_handler!` 加 `middleware_import_default_rules`。
3. **前端 API**：api.ts `middlewareApi.importDefaults()` 封装 invoke。
4. **前端 UI**：MiddlewareRules.tsx 顶部加「导入默认规则」按钮（复用 mitm 模式：loading 态 + toast 反馈 `已导入 X 条默认规则（Y 条已存在跳过）`）。
5. **i18n**：8 语言加 `middleware.importDefaults` / `middleware.importDefaultsDone` key（含 `{{imported}}`/`{{skipped}}` 插值）。

## Acceptance Criteria

- [ ] 点「导入默认规则」按钮：首次导入 10 条内置规则（imported=10, skipped=0）
- [ ] 再次点：imported=0, skipped=10（幂等，不重复插）
- [ ] 用户禁用某内置规则后点导入：该规则不被重新启用（enabled 维持 0）
- [ ] 用户删除某内置规则后点导入：该规则被重新补入（imported 计入）
- [ ] cargo clippy 0 warning；cargo test 通过（含 middleware 现有测试）
- [ ] yarn build clean；8 语言 key 齐全（check-i18n 过）
- [ ] 重启 dev 后按钮可点（新 Rust command 需重启，参 memory `tauri-rust-command-needs-restart`）

## Out of Scope

- 不改 seed 内置规则集内容（`builtin_rule_specs()` 10 条不动）
- 不加"重置/覆盖"语义（纯补缺，不删用户规则、不改用户禁用态）
- 不改 middleware_rule 表结构
- 不加"导入进度" UI（10 条瞬时完成，无需进度条）

## Technical Approach

- 后端：`schema.rs` 把 `seed_builtin_middleware_rules` 拆为「核心 seed fn 返 `(inserted, skipped)`」+ migration wrapper（调核心忽略返回值）。核心 fn 改 `pub`（或放新 pub 入口），command 调核心 fn 包裹 `Db::call_traced`。
  - 或更简：command 内直接 `db.call_traced(|conn| seed_builtin_middleware_rules_counted(conn))`（核心 fn 已是同步 `&Connection` 闭包友好）。
- 前端：复用 mitm importDefaults 模式（MitmConfig.tsx:161-175 + 377 行），按钮放「新建规则」旁。

## Decision (ADR-lite)

- **Context**: seed 仅 migration 跑一次，用户删内置规则后无恢复路径。
- **Decision**: 加 UI 按钮 + command 复用 seed 逻辑，纯补缺语义（已存在跳过）。
- **Consequences**: 用户可随时恢复内置规则；不引入"重置"破坏性语义（避免误删用户自定义）。

## Technical Notes

- 先例文件：`commands/mitm.rs:302` / `MitmConfig.tsx:161,377` / `services/api.ts`（mitmApi.importDefaults）
- 新 Rust command 必须重启 `yarn tauri dev`（memory `tauri-rust-command-needs-restart`）
- i18n key 命名对齐 mitm：`middleware.importDefaults` / `middleware.importDefaultsDone`
