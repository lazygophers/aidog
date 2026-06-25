# 实施计划 — skills-removal-deep-audit

读 prd.md。范围 = `src-tauri/src/gateway/skills/npx.rs` (run_npx_in_scope 加固) + 可能其他 test 文件。

## S1 — 实测当前 master (止血验证)

worktree 内跑 (主仓 ~/.agents 是用户真实数据, 但 worktree 共享同 HOME — 跑测试仍会影响 ~/.agents):

```bash
# 备份快照
sha256sum ~/.agents/.skill-lock.json
find ~/.agents/skills -type f | sort | xargs sha256sum 2>/dev/null
# 跑全量
cargo test --lib 2>&1 | tail -20
cargo test --all-targets 2>&1 | tail -20
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
# 对比
sha256sum ~/.agents/.skill-lock.json  # 必须不变
```

**注意**: 跑前先备份 ~/.agents 到临时目录 (`cp -a ~/.agents /tmp/agents-backup-$(date +%s)`), 防 S1 本身又删。若发现变化, 从备份恢复 ~/.agents 再继续。

## S2 — 若 S1 变化, 二分

逐模块 `cargo test --lib gateway::skills` / `gateway::import_export` / `commands` 等, 前后对比 ~/.agents, 找具体 fn。

## S3 — 系统性加固 (方案 C 优先)

`src-tauri/src/gateway/skills/npx.rs` 的 `run_npx_in_scope`:

```rust
pub fn run_npx_in_scope(args: &[String], scope: &SkillScope, proxy_url: Option<&str>) -> SkillsOpResult {
    // 现有实现...
}

#[cfg(test)]
pub fn run_npx_in_scope(args: &[String], scope: &SkillScope, proxy_url: Option<&str>) -> SkillsOpResult {
    // 测试编译期拦截: Global scope 真实 spawn 是 bug (会删用户 ~/.agents)
    // 测试必须用 Project scope + tempdir, 或 mock args 纯函数断言
    if matches!(scope, SkillScope::Global) {
        panic!(
            "run_npx_in_scope(Global) 在测试中被调用 — 会真实操作用户 ~/.agents。\
             测试改用 SkillScope::Project {{ tempdir }} 或调 *_args 纯函数断言。args={:?}",
            args
        );
    }
    // Project scope 在测试中也应指向 tempdir, 但不硬拦 (tempdir Project 安全)
    SkillsOpResult { success: true, stdout: String::new(), stderr: "[test-stub] npx not executed".to_string() }
}
```

**但注意**: #[cfg(test)] 同名 fn 覆盖会与 prod fn 冲突 (Rust 不允许同模块同名 fn)。改用:
- 方案 C 改: 在 prod `run_npx_in_scope` 内首行加 `#[cfg(test)]` 守卫块:
  ```rust
  pub fn run_npx_in_scope(args: &[String], scope: &SkillScope, proxy_url: Option<&str>) -> SkillsOpResult {
      #[cfg(test)]
      if matches!(scope, SkillScope::Global) {
          panic!("测试禁 run_npx_in_scope(Global): 会操作用户 ~/.agents。改 Project+tempdir 或 *_args 断言。args={:?}", args);
      }
      // 现有 prod 实现...
  }
  ```
  这样测试编译时, 任何 Global scope 真实 spawn 立即 panic, 不可能静默删。

agent 实现 C 改 (in-fn cfg(test) 守卫)。

## S4 — 加固后重验

S3 落地后重跑 S1 全量, ~/.agents 仍不变 + 所有测试绿 (测试已改 Project/args 断言, 不会命中新 panic)。

## S5 — 用户 skills 恢复

- 查 `~/.agents/skills/` 是否真空 (ls)
- 查 Time Machine / .aidogx 导出 / git 历史 (.skill-lock.json 是否纳入版本)
- 查 `~/.claude/skills/` + `~/.codex/` symlink 反推已装清单
- 不可恢复 → 列 skills.sh catalog 重装清单

## 验收

1. S1+S4 实测 ~/.agents 全跑前后不变
2. S3 方案 C 改落地 (run_npx_in_scope cfg(test) Global panic 守卫)
3. cargo test + clippy --all-targets -D warnings 全绿
4. S5 恢复方案 / 重装清单给出

## 执行

单 check agent。P0 急, 完成即回传。

## 禁

- 禁改生产业务逻辑
- 禁 git commit / push
- 禁在未备份 ~/.agents 前跑全量测试
