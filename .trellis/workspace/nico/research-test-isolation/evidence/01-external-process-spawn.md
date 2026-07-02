# Evidence: 外部进程 spawn (Command::new + spawn/output/status)

- **违规类型**: 调用真实外部进程（npx / python3 / uv）
- **判定**: 高严重度（用户明示禁止）

---

## 1. `gateway/hooks/scripts/test_scripts.rs` — 真 spawn python3 跑生成的脚本

**文件**: `src-tauri/src/gateway/hooks/scripts/test_scripts.rs:46-92`

```rust
#[test]
fn event_notify_script_passthrough_runtime() {
    use std::process::{Command, Stdio};
    let script = build_event_notify_script();
    let dir = std::env::temp_dir();
    let path = dir.join(format!("aidog-test-notify-{}.py", std::process::id()));
    std::fs::write(&path, &script).unwrap();

    let python = if Command::new("python3").arg("--version").output().is_ok() {  // ← 探测真实环境
        "python3"
    } else {
        let _ = std::fs::remove_file(&path);
        return; // 无 python3，跳过运行时校验。
    };
    // ...
    let mut child = Command::new(python)   // ← 真 spawn python3 解释器
        .arg(&path)
        .env_remove("ANTHROPIC_BASE_URL")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()                            // ← 实际启动子进程
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(stdin_json.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    // ...
}
```

**问题**: 真实 spawn 用户环境的 `python3` 进程，依赖宿主机安装 python3 + PATH。测试在「无 python3」时静默跳过（return），但在「有 python3」时实跑——违反「禁调用外部进程」。

**修复建议**: 改纯字符串/AST 静态断言（前两个 `script_contains_*` 测试已是该模式），删运行时 spawn 测试；或改用 `tempfile` + `mock` 解释器（成本高，不推荐）。

---

## 2. `commands/test_script_executor.rs:6-10` — `check_uv()` 真实 spawn uv 进程

**文件**: `src-tauri/src/commands/test_script_executor.rs:6-10`

```rust
#[test]
fn check_uv_runs() {
    // returns bool either way; just exercise the path
    let _ = check_uv();
}
```

被调函数（`src-tauri/src/shared.rs:157`）:

```rust
pub(crate) fn detect_uv() -> bool {
    std::process::Command::new("uv")
        .arg("--version")
        .output()                            // ← 真 spawn uv 子进程
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

**问题**: 直接调真实环境的 `uv` 二进制（用户可能装也可能没装）。注释「just exercise the path」说明作者明知是 spawn 但只为覆盖率。

**修复建议**: 删此测试（无断言、仅探覆盖率）；或拆 `detect_uv` 为「命令构造」+「output 执行」两段，仅测前者。

---

## 3. `gateway/skills/test_env.rs:35-42` — `Command::new("npx")` 仅构造不 spawn（**非违规，但需明确**）

**文件**: `src-tauri/src/gateway/skills/test_env.rs:34-42`

```rust
#[test]
fn apply_home_env_sets_home_on_command() {
    let mut cmd = Command::new("npx");        // ← 仅构造 Command 对象
    apply_home_env(&mut cmd);
    let (home, _) = resolve_home_env();
    if let Some(h) = home {
        assert_eq!(env_of(&cmd, "HOME"), Some(std::ffi::OsStr::new(&h)));
    }
}
```

**判定**: **不违规**。`Command::new` 只构造，未调 `.spawn()/.status()/.output()`，仅 `.get_envs()` 读取注入的环境变量。是合法的「builder API」单测。

`gateway/skills/test_proxy_env.rs:71-114` 同模式（3 处 `Command::new("npx")`）——**均不违规**。

---

## 4. `gateway/skills/test_npx.rs` — 编译期 cfg(test) 守卫拦截 spawn（**良好范例**）

**文件**: `src-tauri/src/gateway/skills/test_npx.rs:36-42, 87-92`

```rust
#[test]
#[should_panic(expected = "run_npx_in_scope(Global, 变更类命令) 在测试中被调用")]
fn run_npx_in_scope_global_mutating_panics_in_test() {
    let args = vec!["remove".to_string(), "--all".to_string()];
    let _ = run_npx_in_scope(&args, &SkillScope::Global, None);
}
```

**说明**: npx.rs 的 `run_npx_in_scope` / `run_npx` 在 `#[cfg(test)]` 下对 `Global + 变更类命令` 硬拦 panic（参考记忆 `skills-removal-cfg-test-hard-guard`）。这是治理档3（防再犯）的现成范例——可推广到其它模块。

注: 该文件其他测试（`run_npx_in_scope_project_*`）虽然真 spawn npx 在 tempdir 内，但 Project scope 已隔离到 tempdir，不触用户目录——处于灰色地带，倾向**中**严重度（仍 spawn 真实 npx 二进制）。
