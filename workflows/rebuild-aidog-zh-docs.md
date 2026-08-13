# Workflow: 重做 aidog 中文文档

## 目标

按 aidog 当前代码重做 `docs/docs/zh/*`，形成符合 Rspress v2 的中文文档站。

## 已确认决策

### 范围与 IA

- 中文区全量改为 `.mdx`，旧 `.md` 删除替换。
- 英文区本轮不改。
- 信息架构四区：`开始使用 / 核心概念 / 功能模块 / API`；外加独立顶层 `维护文档`。
- 任务流做一级入口，产品模块做二级落点。
- 路径命名：英文 kebab-case 目录与文件，中文标题放 frontmatter 与 `_meta.json`。
- 顶层目录：`getting-started/`、`core-concepts/`、`features/`、`api/`、`maintenance/`。
- 首页：营销 + 任务分流门户；产品定位、Releases 下载 CTA、首个请求路径、任务卡片与能力概览；不嵌入完整模块 demo。
- 旧中文 URL 全部重命名，不保留兼容。
- 旧 Rspress 模板教程改写为维护/贡献者文档。

### 页面树

#### getting-started/（6 页）

| 文件 | 中文标题 |
|------|---------|
| `introduction.mdx` | 产品介绍 |
| `installation.mdx` | 安装指南 |
| `first-config.mdx` | 首次配置 |
| `connect-claude-code.mdx` | 接入 Claude Code |
| `connect-codex.mdx` | 接入 Codex |
| `first-request.mdx` | 验证首个请求 |

#### core-concepts/（6 页）

| 文件 | 中文标题 |
|------|---------|
| `platforms-protocols.mdx` | 平台与协议 |
| `groups-routing.mdx` | 分组与路由 |
| `model-mapping.mdx` | 模型映射 |
| `failover-circuit-breaker.mdx` | 故障转移与熔断 |
| `usage-pricing.mdx` | 用量与费用 |
| `data-privacy.mdx` | 数据与隐私 |

#### features/（12 页）

| 文件 | 中文标题 |
|------|---------|
| `overview.mdx` | 首页概览 |
| `platforms.mdx` | AI 平台 |
| `groups.mdx` | 分组路由 |
| `proxy.mdx` | 代理服务 |
| `logs.mdx` | 代理日志与请求日志 |
| `stats.mdx` | 统计 |
| `notifications.mdx` | 通知中心 |
| `mcp.mdx` | MCP |
| `skills.mdx` | Skills |
| `claude-code.mdx` | Claude Code |
| `codex.mdx` | Codex |
| `settings.mdx` | 系统设置 |

#### api/（5 页）

| 文件 | 中文标题 |
|------|---------|
| `index.mdx` | API 概览 |
| `local-api.mdx` | Local API |
| `tauri-commands.mdx` | Tauri Command 使用方式 |
| `commands.generated.mdx` | 完整 Command 字典（生成） |
| `contracts.mdx` | 契约规则与错误 |

#### maintenance/（4+ 页）

| 文件 | 中文标题 |
|------|---------|
| `doc-structure.mdx` | 文档结构 |
| `mdx-components.mdx` | MDX 组件 |
| `command-gen.mdx` | Command 字典生成 |
| `contributing.mdx` | 贡献流程 |
| `internal-debug.mdx` | 内部与 Debug 能力（非公开） |
| `testing.mdx` | 测试工具与 Fixture |

### 功能模块页内组织

- 日志合并一页，代理日志与请求日志作为页内章节。
- 系统设置 tabs 作为页内章节，不拆独立页。

### 高保真 HTML 演示

- 范围：12 个功能模块页 + 首页概览。
- 保真度：视觉高保真，仅桌面端，不做移动适配。
- 禁止截图，使用 HTML/MDX 组件渲染。
- fixture 状态：正常态、空状态、错误态、加载态，全部覆盖。
- 演示交互：Tabs 切换、展开折叠、筛选、开关、复制反馈；不连接 aidog 后端。
- 动画：页面切换、面板展开、加载骨架、成功/错误反馈；遵守 `prefers-reduced-motion`。
- 示例数据：固定脱敏 fixture，允许显式命令更新版本化数据。

### 演示组件契约

- 组件目录：`docs/theme/components/`
- fixture 目录：`docs/theme/fixtures/`
- 统一 API：`<ProductDemo module="..." state="..." />`
- 组件内部管理四状态 Tabs 与交互，不暴露业务回调。
- 页面只传 module/state，不内联数据。

### command 字典生成

- 生成器位置：根 `scripts/gen-command-docs.mjs`。
- 解析方式：Node 脚本使用 TypeScript compiler API 读 TS wrappers；结构化扫描 `startup.rs` 注册表与 Rust command 签名。
- 解析失败立即报错，不静默猜测。
- 生成产物：`docs/docs/zh/api/commands.generated.mdx`，提交 Git。
- 产物组织：按 startup/领域分组，支持页面内搜索。
- command 条目字段：command 名、Rust 定义位置（仓库相对路径）、TS wrapper、参数 JSON、返回类型、错误/权限说明、最小调用示例。
- 无 TS wrapper 的 command 仍收录，标记 `internal`，不虚构 wrapper 与调用示例。
- 错误与权限字段：仅从显式 `Result` error、权限/localhost gate、源码注释与 TS 类型提取；无法证明时输出"未声明"。
- 版本追踪：生成页面记录生成 commit/version，fixture 记录来源版本。
- 契约门禁：同时核对 startup 注册名、Rust 签名和 TS wrapper；三方不一致直接失败并输出差异。

### 页面元数据

- 每页必填中文 `title`/`description`。
- 首页 `pageType: home`；模块页按需 `doc`/`doc-wide`；贡献者页 `doc`。
- 搜索：启用 Rspress 本地搜索。
- AI 输出：启用 `llms.txt`/SSG-MD。

### 代码覆盖

- 所有代码功能均需记录，包括内部与测试用途。
- 公开用户功能进入主文档。
- 内部/debug/test 能力隔离到 `maintenance/` 区，标明"非公开、可能变更、禁止生产依赖"。
- 测试内容：维护手册说明测试工具、fixture、测试 command 的目的、运行方式和源码位置；不逐测试函数生成字典。
- 源码引用：仅显示仓库相对路径，不生成 GitHub blob 链接。
- 代码示例语言：Shell + JSON + TypeScript；涉及 Rust command 时补 Rust 签名。

### 代码示例语言

- Shell + JSON + TypeScript；涉及 Rust command 时补 Rust 签名。

### 文档同步

- 所有代码变更都必须同步文档。
- UI 变化时人工检查并更新文档。

### 品牌与配置

- `rspress.config.ts` 全局改为 aidog 品牌：站点名、description、logo、icon。
- GitHub social link 指向 aidog 仓库。
- 首页提供 Releases 下载入口，不复制安装包。
- 替换 Rspress 模板 logo/icon 为 aidog 品牌资源。

### CI 与门禁

- 部署触发：维持现状，仅 `.version` 变化或手动触发。
- 质量门仅放在部署 workflow。
- 脚本位置：根 `scripts/`，由根 package scripts 暴露命令。
- 自动门禁入口：根 `yarn check:docs`，串行执行：
  1. `node scripts/gen-command-docs.mjs --check`（生成结果一致性 + 三方契约门禁）
  2. lint/format check（MDX/TS/CSS）
  3. 内部链接与资源路径检查
  4. `rspress build`
- 视觉验收：不新增浏览器依赖，保留桌面浏览器人工验收清单。

### 导航可见性

- 维护文档进入顶栏。
- 内部与测试参考仅在维护区深层侧栏，不进入首页营销入口。

### 旧文件删除清单

- 删除 `docs/docs/zh/` 下全部旧 `.md`、`.mdx`、`_nav.json`、`_meta.json`。
- 按新 IA 重建 `_nav.json` 与各目录 `_meta.json`。
- 保留英文区 `docs/docs/en/`。
- 替换 `docs/docs/public/` 下 Rspress 模板 logo/icon 为 aidog 品牌资源。

## 事实基线

- Rspress root 为 `docs`，由 `docs/rspress.config.ts` 配置（`root: path.join(__dirname, 'docs')`）。
- 当前主题仅 re-export `@rspress/core/theme-original`（`docs/theme/index.tsx:4`），HTML 展示组件尚未建立。
- 前端主入口：首页、平台、分组路由、设置、代理日志、请求日志、统计、通知中心、MCP、Skills（`src/App.tsx:13-41`）。
- `src-tauri/src/startup.rs:41` 的 `tauri::generate_handler!` 是 command 注册名唯一真值源。
- 当前注册约 209 个 Tauri commands；TS API wrapper 中约 248 个 `invoke` 调用。
- Local API 路由：`/api/group-info`、`/api/notify`、`/api/debug/bench-query`（`src-tauri/crates/aidog_core/src/gateway/proxy/mod.rs:340-345`）。
- 健康端点：`GET /` 与 `GET /proxy`（同上 :349-350）。
- 静态模型端点：`GET /models` 与 `GET /v1/models`。
- 现有 CI：`.github/workflows/deploy-docs.yml`，仅 `.version`/手动触发，只跑 `yarn build`。

## 状态

**Grilling 完成。** 所有决策已收敛。实现者可按本规格执行，无需追问。
