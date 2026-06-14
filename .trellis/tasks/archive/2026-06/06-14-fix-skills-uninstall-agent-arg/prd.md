# 修复单一 skill 卸载命令缺 -a 参数

## 现象

Portal 修复后 modal 正常弹出、`skills_uninstall` invoke 发出（日志确认），但用户反馈"卸载似乎失败"——skill 未从列表消失。

## 根因

`gateway/skills.rs::uninstall_args` 当前构造 `remove -s <name> [-g] -y`，**不带 `-a`**。

`npx skills remove --help` 语义：
- `-a, --agent` = Remove from specific agents (use '*' for all agents)
- 不带 `-a` 且非交互模式 → npx 不知删哪个 agent 的配置 → noop（命令返回但未实际删除该 skill 在 agent 的启用配置 / 规范存储）。

对比 `uninstall_all` 用 `--all`（= `--skill '*' --agent '*' -y`）正常工作，证明必须显式 `-a '*'` 才删所有 agent。

## 修复

`src-tauri/src/gateway/skills.rs`：
- `uninstall_args(name, scope)` 加 `-a '*'`：`["remove", "-s", <name>, "-a", "*"] + apply_scope + "-y"`。
- 更新单测 `uninstall_args_global`：断言含 `-a` 且其值为 `*`（原断言"不含 -a"反转）。
- `uninstall_args_project_no_g`：补 `-a *` 存在断言。

## 验证

- `cargo build` + `cargo clippy --all-targets`（0 warning）
- `cargo test uninstall`（更新后断言通过）
- 用户复现：Skills 页卸载某 skill → 该 skill 从列表消失。

## 不做

- 不改前端 / modal / Portal（上轮已修）。
- 不改 uninstall_all（已正确）。
