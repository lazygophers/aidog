# Build Wiki & Rspress Docs

## 目标

构建 `.wiki/` 开发者知识库（中文）+ Rspress 多语言用户文档（7 语言）+ GitHub Pages 部署。`docs/` 置于项目根目录。

## 交付（单一交付，inline 单 task）

1. **`.wiki/`** 开发者知识库（中文）— 已于 commit e994a09 建立。
2. **`docs/`** Rspress 用户文档，7 语言（zh / en / ja / fr / de / ar / es）：
   - 章节：getting-started / platforms / groups / proxy / **logs** / stats / settings
   - logs 章节（viewing + log-settings）本轮补齐 7 语言。
3. **GitHub Pages 部署**：`.github/workflows/deploy-docs.yml`（push to main，paths docs/**）。

## 验收标准

- `docs/` 在 Node 20 下 `yarn install --immutable && yarn build` 成功，EXIT=0。
- 7 语言全章节 HTML 生成（134 页）。
- 无断链（`/logs/log-settings` 等内部链接可解析）。
- 构建产物 `doc_build/` 不入 git。
- 远端仓库 `lazygophers/aidog` 存在，CI 可部署。

## 实施记录（关键踩坑见 cortex `docs-rspress-build`）

- rspress 版本 `^2.0.14` 不存在 → 钉 `2.0.0-beta.21`。
- `defineConfig` 从 `rspress/config` 导入（非 `rspress/core`）。
- docs 独立 yarn4 项目隔离（`.yarnrc.yml` + `yarn.lock` + `packageManager`）。
- **Node 26 与 rspack SSG 不兼容静默挂起** → `docs/mise.toml` 钉 node 20。
- 补 `docs/docs/public/logo.svg`（root:'docs'，public 在 docs/docs/）。
- `.gitignore` 两处过宽：补 `doc_build/`；`logs` → 锚定 `/logs/`（曾静默吞 logs 章节内容）。
- CI install flag `--frozen-lockfile` → `--immutable`（yarn4）。

## 失败回退

build 无限挂起优先查 Node 版本（非内容问题）。
