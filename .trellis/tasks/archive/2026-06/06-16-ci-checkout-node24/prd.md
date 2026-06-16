# 升级 CI actions/checkout 到 Node 24 兼容版

## 背景
GitHub Actions 2026-06-16 起强制 JS actions 跑 Node 24，Node 20 runner 2026-09-16 移除。
当前 `.github/workflows/release.yml`（2 处）+ `deploy-docs.yml`（1 处）用 `actions/checkout@v4`（Node 20）。
告警：read-version job 输出 Node 20 deprecation warning。

## 目标
把 3 处 `actions/checkout@v4` 升到 Node 24 兼容版，消除 deprecation warning。

## 决策
- 最新 `actions/checkout` = v6.0.3；v5 是 Node 24 迁移稳定基线。
- 选 **v5**（Node 24、广泛验证、官方维护），非 v6（刚发布，无额外收益）。
- v4→v5 无 breaking change 影响本仓用法（裸 `uses: actions/checkout@v4`，无特殊 inputs）。

## 改动范围
- `.github/workflows/release.yml`：2 处 `@v4` → `@v5`
- `.github/workflows/deploy-docs.yml`：1 处 `@v4` → `@v5`

## 验证
- grep 确认全仓无残留 `checkout@v4`
- workflow yaml 语法无破坏（纯版本号替换）
- （可选）push 后观察 CI run 无 Node 20 warning

## 非目标
- 不动其他 actions（setup-node / upload-artifact 等，除非也告警）
- 不改 workflow 逻辑
