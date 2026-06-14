# PRD: dependabot-3-esbuild

## 背景
GitHub Dependabot alert #3: https://github.com/lazygophers/aidog/security/dependabot/3

- 依赖: `esbuild` (npm, transitive, development scope)
- 当前版本: `0.27.7` (经 vite 7 间接引入)
- 漏洞: GHSA-gv7w-rqvm-qjhr — esbuild Deno 模块缺二进制完整性校验，NPM_CONFIG_REGISTRY 可控时 RCE
- 漏洞范围: `>= 0.17.0, < 0.28.1`
- 修复版本: `0.28.1`
- 发布时间: 2026-06-12

## 目标
消除 dependabot 警报 #3，将 esbuild 提升至非漏洞版本 `0.28.1`。

## 产出
1. `package.json` 新增 `resolutions.esbuild = "0.28.1"` 强制提升（transitive 依赖无法直接改 vite 依赖）
2. `yarn.lock` 更新（`yarn install` 重生成）

## 验证
- `yarn install` 成功，无错误
- `grep -E "^esbuild@|^\"@esbuild/" yarn.lock` 全部解析到 `0.28.1`
- `yarn build` 成功（tsc + vite build 走 esbuild）
- 重新拉 dependabot alert（或本地 `gh api` 确认状态可被关闭/已 dismissed）

## 非目标
- 不升级 vite（vite 7 已声明 esbuild 范围兼容 0.27/0.28，resolutions 足矣）
- 不动 Rust / Cargo 依赖
- 不改业务代码

## 风险
- esbuild 0.28 breaking change：`format`/`banner`/`footer` 等 API 在 0.18+ 已 deprecation。但 vite 7 内部封装，resolutions 提升一般安全。`yarn build` 验证兜底。
