# skills_uninstall 加 result debug log

## 背景

回滚 `-a '*'` 后命令实测正确（`remove -s <name> -g -y` exit 0 真删），但用户报仍失效。怀疑 app 未重编译 Rust（跑旧 -a '*' 版本 exit 1，前端 applyResult 失败分支不刷新列表，用户看旧数据误以为"没删"）。

日志当前只 `command invoked`，无 npx 实际输出 → 无法从日志区分命令版本 / 成败。

## 修复

`src-tauri/src/lib.rs` `skills_uninstall` command：返回前加 `tracing::debug!` 打印 `SkillsOpResult` 的 success / stdout / stderr。

这样日志直接暴露：
- 若 stderr 含 `Invalid agents: *` → app 跑旧 -a '*' 版本（未重编译）。
- 若 success:true + stdout `Successfully removed` → 命令对，问题在前端刷新。
- 若其他 stderr → npx 别的错。

## 验证

- `cargo build` + `cargo clippy --all-targets`
- 用户重启 `yarn tauri dev`（Rust 重编译）→ 卸载 → 看日志 result 行。

## 不做

- 不改 uninstall_args（已回滚正确）。
- 不改前端。
