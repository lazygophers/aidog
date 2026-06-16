# 优化 release 发布文本

## 目标
当前 releaseBody 仅一句「自动构建发布…」，信息不足。优化为含简介/下载指引/macOS 公证提示/自动更新/文档的 markdown。

## 范围
1. `release.yml` 的 `releaseBody` 模板（版本无关，按文件后缀给平台指引）。
2. 线上 v0.1.0 release body 同步更新（`gh release edit`，outward-facing，用户已授权）。

## 素材
- 简介取自 README.md：统一管理你的 AI API 网关 · 本地桌面 · 无需上云 · 50+ 平台聚合 · 智能路由 · 用量统计
- macOS 包未 Apple 公证 → 加 `xattr -dr com.apple.quarantine` 提示
- 文档 https://lazygophers.github.io/aidog/zh/

## 验收
- release.yml releaseBody 为新 markdown，YAML 合法。
- v0.1.0 线上 body 更新成功。
