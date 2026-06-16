# 优化 release CI 减少 GitHub Actions 计费分钟

## 目标
降低每次 release 工作流的 billed minutes（GitHub 按所有 job 时长之和计费，macOS 10×/Windows 2×），以在配额内换取更多执行次数。**保 macOS universal 双架构产物形态不变**。

## 范围（保 universal，路线 B+C+D）
1. **Cargo release profile 调优** — `src-tauri/Cargo.toml` 加 `[profile.release]`：`codegen-units=256` / `lto=false` / `opt-level=2` / `strip=true`，换更快编译。
2. **setup-node 加 yarn 缓存** — `actions/setup-node@v4` 设 `cache: yarn`。
3. **内联 read-version** — 删掉独立 read-version job，版本号在 release job 内一个 step 取，砍一次 runner spin-up。
4. **确认 rust-cache 命中** — 维持 `swatinem/rust-cache@v2`，确保 universal 双 target 缓存生效。

## 非目标
- 不拆并行 job（拆 arm64/x64 不省 billed minutes，反增 spin-up）。
- 不砍 macOS 架构（保 Intel 兼容）。

## 验收
- `release.yml` 无独立 read-version job，矩阵仍出 macOS universal + Windows x64。
- `Cargo.toml` 有 `[profile.release]` 块。
- `cargo build --release` 本地仍通过（profile 合法）。
- YAML 语法合法（actionlint 或目视）。
