# src-tauri/.ci

Rust 测试隔离 lint 守卫脚本目录。

## check-test-isolation.sh

扫描 `src-tauri/src/**/test_*.rs` 与 `*_test.rs`，禁止三类违规（防测试代码污染真实环境）：

| # | 违规 | 修复 |
|---|------|------|
| 1 | 裸 `dirs::home_dir()` / `dirs::config_dir()`（不在 `HomeGuard` 上下文） | 包 `crate::gateway::db::test_support::HomeGuard` |
| 2 | `Command::new("(python3\|uv\|node\|git\|sh)").{spawn,output,status}` | mock / `#[ignore = "needs host ..."]` / 删 |
| 3 | `reqwest::get(...)` / `reqwest::Client::new()...send()` | 用 `spawn_stub_upstream` / `spawn_reset_upstream` 本地 stub |

例外：`src-tauri/src/gateway/db/test_support.rs` 自身（`HomeGuard` 定义所在，需 `dirs` 解析 tempdir）；注释行（`//` / `///` 开头）不计。

## 运行

```bash
bash src-tauri/.ci/check-test-isolation.sh
# OK: test-isolation 检查通过（0 违规）  → exit 0
# FAIL: ...                              → exit 1（列出 file:line）
```

## 挂 CI

本仓库 `.github/workflows/` 暂无 `ci.yml` test job（仅 `deploy-docs.yml` / `release.yml`）。
新增 test job 时，在 `cargo test` 前置：

```yaml
- name: test-isolation lint
  run: bash src-tauri/.ci/check-test-isolation.sh
- name: cargo test
  run: cd src-tauri && cargo test
```

在此之前，**手动跑**（每次改测试代码后本地执行一次即可）。

## 关联

- 任务：`.trellis/tasks/07-01-test-isolation-fix/prd.md`
- 审计报告：`.trellis/workspace/nico/research-test-isolation/audit-report.md`
- HomeGuard 定义：`src-tauri/src/gateway/db/test_support.rs`
