# skills 被清理修复

## 现象 (用户报告)

npx skills 安装的 skills 物理文件被清理（`~/.agents/skills/<name>/` 目录消失），时机不明（疑似启动 / 某操作后）。

## 根因 (research/root-cause.md)

无启动/后台自动物理删路径。物理删触发点：
- #2 `skills_uninstall`（用户主动卸载，二次确认）
- #3 `fs_fallback_remove`（#2 兜底，第三方 skill）
- #4 `skills_uninstall_all`（一键卸载）
- #6 `import_skills` 导入 .aidogx 时 enabled=false agent 跑 `npx skills remove -s -a`（默认全选 + 无二次确认 → 误触）

用户「未知什么时候」+ 物理真删 → 最可能 #6 导入误触（用户不知导入会 remove）；#7 缓存写空属假象（物理文件在），但用户报告物理真消失，故 #7 非本次根因，仍防御性修。

## 已定决策 (用户裁定)

1. **现象**: 物理文件真消失
2. **修全部三方向**: #7 缓存写空 + #6 导入误删 + 诊断增强

## 修复范围

### F1 — #7 缓存写空防御 (`src-tauri/src/gateway/skills/cache.rs`)

`list_refresh` (cache.rs:114-122): npx 失败（`list_installed` 返回空且非真空）时**不覆盖**已有缓存。
- 区分「npx 失败返空」vs「真空（真无 skill）」：`list_installed` 增加失败信号（返 `Result` 或加 `npx_ok: bool` 到返回），失败时 `list_refresh` 保留旧缓存 + 返 `stale=true` + 加载失败标志，让前端显「加载失败，显示上次列表」。
- 注释 :116-117 已意识到此问题（"这里仍写空覆盖"），改之。
- 验收：npx 失败时缓存不被空覆盖，前端显示旧列表 + 失败提示。

### F2 — #6 导入误删修复 (`src-tauri/src/gateway/import_export/skills_sync.rs`)

`import_skills` (skills_sync.rs:107-111): **enabled=false 分支不主动 remove**。
- 导入语义只「增」不「减」：只对 enabled=true 的 agent 跑 `npx skills add`，enabled=false **跳过**（不跑 remove）。
- 移除 `build_remove_args` 调用（或保留函数但 import_skills 不再调用）。
- 测试 `import_skills_disabled_agent_runs_remove_gracefully` (skills_sync.rs:333) 改语义：disabled agent 不再触发 remove，验证 no-op。
- 前端加固（`src/components/settings/ImportExport.tsx:136`）：skills scope 条目**不默认全选**（其他 scope 仍默认全选），强制用户显式勾选 skills 才导入。
- 验收：导入含 enabled=false agent 的 .aidogx 不删任何现有启用；skills scope 默认不勾选。

### F3 — 诊断增强

- **物理删除日志** (`ops.rs` disable/uninstall/uninstall_all/fs_fallback_remove + `skills_sync.rs` 若保留 remove)：每次 npx remove/uninstall 执行前记 `tracing::warn!` 含 skill name + scope + 触发源（命令名 / 导入自动）+ 完整 args，便于用户/support 从事后日志定位「谁删的」。
- **HOME env 防御** (`env.rs:62-72` `apply_home_env`)：`dirs::home_dir()` 返 None 时，不再 warn 跳过，而是返回明确错误（list/ops 拒执行 + 前端显「HOME 目录无法解析」），避免假空缓存。
- **list_refresh npx 失败日志**：记 warn 含 stderr 摘要。
- 验收：物理删除有 warn 日志可追溯；HOME None 时明确报错而非静默空。

## 验收

1. F1: 模拟 npx 失败，缓存保留旧值，前端显失败提示（非空列表）
2. F2: 导入 enabled=false 的 .aidogx，现有 skills 启用不被 remove；skills scope 默认不勾选
3. F3: 物理删除操作有 warn 日志（含触发源）；HOME None 返明确错误
4. `cargo test` + `cargo clippy --all-targets -- -D warnings` + `yarn build` 全绿
5. 测试用例更新（import_skills disabled 不 remove / cache 失败保留 / HOME None 报错）
6. i18n：新增 key（加载失败提示 / HOME 错误 / skills 不默认勾选提示）补全 8 语言

## 不修

- #1/#5 disable/align（用户主动 toggle，半物理可恢复，非 bug）
- #2/#3/#4 主动卸载（二次确认，用户必记得；仅加 F3 诊断日志）

## 待确认

无（决策已定）。implement.md 见后续。
