<div align="center">

# 🐕 AiDog

**统一管理你的 AI API 网关**

多平台聚合 · 智能路由 · 用量统计 —— 一个跨平台桌面应用，管好所有 AI 平台的 Key、请求与花费

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/zh/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/releases)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)

`简体中文` · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

> 📖 **完整文档**：<https://lazygophers.github.io/aidog/zh/>

AiDog 是一款基于 Tauri 的桌面端 AI API 网关，在本地统一管理、路由和监控 50+ AI 平台的请求。把分散在各处的 API Key、模型映射、负载均衡、用量统计收拢到一个应用，无需后台服务、无需上云，所有数据本地存储。

## ✨ 功能

- **多平台聚合** — 50+ 平台预设（Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / 通义千问 等），一键配置
- **智能分组** — 按 Bearer token / 路径匹配请求，支持 Failover（故障转移）与 Load Balance（负载均衡）路由
- **模型映射** — 透明替换模型名（如 `claude-sonnet-4` → `deepseek-chat`）
- **协议转换** — OpenAI Chat / Completions / Responses、Anthropic、Gemini 协议双向互转
- **负载均衡与故障转移** — 多平台失败自动重试，异常平台自动熔断 / 三态管理 / 指数退避
- **用量监控** — Token 统计、费用估算、各平台余额查询、Coding Plan 配额展示
- **请求日志** — 三级粒度记录（用户原始请求 / 上游请求 / 摘要），分别可配开关与保留期
- **中间件规则引擎** — 入站/出站规则：整流、覆写、脱敏、注入、敏感词过滤、错误检测
- **编程助手集成** — 原生支持 Claude Code、OpenAI Codex 等编程助手一键接入
- **国际化与主题** — 8 种语言（含阿拉伯语 RTL），Liquid Glass 等多主题 + 亮暗双模式

## 🚀 快速开始

### 下载安装

从 [GitHub Releases](https://github.com/lazygophers/aidog/releases) 下载对应平台安装包（macOS / Windows / Linux）。

详见 [安装指南](https://lazygophers.github.io/aidog/zh/getting-started/installation)。

### 三步上手

1. **添加平台** — 填入某个 AI 平台的 API Key 与端点
2. **配置代理** — 把客户端 API 地址指向本地代理：
   ```
   http://127.0.0.1:9876/proxy/v1
   ```
3. **开始使用** — 请求自动路由、计量、记录

用 curl 验证：

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

> 💡 代理地址中的 Key 可填任意值，AiDog 会使用你配置的真实 Key 转发。

📖 完整教程见 [快速开始](https://lazygophers.github.io/aidog/zh/getting-started/quick-start)。

## 🧩 技术栈

| 层 | 技术 |
| --- | --- |
| 桌面框架 | Tauri 2.0 |
| 前端 | React 19 + TypeScript + Vite |
| 后端 | Rust + Axum 代理 + SQLite 存储 |
| 构建 | Yarn + Vite |

## 🛠️ 开发

```bash
yarn                          # 安装前端依赖
yarn tauri dev                # 启动桌面应用（开发模式）
yarn build                    # 前端构建（tsc && vite build）
cd src-tauri && cargo build   # 构建 Rust 后端
cd src-tauri && cargo clippy  # Rust lint（warning 必须清）
cd src-tauri && cargo test    # Rust 测试
```

前置依赖：Node.js ≥ 18、Yarn 4.x、Rust toolchain、Tauri CLI。

## 📚 文档

完整文档站点：<https://lazygophers.github.io/aidog>

- [快速开始](https://lazygophers.github.io/aidog/zh/getting-started/quick-start)
- [平台协议](https://lazygophers.github.io/aidog/zh/platforms/protocols)
- [分组与路由](https://lazygophers.github.io/aidog/zh/groups/routing-rules)
- [Codex 集成](https://lazygophers.github.io/aidog/zh/proxy/codex-integration)
- [用量统计与定价](https://lazygophers.github.io/aidog/zh/stats/usage-stats)

## 🌍 多语言

| 语言 | README |
| --- | --- |
| 简体中文 | [README.md](README.md) |
| English | [README.en.md](README.en.md) |
| Français | [README.fr.md](README.fr.md) |
| Deutsch | [README.de.md](README.de.md) |
| Русский | [README.ru.md](README.ru.md) |
| العربية | [README.ar.md](README.ar.md) |
| Español | [README.es.md](README.es.md) |
| 日本語 | [README.ja.md](README.ja.md) |

## 推荐 IDE

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)。

## License

MIT
