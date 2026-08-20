# 设计：aidog 中文文档全量重做

## 架构

Rspress root 仍为 `docs`。只重建 `docs/docs/zh/`，保留 `docs/docs/en/`。中文页面按 `getting-started`、`core-concepts`、`features`、`api`、`maintenance` 五区组织，所有页面使用 `.mdx`。

`docs/theme/components/ProductDemo.tsx` 提供统一桌面端演示组件；`docs/theme/fixtures/*.ts` 提供版本化脱敏数据。页面只声明 `module` 与 `state`，组件不调用 Tauri 或 Local API。

根 `scripts/gen-command-docs.mjs` 解析 startup 注册表、Rust command 签名与 TS invoke wrapper，输出提交到 Git 的 `docs/docs/zh/api/commands.generated.mdx`。`--check` 比较生成结果和三方契约。

## 页面数据流

源码 → command generator → `commands.generated.mdx` → Rspress build。

fixture → `ProductDemo` → MDX 页面；fixture 不读取本机配置、数据库、凭证。

Rspress config → aidog 品牌、中文导航、本地搜索、llms/SSG-MD 输出。

## 取舍

- 静态 fixture 而非真实后端：保证文档站可独立部署，避免泄露用户数据。
- 共享组件而非每页内联 HTML：统一视觉和交互，减少重复维护。
- 生成 MDX 而非运行时读 JSON：构建产物稳定，生成结果可 review。
- 相对源码路径而非 GitHub 链接：不绑定远端仓库或分支。
- 人工视觉验收而非新增 Playwright：遵守不新增浏览器依赖的最终决定；自动门禁只覆盖可重复的静态检查。

## 测试接缝

- `node scripts/gen-command-docs.mjs --check`：验证 startup/Rust/TS 三方契约和生成文件一致性。
- `yarn check:docs`：串联生成检查、MDX/TS/CSS lint、内部链接/资源检查与 Rspress build。
- `ProductDemo` fixture 可独立渲染四种状态，人工桌面浏览器逐模块检查交互、溢出、computed style、动画和 reduced-motion。
- 构建结果必须包含中英文页面；中文源码目录不得残留 `.md`。
