# release CI 失败也缓存

## 目标
`release.yml` 中无论 build 成功或失败都保存 cache（Rust + yarn），让失败 run 的编译产物也能被下次复用，省额度。

## 现状
- `swatinem/rust-cache@v2`：默认 `cache-on-failure: false` → job 失败不存 Rust 缓存。
- yarn `actions/cache@v4`：save 在 post 步，job 失败时 GitHub 跳过 → 不存。

## 范围
1. rust-cache 加 `cache-on-failure: true`。
2. yarn cache 拆 `actions/cache/restore@v4`（开头）+ `actions/cache/save@v4`（结尾 `if: always()`），失败也存。

## 验收
- `release.yml` rust-cache 有 `cache-on-failure: true`。
- yarn 用 restore/save 分离，save 带 `if: always()`。
- YAML 语法合法。
