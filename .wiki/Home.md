# AiDog 开发者知识库

## 项目简介

AiDog 是一款跨平台桌面应用（macOS / Windows / Linux），作为 AI API 网关代理，统一管理多个 AI 平台、智能路由请求、追踪用量与费用。

## 核心功能

- 多平台聚合（50+ 平台预设）
- 智能路由（分组 + 模型映射 + 故障转移）
- 用量统计与费用估算
- 三级日志记录
- Codex / Claude Code 集成
- 多协议支持（OpenAI / Anthropic / Gemini）
- 主题定制（5 套内置主题）
- 系统托盘

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2.0 (Rust) |
| 前端 | React 19 + TypeScript |
| 后端 | Rust + Axum |
| 数据库 | SQLite (rusqlite) |
| 构建 | Vite + Yarn 4.x |
| i18n | i18next + react-i18next |

## 项目结构

```
src/                        # React 前端
  pages/                    # 页面组件
    Platforms.tsx           # 平台管理
    Groups.tsx              # 分组管理
    Logs.tsx                # 日志查看
    Stats.tsx               # 使用统计
    PricingTab.tsx          # 定价管理
    Settings.tsx            # 设置（编排容器）
    AppSettings.tsx         # 应用设置（tab 切换）
    ModelTestPanel.tsx      # 模型测试
    TrayConfigTab.tsx       # 托盘配置
    Proxy.tsx               # 代理状态
  components/
    settings/               # 设置页子组件
    shared/                 # 共享展示组件
  services/api.ts           # TS 类型 + Tauri invoke
  themes/                   # CSS 变量主题系统
  utils/                    # 工具函数
  locales/                  # 7 语言翻译文件

src-tauri/src/
  lib.rs                    # Tauri commands（~50 个）
  gateway/
    proxy.rs                # Axum 代理服务器
    router.rs               # 分组匹配 + 模型映射
    db.rs                   # SQLite CRUD
    models.rs               # 数据模型（Protocol 枚举 53 变体）
    quota.rs                # 余额查询
    estimate.rs             # 费用估算
    price_sync.rs           # 定价同步
    manual_budget.rs        # 手动预算
    adapter/
      converter.rs          # 协议转换入口
      openai_completions.rs
      openai_responses.rs
      gemini.rs
      codex.rs              # Codex TOML 配置
      minimax.rs
      glm.rs
```

## 知识库索引

- [架构全景](architecture/overview.md)
- [代理请求流](architecture/proxy-flow.md)
- [协议适配器](architecture/protocol-adapter.md)
- [Gateway 模块](modules/gateway.md)
- [数据库设计](modules/database.md)
- [路由算法](modules/routing.md)
- [前端页面](frontend/pages.md)
- [主题系统](frontend/theme-system.md)
- [开发环境搭建](development/setup.md)
