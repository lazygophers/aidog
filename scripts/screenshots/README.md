# 真渲染截图脚本

对 vite dev 前端注入假 Tauri 数据后，用 Playwright 对全部（或指定）页面截深浅两套图。

## 前置

- 依赖已安装（playwright-core；浏览器用系统 Chrome，Playwright 启动参数带 `channel: "chrome"`，无需另装 headless shell）。
- 先起 dev server：`yarn dev`（vite，端口 1420），脚本自身不起 server。

## 用法

```sh
node scripts/screenshots/shoot.mjs            # 全部页面
node scripts/screenshots/shoot.mjs home logs  # 指定页面
```

- 页面 id 清单与假数据见 `shoot.mjs` 的 `PAGES` 与 `mocks.mjs`。
- 假数据经 `page.addInitScript` 在页面加载前替换 `window.__TAURI_INTERNALS__.invoke`，未匹配的 invoke 名落 `.tmp/screenshots-redo/unmatched.log`。
- 截图输出 `screenshots/`（仓库根，产物入库，docs 引用它们）；未匹配日志在 `.tmp/screenshots-redo/unmatched.log`。
