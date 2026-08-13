# NOTES

## 项目与术语

- 项目：aidog，Tauri 2 + React 19 + TypeScript + Rust 桌面端 AI API 网关。
- 文档框架：Rspress v2。
- 中文文档根：`docs/docs/zh/`。
- 产品核心术语：AI 平台、分组路由、代理服务、代理日志、请求日志、统计、通知中心、MCP、Skills、Claude Code、Codex。
- Tauri command 注册名真值源：`src-tauri/src/startup.rs` 中 `tauri::generate_handler!`。
- TS invoke wrapper：`src/services/api/*.ts`。
- 文档中"HTML 渲染现状"：使用共享 MDX/React 组件复刻当前界面，不使用截图，不读取真实用户数据。

## 已确认偏好

- 重做 `zh/*` 全部内容与信息架构。
- 全部内容文件使用 `.mdx`。
- 用户文档主线 + 独立维护文档支线。
- 首页偏营销，但提供任务分流。
- 核心模块做视觉高保真 HTML 示意，仅桌面端。
- 完整 Tauri command 字典从源码生成。
- 示例使用固定脱敏 fixture，可通过显式命令更新。
- 英文文档本轮不改。
- 旧中文 URL 全部重命名，不保留兼容。
- command 文档手动生成、CI 检查；生成器同时核对 startup/Rust/TS 三方契约。
- HTML fixture 覆盖正常、空、错误、加载四种状态。
- 文档演示仅桌面端，提供静态站可运行的交互与动画，不连接 Tauri/backend，遵守 reduced-motion。
- 所有代码变更都要求同步文档，包括内部重构。
- 中文站启用本地搜索与 `llms.txt`/SSG-MD。
- 最终门禁：构建、链接、代码质量为自动；视觉为人工。
- 功能模块固定 12 页；日志合并，设置 tabs 页内组织。
- 维护文档独立顶层区。
- 用户要求所有代码功能纳入文档，包括内部与测试用途；内部能力隔离标注。
- 源码引用仅显示仓库相对路径。
- 不新增 Playwright，视觉验收走人工桌面浏览器清单。
- command 生成器与检查脚本放根 `scripts/`。
- `yarn check:docs` 串联所有自动门禁。
