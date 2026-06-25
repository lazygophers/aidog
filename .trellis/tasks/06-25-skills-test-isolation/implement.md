# 实施计划 — skills-test-isolation

读 prd.md。范围 = `src-tauri/src/gateway/skills/test_bulk.rs` (+ 可能 bulk.rs 加测试钩子)。

## S1 — 读 align_agents/enable_all 实现

`src-tauri/src/gateway/skills/bulk.rs`:
- 读 `align_agents(from, to, scope, ?)` + `enable_all(agent, scope, ?)` 全签名
- 确认: SkillScope::Project 是否走 path (list.rs:169 锁文件 Project→path ✓)
- 确认: shell out npx 命令构造 (是否 `npx skills enable -s <name> -a <slug>` + cwd/scope 参数)
- 看 npx skills CLI 是否支持项目 scope (避免操作全局 ~/.agents)

## S2 — 测试改隔离 (方案 A 改)

`test_bulk.rs` 改:
- `align_agents_different_agents_does_not_panic` + `enable_all_does_not_panic`:
  1. `let tmp = tempfile::tempdir().unwrap();`
  2. `let scope = SkillScope::Project { path: tmp.path().to_path_buf() };`
  3. 调 align_agents/enable_all 传 Project scope
  4. tempdir Drop 自动清理

依赖 `tempfile` crate (查 Cargo.toml 是否已有; aidog 测试常用, 应已引入)。

## S3 — 若 npx CLI 不支持 Project scope 隔离

退路:
- 方案 B: bulk.rs 加 `#[cfg(test)]` 可注入 cmd builder (trait 或 fn 参数), 测试传 mock
- 方案 C: 标 `#[ignore]` + 注释说明手动跑

agent 自判 A/B/C, 优先 A (最小且真隔离)。

## S4 — 验证零副作用

1. 记录测试前 `~/.agents/.skill-lock.json` hash/mtime
2. cargo test 跑 skills 测试
3. 测试后 hash/mtime 不变 → 隔离成功
4. 若变 → 说明仍有全局操作, 回 S2/S3 修

## 验收

1. cargo test 零副作用 (用户 ~/.agents 不变)
2. 测试仍绿 (覆盖逻辑不丢)
3. `cargo test` + `cargo clippy --all-targets -- -D warnings` 全绿
4. 无新 warning

## 执行

单 agent。P0 急, 完成即回传。

## 禁

- 禁改生产业务逻辑 (align/enable 行为不变)
- 禁简单删除测试 (#[ignore] 仅最后兜底)
- 禁 git commit / push
