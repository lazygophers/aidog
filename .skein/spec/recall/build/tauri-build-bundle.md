---
title: tauri-build-bundle
layer: recall
category: build
keywords: [tauri,build,bundle,macos,app-package,binary]
status: active
---

## yarn tauri build --no-bundle 不产 .app

## 触发场景

Tauri macOS 构建时使用 `yarn tauri build --no-bundle` 时，只产生裸二进制 `src-tauri/target/release/aidog`，并不生成 `.app` 应用包（`bundle/macos/` 目录根本不创建）。

## 陷阱 & 正解

❌ **陷阱**：假设 `--no-bundle` 仅跳过签名/通证，仍产 `.app`

```bash
yarn tauri build --no-bundle   # 只产裸二进制，无 .app
```

实际 `--no-bundle` 是跳过 Tauri 的 macOS app bundle 打包器全过程，仅产编译后的二进制文件。

✅ **正解**：要产 `.app` 必须指定 `--bundles app`

```bash
yarn tauri build --bundles app   # 产生完整 .app 应用包
```

不指定 bundles 时会走所有 platform 的默认 bundle（同等 `--bundles all`）；仅要 .app 用 `--bundles app` 精确指定。

## 反例（错误模式）

| ❌ 错 | ✅ 改为 |
|---|---|
| `yarn tauri build --no-bundle` | `yarn tauri build --bundles app` |
| 期望无 bundle 参数时有 .app | 显式指定 `--bundles app` 或走默认 all bundles |
| `--no-bundle` 用于跳过签名（误用） | 签名通过 `tauri.conf.json` 或 build 环境变量配置 |

## 案例

性能测试中需要获取原始二进制做行为测试。尝试 `yarn tauri build --no-bundle` 后发现 `bundle/macos/` 目录不存在，查找到的 `src-tauri/target/release/aidog` 仅是裸二进制（无 .app 壳）。改用 `--bundles app` 后正常产 `.app`；若仅需二进制则直接使用 `target/release/aidog`（已有）。

## 适用

- Tauri macOS 应用打包
- CI/CD 中需确保 .app 生成
- 区分二进制构建 vs app bundle 打包

## 关联

[[tauri-build-flags]] [[app-bundle-structure]] [[ci-macos-build]]
