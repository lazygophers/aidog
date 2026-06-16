# CI actions 升级到 Node 24 兼容版

## 背景
GitHub 弃用 Node 20 action（2026-06-16 起强制 Node 24，2026-09-16 移除 Node 20）。当前 workflow 多个 action 仍 node20。

## 已核实 runtime（curl action.yml）
node20 需升：setup-node@v4 / cache/restore@v4 / cache/save@v4 / deploy-pages@v4
已 node24（不动）：checkout@v5 / swatinem/rust-cache@v2 / tauri-action@v0 / rust-toolchain(composite)
composite 同步升：upload-pages-artifact@v3→v5

## 范围
- `release.yml`: setup-node@v4→v6, cache/restore@v4→v5, cache/save@v4→v5
- `deploy-docs.yml`: setup-node@v4→v6, upload-pages-artifact@v3→v5, deploy-pages@v4→v5

## 验收
- 两文件无 @v4 的 setup-node / cache；无 node20 action。
- YAML 合法。
