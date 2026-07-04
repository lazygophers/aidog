# PRD — rustls CryptoProvider 未 install_default 致 MITM TLS panic

## 背景
`curl -x http://127.0.0.1:9892/proxy https://www.baidu.com`（baidu 非白名单走 blind_relay 不触发，但若白名单 host 或 MITM 候选触发 TLS）→ RST。运行时 panic：
```
thread 'tokio-rt-worker' panicked at rustls-0.23.40/src/crypto/mod.rs:249:14:
Could not automatically determine the process-level CryptoProvider from Rustls crate features.
Call CryptoProvider::install_default() before this point...
```

## 根因（panic 自报 + 代码证据闭环）
- rustls 0.23 需显式 `CryptoProvider::install_default()` 选 provider（aws-lc-rs / ring 二选一），否则首次 `ServerConfig::builder()` / `ClientConfig::builder()` panic。
- `Cargo.toml:93` `rustls = { features = ["ring", ...] }` — 用 ring。
- `gateway/mitm/tls.rs:94-98` 注释明说 builder() 用 process-default CryptoProvider，未 process 则 panic。
- **测试** 侧 `tls.rs:248` + `test_e2e_mitm.rs:124` 显式 `rustls::crypto::ring::default_provider().install_default()` → 测试全过。
- **生产** 代码（startup/app_setup）无 install_default → app 运行时首次 TLS 操作 panic。

## 目标
app 启动时调一次 `rustls::crypto::ring::default_provider().install_default()`，在任何 rustls 操作之前。

## 产出

### D1 — startup.rs install_default
`startup::run()`（startup.rs:6）函数体最开头，`tauri::Builder::default()`（L7）之前，加：
```rust
// rustls 0.23 需显式装 process-level CryptoProvider（ring），否则首次 TLS builder() panic。
// 测试侧各自 install_default，生产侧在此统一装一次（幂等，多次调用仅首次生效）。
let _ = rustls::crypto::ring::default_provider().install_default();
```
- 必须在 proxy 启动 / 任何 mitm TLS 操作之前（run() 是 app 入口，最早点）。
- `let _ =` 忽略 AlreadyInstalled（二次调用返 Err 但无害，幂等）。

## 验证
- [ ] `cd src-tauri && cargo build --release`（确认 ring feature 链接 OK）
- [ ] `cargo test gateway::mitm`（既有测试不回归；测试侧 install_default 幂等不冲突）
- [ ] `cargo test gateway::proxy::test_connect`（CONNECT 路径不回归）
- [ ] `cargo clippy` 0 warning
- [ ] 实测：用户 `yarn tauri dev` 起代理后，白名单 host 的 CONNECT 不再 panic；`curl -x ... https://api.anthropic.com`（白名单 host）TLS 握手成功（subagent 无 GUI，main 指引用户实测）

## 非目标
- ❌ 改 Cargo.toml rustls features（ring 已正确）
- ❌ 改 tls.rs builder() 调用方式（process-default 模式正确，只需启动装 provider）
- ❌ 删测试侧 install_default（测试独立进程仍需自装，保留幂等安全）
- ❌ aws-lc-rs 迁移（ring 已用且跨平台，YAGNI）

## grill 自审 trace
- 轴 A 目标 ✓ 装一次 CryptoProvider，封闭
- 轴 B 产出 ✓ startup.rs 1 行，可验收（build + test + 实测）
- 轴 C 验证 ✓ cargo build/test/clippy + 实测不 panic
- 轴 D 资源 ✓ startup.rs 单文件（可能加 use，全路径免 use）
- 轴 E 依赖 ✓ 单文件无并行冲突
- 轴 F 失败 ✓ install_default 幂等（AlreadyInstalled 返 Err 无害）；放 run() 最开头保证先于任何 TLS
- 轴 G 检查点 ✓ 根因 panic 自报，修复标准，无用户决策
