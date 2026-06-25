# 实施计划 — skills-cleanup-fix

决策见 `prd.md`。先读 prd.md + research/root-cause.md 再动手。

## F1 — #7 缓存写空防御

### F1.1 list_installed 返失败信号
`src-tauri/src/gateway/skills/list.rs:15` `list_installed` 改返 `(Vec<SkillInfo>, bool)`（items, npx_ok）或新增 `ListResult { items, ok }`：
- `run_npx_in_scope` `!res.success` → 返 `(vec![], false)`（npx 失败）
- JSON 解析失败 → `(vec![], false)`（npx 成功但输出坏，也视为失败保守）
- 成功 → `(items, true)`
- 调用方：`cache.rs list_refresh` + `bulk.rs align_agents/enable_all`（后者仍取实时态，忽略 ok 用 items）+ 测试，全部适配新签名。

### F1.2 list_refresh 失败保留旧缓存
`src-tauri/src/gateway/skills/cache.rs:114` `list_refresh`：
```rust
pub fn list_refresh(scope, proxy_url) -> CachedSkills {
    let (items, ok) = list_installed(scope, proxy_url);
    if !ok {
        tracing::warn!(scope = ?, "list_refresh npx 失败，保留旧缓存");
        // 不 write_cache；返回旧缓存 items + stale=true + load_failed=true
        let cached = list_cached(scope); // 取旧
        return CachedSkills { items: cached.items, stale: true, load_failed: true };
    }
    write_cache(scope, items.clone());
    CachedSkills { items, stale: false, load_failed: false }
}
```
- `CachedSkills` struct 加 `load_failed: bool` 字段（serde default=false 兼容旧前端）。
- 更新 doc 注释（删「这里仍写空覆盖」旧措辞）。

### F1.3 前端失败提示
`src/pages/Skills.tsx`：`listRefresh` 返 `load_failed=true` 时显 toast/inline 提示「skills 列表加载失败，显示上次缓存」（i18n key `skills.loadFailed`）。非空列表覆盖。

## F2 — #6 导入误删修复

### F2.1 import_skills 不主动 remove
`src-tauri/src/gateway/import_export/skills_sync.rs:107` `import_skills` enabled=false 分支：
- **删** `build_remove_args` 调用 + `run_npx`，改 `continue`（跳过，不 remove）。
- 注释改：「导入语义只增不减：enabled=true → add；enabled=false → 跳过（保留现有启用，不主动 remove）」。
- `build_remove_args` 函数（:137）若仅 import_skills 调用 → 删函数（减死代码）；grep 确认无其他调用方再删。

### F2.2 测试改语义
`skills_sync.rs:333` `import_skills_disabled_agent_runs_remove_gracefully`：
- 改为 `import_skills_disabled_agent_skips_no_remove`：验证 disabled agent 不触发 remove（no-op），不调 npx。
- 若测试用 spy/mock npx → 断言 remove 未被调。

### F2.3 前端 skills scope 不默认全选
`src/components/settings/ImportExport.tsx:136` `setSelectedItems(new Set(prev.items.map(...)))`：
- 默认全选时**排除 skills scope** 条目：`prev.items.filter(it => it.scope !== "skills").map(...)`。
- 其他 scope（platform/group/group_platform/setting/codex/claude-code）仍默认全选。
- 用户需手动勾选 skills 条目才导入。
- i18n：skills scope 标签加提示「（默认不导入，需手动勾选）」。

## F3 — 诊断增强

### F3.1 物理删除 warn 日志
`src-tauri/src/gateway/skills/ops.rs`：
- `disable` (:133) / `uninstall` (:185) / `uninstall_all` (:161) / `fs_fallback_remove` (:254)：执行 npx remove 前 `tracing::warn!(skill = %name, scope = ?, trigger = "skills_<cmd>", args = ?full_args, "物理删除 skill")`。
- `commands/skills.rs`：command 层加 trigger 标签（`skills_disable` / `skills_uninstall` / `skills_uninstall_all` / `skills_align_agents`）传 ops，或 ops 函数加 `trigger: &str` 参数。

### F3.2 HOME env 防御
`src-tauri/src/gateway/skills/env.rs:62` `apply_home_env`：
- `dirs::home_dir()` None 时：不再静默 warn 跳过。改为 `resolve_home_env` 返 Result 或 list/ops 入口检查 home None → 返明确错误（前端显「HOME 目录无法解析，skills 操作不可用」）。
- 简化方案：`list_installed` 入口先检 `resolve_home_env().0.is_none()` → 返 `(vec![], false)` + 额外 warn「HOME 缺失」（复用 F1 失败信号，不破坏签名）。
- i18n key `skills.homeMissing`。

### F3.3 list_refresh npx 失败日志
F1.2 已加 `tracing::warn!`。补 stderr 摘要（`run_npx_in_scope` res.stderr 截断 200 字符）。

## 测试

- `test_cache.rs`：list_refresh npx 失败保留旧缓存用例
- `skills_sync.rs` 测试模块：disabled agent 不 remove
- `test_env.rs` / `test_list.rs`：HOME None / npx 失败信号
- 现有 ops/list 测试适配新签名

## 验收

1. F1: npx 失败缓存保留旧值 + 前端失败提示
2. F2: 导入 enabled=false 不 remove；skills scope 默认不勾选
3. F3: 物理删除 warn 日志含触发源；HOME None 明确报错
4. cargo test + clippy --all-targets -D warnings 全绿
5. yarn build + check-i18n.mjs 零缺失（新增 key：skills.loadFailed / skills.homeMissing / skills scope 默认不导入提示）
6. warning 必须清

## 执行

worktree 隔离（task.py start hook 建）。派 trellis-implement agent。
