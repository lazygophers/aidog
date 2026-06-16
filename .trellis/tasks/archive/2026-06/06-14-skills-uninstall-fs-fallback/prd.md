# uninstall fs 兜底删第三方 symlink skill

## 根因

第三方/手动 symlink skill（如 understand-*，由 understand-anything 应用自建 symlink 到 `~/.agents/skills/<name>`）**不在 npx skills 锁文件**。`npx skills remove -s <name>` 返回 "No matching skills found"（exit 0 success=true 但没删）→ 前端以为成功刷新，skill 还在 → 用户看"失效"。

`npx skills list` 扫 `~/.agents/skills/` 目录显示所有子项（含第三方 symlink），但 remove 只删锁文件注册的 skill。list/remove 命名空间不一致。

用户决策（A）：fs 兜底删。突破"全 npx"约束，对称于 enable 用 path 绕锁文件。

## 修复

`src-tauri/src/gateway/skills.rs`：

1. `uninstall(name, scope, proxy)`：
   - 先 npx remove（uninstall_args 现状 `remove -s <name> [-g] -y`，不改）。
   - 若 npx stdout 或 stderr 含 `"No matching skills found"` → 调 `fs_fallback_remove(name, scope)`，汇总结果到 SkillsOpResult（stdout 记删了哪些路径，stderr 记错误）。
   - 否则返回 npx 原结果。

2. 新增 `fs_fallback_remove(name, scope) -> (Vec<String> removed, Vec<String> errors)`：
   - **安全校验**：name 非空、不含 `/`、不含 `..`（防路径遍历，见 pathbuf-starts-with-traversal memory）。
   - **规范存储**：global `~/.agents/skills/<name>`，project `<project>/.agents/skills/<name>`。
   - **各 agent symlink**（仅 global）：扫 `~/` 下所有 `.` 开头目录，若 `<dir>/skills/<name>` 存在则删。覆盖 .claude/.codex/.trae-cn/.gemini/...（不硬编码 agent 列表，通配扫）。
   - **删除语义**：symlink → `remove_file`（删 symlink 不删 target）；目录 → `remove_dir_all`；不存在 → skip。
   - project scope 仅删项目 `.agents/skills/<name>`（不扫 home）。

3. 不改 uninstall_args（npx 命令仍正确用于 npx 管理的 skill）。

## 验证

- `cargo build` + `cargo clippy --all-targets`（0 项目 warning）
- `cargo test`：新增 fs_fallback 路径安全单测（name 含 `..`/`/` 拒绝；合法 name 构造路径）。
- 用户复现：卸载第三方 symlink skill → 成功删除 + list 刷新消失。

## 不做

- 不改 uninstall_args / 前端 / modal。
- 不动 npx 管理的 skill 路径（仍走 npx remove）。
- 不清理 dangling agent symlink 的 target（第三方应用自管）。
