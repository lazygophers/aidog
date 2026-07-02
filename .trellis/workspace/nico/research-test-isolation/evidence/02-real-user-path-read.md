# Evidence: 裸读真实用户路径（dirs::home_dir 无 HomeGuard 包裹）

- **违规类型**: 读真实 `~`（只读，但未隔离）
- **判定**: 中严重度（仅读、无写入，但泄露用户真实路径到测试断言）

---

## 1. `commands/test_fs_autocomplete.rs:4-10` — `dirs::home_dir().unwrap()` 裸读

**文件**: `src-tauri/src/commands/test_fs_autocomplete.rs:4-10`

```rust
#[test]
fn expand_path_tilde_and_plain() {
    let home = dirs::home_dir().unwrap();   // ← 裸读真实用户 home
    assert_eq!(expand_path("~"), home);
    assert_eq!(expand_path("~/sub"), home.join("sub"));
    assert_eq!(expand_path("/abs/path"), std::path::PathBuf::from("/abs/path"));
}
```

**问题**: 测试断言 `expand_path("~")` 返回真实用户家目录。本可在 tempdir 内验证（`HomeGuard::new().home()`），但当前依赖真实 `dirs::home_dir()`。

**修复建议**: 用 `HomeGuard::new()` 包裹，断言 `expand_path("~") == h.home()`。零行为差异。

---

## 2. `gateway/skills/test_env.rs:24-32` — `resolve_home_env()` 间接读真实 HOME

**文件**: `src-tauri/src/gateway/skills/test_env.rs:24-32`

```rust
#[test]
fn resolve_home_env_returns_dirs_home() {
    let (home, _) = resolve_home_env();
    let expected = dirs::home_dir()        // ← 裸读
        .map(|h| h.to_string_lossy().into_owned())
        .or_else(|| std::env::var("HOME").ok().filter(|h| !h.is_empty()));
    assert_eq!(home, expected);
}
```

**问题**: 测试本意就是「resolve_home_env 与 dirs::home_dir 一致」，属于 round-trip 测试，读真实 HOME 是语义本身。但仍触达用户环境。

**修复建议**: 包 `HomeGuard`，断言 `resolve_home_env() == Some(tempdir_path)`（语义更严格）。

---

## 3. `gateway/skills/test_env.rs:46-57` — `check_env()` 调真实 npx/node 探测

**文件**: `src-tauri/src/gateway/skills/test_env.rs:46-57`

```rust
/// check_env exercises probe_env (OnceLock init path) and returns valid SkillsEnv.
/// Since node/npx are present in the test environment, npx_available should be true.
#[test]
fn check_env_does_not_panic_and_is_consistent() {
    let env1 = check_env();                 // ← 内部 spawn node --version + npx --version
    let env2 = check_env(); // second call returns cached value
    assert_eq!(env1.npx_available, env2.npx_available);
    if let Some(ver) = &env1.node_version {
        assert!(ver.starts_with('v'), ...);
    }
}
```

`check_env` → `gateway/skills/env.rs:80-89`:

```rust
let node_version = Command::new("node").arg("--version").output()...;
let npx_available = Command::new("npx").arg("--version").output()...;
```

**问题**: 注释「Since node/npx are present in the test environment」明示依赖宿主机装了 node/npx——属环境耦合，CI 无 node 时该测试会变软失败（断言只查 OnceLock 一致性，不查 npx_available=true，所以不会硬失败，但语义靠运气）。

**修复建议**: 改纯 `probe_env` 单测（构造 mock OnceLock）；或显式 `[ignore]` 标记「依赖宿主 node」。

---

## 统计

- 高严重度违规（spawn uv / spawn python3）: 2 处（evidence 01）
- 中严重度违规（读真实路径 + 间接 spawn node/npx）: 3 处（本文件）
- 其他 `dirs::home_dir()` 调用（`app_log.rs:28` / `sync_settings.rs:23,111,198` / `fs_autocomplete.rs:30` / `settings.rs:92` / `codex.rs:31` / `mcp/backend_claude.rs:12` / `skills/cache.rs:50` / `skills/list.rs:42,78,91,116,127` / `backup/cleanup.rs:12` / `import_export/*`）—— **均在非 `#[cfg(test)]` 生产代码**，测试通过 HomeGuard 重定向 HOME 间接覆盖，**非违规**。
