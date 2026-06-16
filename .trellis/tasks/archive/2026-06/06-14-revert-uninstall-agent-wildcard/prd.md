# 回滚 uninstall_args -a 通配 (无效参数)

## 现象

上一修复（fix-skills-uninstall-agent-arg）给 uninstall_args 加 `-a '*'`，实测 `npx skills remove -s brandkit -a '*' -g -y` 报错 `Invalid agents: *`（exit 1）—— npx skills remove 的 `-a` 不接受通配，仅 `--all` 简写内部展开。该修复**引入** bug，用户两次卸载失败。

## 实测真相

- `npx skills remove -s brandkit -g -y`（**无 -a**）→ `Successfully removed 1 skill(s)` exit 0，删规范存储 + 所有 agent symlink（实测 list + fs 确认）。
- `-a '*'` → invalid，exit 1。

结论：原始 `remove -s <name> [-g] -y`（fix-skills-uninstall-single 任务产出）**本就正确**。"不带 -a 时 noop" 是错误推断（未实测）。

## 修复

`src-tauri/src/gateway/skills.rs`：
- `uninstall_args` 去掉 `-a '*'`，回到 `["remove", "-s", <name>] + apply_scope + "-y"`。
- 更新注释：明确"无 -a = 删所有 agent（实测），-a 不接受通配"。
- 单测 `uninstall_args_global` / `uninstall_args_project_no_g` 反转回无 -a 断言。

## 验证

- `cargo build` + `cargo clippy --all-targets`
- `cargo test uninstall`
- **实测**：`npx skills remove -s <name> -g -y` exit 0（已在诊断阶段验证）

## 教训

npx 命令构造改动**必须实测**（exit code + stdout），禁凭 help 文字推断。help 的 `--all` 简写描述（`--skill '*' --agent '*'`）误导 —— 单独 `-a '*'` 被验证层拒绝。

## 附带

诊断实测删了用户 `brandkit` skill（规范存储 + 锁文件条目均清，无法自动恢复）—— 需告知用户手动重装。
