# PRD — 测试隔离治理

> 审计报告: `.trellis/workspace/nico/research-test-isolation/audit-report.md`（research agent 2026-06-30 产出，6 处违规 + 4 处 HomeGuard 复制散布）

## 目标
消除测试代码对真实环境（外部进程 spawn / 裸 home_dir / 死端口 TCP / 跨模块 env 竞态）的依赖，加 lint 守卫防再犯。

## 交付项

### D1 — 档1 高优：删 2 处确定 spawn 真实外部进程
- `src-tauri/src/gateway/hooks/scripts/test_scripts.rs:46-92` 删 `event_notify_script_passthrough_runtime`（spawn python3 跑生成脚本；保留前两个纯字符串断言 `script_contains_*`）
- `src-tauri/src/commands/test_script_executor.rs:6-10` 删 `check_uv_runs`（spawn uv 探测，无实质断言，注释「just exercise the path」明示为覆盖率）

### D2 — 中优 4 处改隔离
- `src-tauri/src/gateway/skills/test_env.rs:24-32` `resolve_home_env_returns_dirs_home` 裸 `dirs::home_dir()` → 包 `HomeGuard`，断言返值为 tempdir
- `src-tauri/src/gateway/skills/test_env.rs:46-57` `check_env_does_not_panic_and_is_consistent` 间接 spawn node/npx → 改纯 `probe_env` 单测 或 `#[ignore = "needs host node/npx"]`
- `src-tauri/src/gateway/proxy/test_integration.rs:677-678` 死端口 TCP（`http://127.0.0.1:1`）→ 改 `spawn_stub_upstream(599, "")` 模拟上游错误
- `src-tauri/src/commands/test_fs_autocomplete.rs:4-10` 裸 `dirs::home_dir()` → 包 `HomeGuard`，断言 `expand_path("~") == h.home()`
- `src-tauri/src/gateway/skills/test_npx.rs:56-83, 111-121` 3 处 `run_npx_in_scope(Project{tempdir}, ...)` 加 `#[ignore = "needs host npx"]`（用户裁定：tempdir 隔离写入够，但 spawn 真二进制默认跳过，仅 `--ignored` 显式跑）

### D3 — 档2 HomeGuard 收拢（4 把锁 → 1 把）
1. 扩展 `test_support::HomeGuard` 支持 `CLAUDE_CONFIG_DIR`（吸收 `EnvGuard` 语义，新增 `prev_claude_cfg` 字段）
2. 删 4 处复制：
   - `gateway/claude_integration.rs:142-166`（含删 :141 错误注释「can't import from test_support」——`test_sync_settings.rs:63` 成功 import 证伪）
   - `gateway/mcp/test_domain.rs:14-50`
   - `gateway/skills/test_list.rs:11-54`（EnvGuard + 独立 ENV_LOCK）
   - `gateway/test_codex.rs:8-21`（CodexHomeGuard + 独立 ENV_LOCK）
   改 `use crate::gateway::db::test_support::{HomeGuard, ENV_LOCK};`
3. 反向合并 `CLAUDE_CONFIG_DIR` 回 test_support

### D4 — 档3A grep lint 守卫（防再犯）
- 新增 `src-tauri/.ci/check-test-isolation.sh`（或 `scripts/`）：扫 `test_*.rs`/`*_test.rs` 禁 ① 裸 `dirs::home_dir()` 不在 HomeGuard 上下文 ② `Command::new("(python3|uv|node|git|sh)").{spawn,output,status}` ③ `reqwest::{get, Client::new().*send}`
- 挂 `.github/workflows/ci.yml` test job 前置（若 ci.yml 不存在或结构不适配，落 `scripts/check-test-isolation.sh` + 文档说明手动跑）

## 验收
1. `cd src-tauri && cargo test` 绿（删后不触外部进程，可安全跑）
2. `cd src-tauri && cargo clippy` 零 warning
3. lint 脚本扫出 0 违规（自身能检出已知 6 处被修后归零）
4. `grep -rn "static.*ENV_LOCK\|static.*Mutex" src-tauri/src --include="test_*.rs"` 仅 test_support.rs 一处（4→1）
5. HomeGuard 仅 test_support.rs 定义（4 处复制删净）

## 非目标
- 不改生产代码逻辑（仅测试代码 + test_support 基础设施 + lint 脚本）
- 不上档3 方案 B/C（`#[cfg(test)]` 守卫侵入生产代码 / build.rs AST 解析，成本过高，审计判定现阶段无需）
- 不动已合规的 >99% 测试

## 风险
- D3 改 HomeGuard struct 影响所有 env-mutating 测试 —— 须 cargo test 全跑验证无串扰
- D2 test_integration 改 stub 可能影响该集成测试语义 —— 须保断言意图（上游错误返 5xx）不变

## Open Question（已裁定）
- `gateway/skills/test_npx.rs:56-83, 111-121` Project scope spawn npx → **加 `#[ignore = "needs host npx"]`**（见 D2）
