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

## Flutter↔Tauri UI 对齐（workflows/flutter-ui-parity.md）

- 「完全一致」= 元素级+行为级，React 现状为唯一真值；不做像素比对。
- 触发 = 用户报差异；验收 = 机器（analyze + widget 测试 + check-ui-parity + 清单更新）。
- 2026-09-23 推翻旧裁决：4 条有意偏离全部抹平（组内拖拽、导入确认卡、拖入判扩展名、关窗——末条两侧已一致无事可做）。

## Registry 数据对齐（workflows/registry-data-alignment.md）

- 「无缺失」= 分层：family/version/capabilities/context_window 必补（官方有公布即清零），thinking_*/predecessor/display_name 尽力。
- 写入授权（2026-09-23）：agent 查证官方来源后自动写入 registry，事后审计；推翻「禁机器生成覆盖」，但「禁推断、缺省=未知、文本级编辑、数值保真」铁律不变。
- 门禁：四必补字段覆盖率 ≥90% 硬阈值（单调收紧）。
- 批次：按平台分批，每轮 1-2 平台。
