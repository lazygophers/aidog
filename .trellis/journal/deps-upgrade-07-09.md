# Deps Upgrade 2026-07-09 (task 07-09-deps-upgrade-stable)

PRD ADR: A+B+C 档执行，D 档（rusqlite/rand/tokio-rusqlite）禁动留版本。

## A 档（semver 内安全升）

- `cargo update` 守 Cargo.toml 上界，主要 minor/patch：
  - tauri-utils 2.9.2 → 2.9.3
  - tauri-winrt-notification 0.7.2 → 0.7.3
  - time 0.3.47 → 0.3.53
  - tray-icon 0.23.1 → 0.24.1
  - uuid 1.23.2 → 1.23.4
  - wasm-bindgen 0.2.123 → 0.2.126
  - web-sys 0.3.100 → 0.3.103
  - webpki-root-certs 1.0.7 → 1.0.8
  - zbus 5.16.0 → 5.17.0
  - zerocopy 0.8.50 → 0.8.54
  - zeroize 1.8.2 → 1.9.0
  - +15 unchanged
- `yarn up` 守 package.json ^/~：yarn.lock 无变化（依赖已在最新范围内）
- 修复 edition 2024 set_var/remove_var unsafe 编译错误：
  cargo update 后 rustc 1.96 严格检查 edition 2024 unsafe 要求
  baseline 本来同坏（4 errors 在 http_client.rs test），A 档只是触发未缓存的真编译
  修复：wrap unsafe 块，前后兼容

## B 档（minor 跨）

- Cargo.toml 上界用主版本号（"2"/"1"/"0.8"/"0.12" 等），cargo update 自动拉最新 minor，无需放宽 Cargo.toml
- tauri 2.11.2 → 2.11.5（latest 2.x）
- 间接依赖 minor/patch：
  - alloc-stdlib 0.2.2 → 0.2.4
  - anyhow 1.0.102 → 1.0.103
  - brotli 8.0.3 → 8.0.4
  - brotli-decompressor 5.0.1 → 5.0.3
  - bytes 1.11.1 → 1.12.1
  - camino 1.2.2 → 1.2.4
  - cc 1.2.63 → 1.2.66
  - crossbeam-channel 0.5.15 → 0.5.16
  - crossbeam-utils 0.8.21 → 0.8.22
  - syn 2.0.117 → 2.0.118

## C 档（跨低风险 major）

- **dirs 5.0.1 → 6.0.0**：API 兼容（仅 `dirs::home_dir()` 调用点，~30 处），无源码迁移，cargo build 直接绿
- **aes-gcm 0.10.3 → 0.11.0**：API 破坏
  - `Aes256Gcm::new(key)` 签名改 `new(&key)`（&Key 引用而非 owned）
  - `Key::<Aes256Gcm>::from_slice` 弃用 → 改 `try_from(&key[..])`
  - `Nonce::from_slice` 弃用 → 改 `try_into()`，用 `aead::Nonce<Aes256Gcm>` 类型别名
  - 影响文件：`src/gateway/import_export/container.rs`（encrypt/decrypt 各一处）
- **sha2 0.10.9 → 0.11.0**：被 hmac 0.13 强制要求（EagerHash trait bound），API 兼容（Digest/Sha256::new/update/finalize 不变）
- **hmac 0.12.1 → 0.13.0**：API 破坏
  - `Mac::new_from_slice` 移除 → 用 `KeyInit::new_from_slice`
  - 与 aes-gcm::aead::KeyInit 命名冲突 → import 别名 `use hmac::{KeyInit as HmacKeyInit, ...}`
  - 影响文件：`src/gateway/import_export/container.rs`（encrypt/decrypt 各一处）
- **tts 0.26.3**：已是 latest，无升级动作
- **serde_yml 0.0.13**：crate 已 DEPRECATED（unmaintained shim），0.0.13 即最终版本，无可升级目标
  - 现状调用点 8 处（`serde_yml::to_string` / `serde_yml::from_str`，share 串 + test_platform）
  - 备选迁移目标：`serde_yaml`（更陈旧亦 deprecated）；当前 0.0.13 仍能编译，留版本 + 监控

## D 档禁动（留版本清单）

PRD ADR：跨 major 高危，DB 层 + 加密随机层核心，回归风险高于收益，本次跳过。

- **rusqlite 0.32.1 → latest 0.40.1**：跨 8 个 major（0.32→0.40）
  - 影响：src-tauri/src/gateway/db.rs + 19 个 tests_* 子模块（全 SQL 层）
  - 风险：Connection/Statement 签名变化、Features gate 改名、Error 类型重构
  - 建议：另开专项迁移任务，需逐 API 迁移 + DB 层全量回归
- **rand 0.8.6 → latest 0.10.2**：跨 2 个 major
  - 影响：src-tauri/src/gateway/import_export/container.rs（fill_bytes）+ 其他随机数调用点
  - 风险：trait 路径大变（`thread_rng()` → `rng()`，`Rng::gen_range` → `Random::random_range`）
  - 建议：另开专项任务
- **tokio-rusqlite 0.6.0 → latest 0.7.0**：跨 1 major
  - 影响：src-tauri/src/gateway/db.rs（call/call_traced 异步 SQL 接口）
  - 风险：与 rusqlite 0.40 强耦合，需与 rusqlite 升级同步
  - 建议：并入 rusqlite 专项迁移任务

## E 档禁动（框架跨 major）

PRD R0.2：框架 major 锁，禁动。

- Tauri 2 → 3：尚未发布 stable
- React 19 → 20：未发布
- Vite 7 → 8：major 跨，dev 工具链，本次不动
- TypeScript 5 → 7：5.9.3 latest 5.x，PRD 原期望 5.8→5.9 minor 已满足；6/7 跨 major 不动
- @vitejs/plugin-react 4 → 6：随 vite 8 一起跨，不动

## 验证结果

- A 档：cargo build --tests ✓ / cargo test --lib 1348 passed / yarn build ✓
- B 档：cargo build --tests ✓ / cargo test --lib 1348 passed ✓
- C 档：cargo build --tests ✓ / cargo test --lib 1348 passed / cargo clippy 0 errors, 124 warnings（全为 pre-existing style）/ yarn build ✓

## baseline 文件

- `.trellis/journal/deps-baseline-cargo.txt`：升级前 cargo tree --depth 1 快照
- `.trellis/journal/deps-baseline-yarn.txt`：升级前 yarn list（yarn 4 移除 list 子命令，仅记录工具限制）
