# 测试隔离审计报告 — aidog `src-tauri/`

- **审计日期**: 2026-06-30
- **审计范围**: `src-tauri/src/**/*.rs` 全部测试代码（101 个含 `#[cfg(test)]` 的文件，1095 个 `#[test]`/`#[tokio::test]` 函数）
- **审计方式**: 静态 Grep + Read（**未跑 `cargo test`**，避免触发真实环境访问）
- **执行者**: Research Agent（仅审计，未改源码）

---

## 总览结论

**整体隔离质量良好**。绝大多数测试（>99%）使用 `:memory:` DB + `HomeGuard`/`tempdir` 隔离，未污染真实 `~/.aidog`/`~/.claude`/`~/.codex`。历史教训（[[default-group-badge-restart-no-bug]]、[[skills-removal-cfg-test-hard-guard]]）已转化为 `#[cfg(test)]` 编译期硬拦 + HomeGuard 包裹，治理到位。

**未发现**:
- 测试写真实 `~/.aidog/`、`~/.claude/`、`~/.codex/`（所有 FS 写入经 HomeGuard 重定向到 tempdir）
- 测试读真实 `settings.json`/`groups.json`/`~/.claude/skills`（fixture 全在 tempdir 内自建）
- 测试调 `git`（仓库内零处 `Command::new("git")` in test）
- 测试用真实 `~/.aidog/aidog.db`（全部 `:memory:` 或 `tempdir` 内 `.db`）

**发现违规**: 共 6 处（高 2 / 中 4），另 4 处低严重度风格问题（HomeGuard 复制散布）。

---

## 违规清单（按严重度排序）

### 高严重度（确定调用真实外部进程）

| file:line | 违规类型 | 现状 | 修复建议 |
|---|---|---|---|
| `src-tauri/src/gateway/hooks/scripts/test_scripts.rs:46-92` | spawn python3 跑生成脚本 | `event_notify_script_passthrough_runtime` 真 `Command::new("python3").spawn()` 执行生成的 hook 脚本；依赖宿主装 python3，无则静默 skip | 删运行时测试，保留前两个纯字符串断言（`script_contains_*`） |
| `src-tauri/src/commands/test_script_executor.rs:6-10` | spawn uv 探测 | `check_uv_runs` 调 `check_uv()` → `shared.rs:157` `Command::new("uv").output()`；注释「just exercise the path」明示是为覆盖率 | 删（无实质断言）；或拆 `detect_uv` 为「构造」+「执行」两段，仅测前者 |

### 中严重度（读真实路径 / 真实出站连接 / 间接 spawn）

| file:line | 违规类型 | 现状 | 修复建议 |
|---|---|---|---|
| `src-tauri/src/gateway/skills/test_env.rs:46-57` | 间接 spawn node + npx | `check_env_does_not_panic_and_is_consistent` 调 `check_env()` → 内部 `Command::new("node").output()` + `Command::new("npx").output()`；注释「Since node/npx are present」明示依赖宿主 | 改纯 `probe_env` 单测，或 `#[ignore = "needs host node/npx"]` |
| `src-tauri/src/gateway/proxy/test_integration.rs:677-678` | 真实 TCP 连死端口 | `responses_endpoint_dead_upstream_returns_5xx` 注册上游 `http://127.0.0.1:1`，handle_proxy 真发起 connect（必 refused） | 改 `spawn_stub_upstream(599, "")` 模拟上游错误 |
| `src-tauri/src/commands/test_fs_autocomplete.rs:4-10` | 裸读 `dirs::home_dir()` | `expand_path_tilde_and_plain` 断言 `expand_path("~") == dirs::home_dir().unwrap()` | 包 `HomeGuard::new()`，断言 `expand_path("~") == h.home()` |
| `src-tauri/src/gateway/skills/test_env.rs:24-32` | 裸读 `dirs::home_dir()` | `resolve_home_env_returns_dirs_home` round-trip 断言 | 包 `HomeGuard`，断言返值为 tempdir 路径 |
| `src-tauri/src/gateway/skills/test_npx.rs:56-83, 111-121` | spawn npx 在 tempdir Project scope | 3 处 `run_npx_in_scope(Project{tempdir}, ...)` 真 spawn npx；Project scope 隔离了写入，但仍 spawn 真二进制 | 接受（隔离够）/ 改 mock / 加 `#[ignore]` |

### 低严重度（风格 / 维护性 — HomeGuard 复制散布）

| file:line | 违规类型 | 现状 | 修复建议 |
|---|---|---|---|
| `src-tauri/src/gateway/claude_integration.rs:142-166` | HomeGuard 复制 + 错误注释 | 注释「can't import from test_support (cfg(test) only)」**错误**——`test_sync_settings.rs:63` 成功 import 证伪 | 改 `use crate::gateway::db::test_support::HomeGuard;` |
| `src-tauri/src/gateway/mcp/test_domain.rs:14-50` | HomeGuard 完整复制 | 已 import `test_support::{ENV_LOCK, test_db}`，却仍复制 HomeGuard | 删复制，直接 `use ...HomeGuard` |
| `src-tauri/src/gateway/skills/test_list.rs:11-54` | EnvGuard 复制 + 独立 ENV_LOCK | 自定义 EnvGuard（增 CLAUDE_CONFIG_DIR）+ **自己的 ENV_LOCK**（与中心锁不互斥） | 反向合并 CLAUDE_CONFIG_DIR 回 test_support，删复制 |
| `src-tauri/src/gateway/test_codex.rs:8-21` | CodexHomeGuard 复制 + 独立 ENV_LOCK | 第三把锁，语义是 HomeGuard 子集 | 同上 |

---

## 已有隔离机制盘点

### 1. `HomeGuard`（核心）

- **定义**: `src-tauri/src/gateway/db/test_support.rs:11-47`
- **机制**: RAII 守卫，构造时 set `HOME` + `CODEX_HOME` → `tempfile::TempDir`，Drop 时恢复原值
- **覆盖**: 所有触 FS 写入（`~/.claude`、`~/.codex`、`~/.aidog`）的测试均应包裹
- **可见性**: `pub(crate)`，整个 crate 的 `#[cfg(test)]` 模块可 import

### 2. `ENV_LOCK`（env 串行化）

- **定义**: `src-tauri/src/gateway/db/test_support.rs:8`
- **机制**: `static Mutex<()>`，所有改 env 的测试持同一把锁，强制串行
- **覆盖缺口**: 4 把独立锁并存（见 evidence/03），跨模块不互斥——潜在竞态

### 3. `tempdir` / `:memory:` DB

- **`test_db()`**: `test_support.rs:51-55`，`:memory:` SQLite + init_tables
- **文件库**: 6 处用 `tempdir` 路径（系统 `/var/folders/.../T/`，非用户目录），语义需 WAL 行为

### 4. `#[cfg(test)]` 编译期硬拦（防 spawn 守卫）

- **范例**: `src-tauri/src/gateway/skills/npx.rs` 内 `run_npx_in_scope` / `run_npx` 对 `Global + 变更类命令` 在测试构建下 panic
- **覆盖**: 仅 skills 模块（历史踩坑后加）
- **关联记忆**: [[skills-removal-cfg-test-hard-guard]]、[[skills-fs-fallback-delete-bypass-guard]]

### 5. in-process stub server

- **模式**: `tokio::net::TcpListener::bind("127.0.0.1:0")` + `axum::serve`，测试自带 mock 上游
- **覆盖**: `test_integration.rs`、`test_http.rs`、`test_group_info.rs` 等代理/quota 集成测试

### 6. `mock_app_with_db`（Tauri AppHandle mock）

- **定义**: `src-tauri/src/commands/test_harness.rs`
- **覆盖**: 所有需 `tauri::AppHandle` 的 command 测试（test_app_log/test_platform/test_hooks/test_script_executor 等）

---

## 治理方案建议（三档分层，可独立验收）

### 档1（立修）— 高严重度违规逐个改隔离

| 目标 | 改动 | 验收 |
|---|---|---|
| `test_scripts.rs:46-92` | 删 `event_notify_script_passthrough_runtime` 测试函数 | 该文件剩 4 个纯字符串断言测试通过 |
| `test_script_executor.rs:6-10` | 删 `check_uv_runs` 测试函数（无实质断言） | 该文件剩 `set_executor_normalizes` 通过 |

**预估工作量**: 删 2 个测试函数 + 跑 `cargo test`（删后不会触外部进程，可安全跑）。30 分钟。

### 档2（集中化）— HomeGuard 收拢到单一 test_support 模块

**步骤**:

1. 扩展 `test_support::HomeGuard` 支持 `CLAUDE_CONFIG_DIR`（吸收 `EnvGuard` 语义）:
   ```rust
   pub(crate) struct HomeGuard {
       dir: tempfile::TempDir,
       _lock: std::sync::MutexGuard<'static, ()>,
       prev_home: Option<String>,
       prev_codex: Option<String>,
       prev_claude_cfg: Option<String>,   // 新增
   }
   ```
2. 删 4 处复制（`claude_integration.rs:142-166`、`mcp/test_domain.rs:14-50`、`skills/test_list.rs:11-54`、`test_codex.rs:8-21`），改 `use crate::gateway::db::test_support::{HomeGuard, ENV_LOCK};`
3. 删 `claude_integration.rs:141` 错误注释
4. 跑 `cargo test` 验所有 env-mutating 测试串行无串扰

**收益**: 4 把锁 → 1 把锁，消除跨模块 env 竞态；HomeGuard 单一定义，未来增 env 变量（如 `MCP_HOME`）只改一处。

**预估工作量**: 2 小时（含跑测试验证）。

### 档3（防再犯）— 编译期 / CI 守卫

参考 [[skills-removal-cfg-test-hard-guard]] 模式，三选一或叠加:

#### 方案 A: grep-based lint 脚本（最低成本）

在 `src-tauri/` 加 `.ci/check-test-isolation.sh`:

```bash
#!/usr/bin/env bash
# 扫描所有 test_*.rs / *_test.rs / #[cfg(test)] mod，禁止：
# 1. 裸 dirs::home_dir() 不在 HomeGuard 上下文
# 2. Command::new("python3"|"uv"|"node"|"git"|"sh").{spawn,output,status}
# 3. reqwest::{get, Client::new().*send}
set -e
cd src-tauri/src

# (1) 裸 dirs::home_dir（除 test_support.rs 自身定义外）
violations=$(grep -rn "dirs::home_dir\|dirs::config_dir" --include="test_*.rs" --include="*_test.rs" . \
    | grep -v "test_support.rs" || true)
# 配合上下文检查：同行或前 5 行须有 HomeGuard
# ...（完整 awk 略）

# (2) spawn 真实进程
spawn_violations=$(grep -rEn 'Command::new\("(python3|uv|node|git|sh)"\)' \
    --include="test_*.rs" --include="*_test.rs" . \
    | grep -E '\.(spawn|output|status)\(\)' || true)

[ -z "$violations$spawn_violations" ] || { echo "$violations$spawn_violations"; exit 1; }
```

挂到 `.github/workflows/ci.yml` 的 test job 前置。**零编译期成本，立即可加**。

#### 方案 B: 模块级 `#[cfg(test)]` 守卫（中等成本，最严格）

仿 `skills/npx.rs`，对高风险入口函数（`detect_uv`、`check_env`、`scripts.rs::ScriptInvoker::run` 等）在 `#[cfg(test)]` 下 panic:

```rust
pub(crate) fn detect_uv() -> bool {
    #[cfg(test)]
    { panic!("detect_uv() spawns real `uv` — mock in tests"); }
    #[cfg(not(test))]
    {
        std::process::Command::new("uv").arg("--version").output()...
    }
}
```

**优点**: 编译期硬拦，测试想调都调不到。
**缺点**: 生产代码侵入（`#[cfg(test)]` 分支散布），且 `cargo test` 仍能编译这些分支（只是运行时 panic）。

#### 方案 C: 构建脚本（build.rs）静态扫描（高成本，全覆盖）

`src-tauri/build.rs` 解析所有 `test_*.rs` AST，对 `Command::new(...).spawn()` / `dirs::home_dir()` 报编译错误。

**优点**: 编译期失败，最严格。
**缺点**: 需引入 `syn`/`ra_ap_syntax` 解析，构建时间增加。**仅当团队规模扩大、违规频发时才上**。

**推荐**: 现阶段上**方案 A**（grep lint）即可，成本低、覆盖足够；档1+档2 完成后违规面已极小。

---

## 统计

| 指标 | 数值 |
|---|---|
| 含 `#[cfg(test)]` 的 `.rs` 文件数 | 101 |
| 测试文件（`test_*.rs` / `*_test.rs`） | 109 |
| `#[test]` + `#[tokio::test]` 总数 | 1095 |
| **违规测试函数数** | **6**（高 2 + 中 4，不含低严重度风格项） |
| 违规率 | 0.55% |
| HomeGuard 复制散布点 | 4 |
| 已隔离（HomeGuard / tempdir / :memory: / mock_app）测试占比 | >99% |

### 按模块分布

| 模块 | 违规数 | 说明 |
|---|---|---|
| `commands/` | 2 | test_script_executor(spawn uv) + test_fs_autocomplete(裸 home_dir) |
| `gateway/hooks/scripts/` | 1 | test_scripts(spawn python3) |
| `gateway/skills/` | 3 | test_env×2(裸 home_dir + spawn node/npx) + test_npx(spawn npx Project scope) |
| `gateway/proxy/` | 1 | test_integration(死端口 TCP) |
| 其他 96 个测试文件 | 0 | 全部合规 |

---

## 待定项 / 缺信息

- `gateway/skills/test_npx.rs:56-83, 111-121`（Project scope 真 spawn npx）：判定为「中」但可接受——Project scope 已隔离到 tempdir，不触用户目录，仅依赖宿主装 npx。是否算违规取决于用户对「spawn 真实 npx 二进制」的容忍度（即使隔离了写入）。**需要: 用户确认是否要求 test 阶段完全不 spawn 任何 npx 进程（即便只读 / 即便在 tempdir）**。

## 关联记忆

- [[default-group-badge-restart-no-bug]] — 历史教训：单测污染真实 `~/.claude`，已修为 HomeGuard 隔离
- [[skills-removal-cfg-test-hard-guard]] — `#[cfg(test)]` 编译期硬拦范例（档3 方案 B 参考）
- [[skills-fs-fallback-delete-bypass-guard]] — fs 兜底物理删守卫范例
- [[proxy-egress-http-logging]] — proxy 出站 HTTP 落 log（与 test_integration stub server 设计相关）
