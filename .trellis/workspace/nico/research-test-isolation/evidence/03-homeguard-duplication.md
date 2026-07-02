# Evidence: HomeGuard / ENV_LOCK / EnvGuard 复制散布

- **违规类型**: 隔离机制代码复制（应集中到 `test_support`）
- **判定**: 低严重度（风格 / 维护性问题，不影响隔离效果）

---

## 中心定义（应作为唯一源）

**文件**: `src-tauri/src/gateway/db/test_support.rs:8-47`

```rust
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) struct HomeGuard {
    pub(crate) dir: tempfile::TempDir,
    _lock: std::sync::MutexGuard<'static, ()>,
    prev_home: Option<String>,
    prev_codex: Option<String>,
}
impl HomeGuard {
    pub(crate) fn new() -> Self { /* set HOME + CODEX_HOME → tempdir */ }
    pub(crate) fn home(&self) -> &std::path::Path { self.dir.path() }
}
impl Drop for HomeGuard { /* restore HOME + CODEX_HOME */ }
```

`pub(crate)` + `#![cfg(test)]` —— 理论上整个 crate 内的测试都能 import。

---

## 复制点 1: `gateway/mcp/test_domain.rs:14-50`（**完整复制 HomeGuard**）

```rust
struct HomeGuard {
    _dir: tempfile::TempDir,
    prev_home: Option<String>,
    prev_codex: Option<String>,
}
impl HomeGuard { fn new() -> Self { /* 同 test_support 逻辑 */ } }
impl Drop for HomeGuard { /* 同 */ }
```

**原因注释（无）**: 但 import 了 `crate::gateway::db::test_support::ENV_LOCK`（line 12）和 `test_db`（line 7）—— 说明 **能 import test_support**，却仍复制了 HomeGuard。**这是纯粹的复制粘贴债务，应直接 import**。

---

## 复制点 2: `gateway/skills/test_list.rs:11-54`（**EnvGuard，改名 + 加 CLAUDE_CONFIG_DIR**）

```rust
static ENV_LOCK: Mutex<()> = Mutex::new(());          // ← 自己一份锁（与 db::test_support 的不是同一把！）
struct EnvGuard {
    prev_home: Option<String>,
    prev_claude_cfg: Option<String>,                  // ← 比 HomeGuard 多一个 CLAUDE_CONFIG_DIR
    prev_codex_home: Option<String>,
}
```

**问题**:
1. 自己定义了一个**不同的** `ENV_LOCK`——与 `db::test_support::ENV_LOCK` 是两把不同的锁，无法跨模块串行化 env 改动。两个 skills 测试与一个 mcp 测试若并行改 HOME，仍可能串扰（虽然各自有锁保护组内，但跨组无互斥）。
2. 语义扩展（CLAUDE_CONFIG_DIR）有价值，应**反向合并回 test_support**。

---

## 复制点 3: `gateway/claude_integration.rs:142-166`（**HomeGuard 复制 + 错误注释**）

```rust
// ── HomeGuard: redirect HOME to tempdir, protected by ENV_LOCK ──
// We can't import from gateway::db::test_support (cfg(test) only), so we replicate it.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
struct HomeGuard { /* 仅 HOME，无 CODEX_HOME */ }
```

**注释错误**: `test_support.rs` 顶部是 `#![cfg(test)]`，整个模块仅在测试构建存在——但它本身就是 `#[cfg(test)] mod tests` 内的代码，**完全可以 import**（`commands/test_sync_settings.rs:63` 就成功 import 了 `crate::gateway::db::test_support::HomeGuard`，证伪注释）。**注释是历史误判，应删复制改 import**。

同样问题: 自己的 `ENV_LOCK` 与中心不是同一把锁。

---

## 复制点 4: `gateway/test_codex.rs:8-21`（**CodexHomeGuard，仅 CODEX_HOME**）

```rust
static ENV_LOCK: Mutex<()> = Mutex::new(());          // ← 第三把锁
struct CodexHomeGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    _dir: tempfile::TempDir,
}
fn set_codex_home() -> CodexHomeGuard { /* set CODEX_HOME → tempdir */ }
```

**问题**: 第四把独立锁。语义是 HomeGuard 的子集（只管 CODEX_HOME）。

---

## 锁不互通的后果

| 锁定义 | 保护范围 | 与中心锁互斥? |
|---|---|---|
| `db::test_support::ENV_LOCK` | mcp/test_domain(部分)、commands/test_*、import_export/test_collect、skills/test_cache | — |
| `skills::test_list::ENV_LOCK` | skills/test_list | **否** |
| `claude_integration::tests::ENV_LOCK` | claude_integration tests | **否** |
| `test_codex::ENV_LOCK` | gateway/test_codex | **否** |

cargo 默认多线程跑测试时，4 把锁保护的测试组之间可并行改 `HOME`/`CODEX_HOME`——若两组测试同时跑且都改 env，**会串扰**（一组 Drop 恢复的 HOME 被另一组的 set 覆盖）。目前未爆可能因测试数少 / 时序幸运，但属于潜在竞态。

---

## 修复建议（档2）

1. `db::test_support::HomeGuard` 扩展支持 `CLAUDE_CONFIG_DIR`（吸收 test_list 的 EnvGuard 语义）。
2. 4 处复制全部删，改 `use crate::gateway::db::test_support::{HomeGuard, ENV_LOCK};`。
3. 删 `claude_integration.rs:141` 的错误注释。
4. 统一后所有 env-mutating 测试串行在同一把 `ENV_LOCK` 上。
