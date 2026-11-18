<div align="center">

# AiDog

**在本机管理 AI 平台，并把客户端请求转发到你选择的平台。**

一个桌面应用，提供本地代理、分组路由、模型映射、请求记录与用量估算。

[文档站](https://lazygophers.github.io/aidog/) · [下载最新版](https://github.com/lazygophers/aidog/releases/latest) · [问题反馈](https://github.com/lazygophers/aidog/issues)

简体中文 · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

AiDog 在你的电脑上运行代理服务。你把上游平台和 API Key 保存在 AiDog，再让 Claude Code、Codex、pi 或其他兼容客户端连接本地代理。请求仍会发送到所选上游；本地运行不代表请求只留在电脑上。

## 适合什么场景

- 你使用多个 AI 服务，希望在一个应用里管理平台凭证和模型。
- 你想让不同客户端使用不同分组、模型映射或候选平台。
- 你需要查看请求状态、用量、估算费用和路由结果。
- 你希望 Claude Code、Codex 或 pi 使用同一个本地代理入口。

## 能做什么

![AiDog 首页总览](screenshots/home-light.png)

![AI 平台管理](screenshots/platforms-light.png)

*界面示意，数据为模拟。*

### 本地代理与路由

- 管理内置平台预设，也可配置自定义平台。
- 用分组隔离客户端请求，并关联各自的平台。
- 按模型映射、端点协议和可用状态选择候选平台。
- 多模型槽位可配置有序模型链；请求失败时可尝试后续候选。
- 支持 OpenAI、Anthropic、Gemini 等协议的转发与适配，实际能力取决于平台端点和请求类型。
- 支持中间件规则、时段模型配置和路由健康调度。

### 记录与统计

- 查看代理请求与独立的测试、余额查询、模型列表请求记录。
- 按设置保留或清除请求两侧的原始内容；敏感请求头会脱敏。
- 根据响应用量和内置模型价格估算费用。估算不替代上游账单。
- 查看平台余额、配额和按分组汇总的使用统计；这些数据口径不同，不能直接等同。

### 客户端与工具

- 管理 Claude Code、Codex 和 pi 配置，并为分组生成连接方式。
- 管理 MCP Server、Skills、通知和系统设置。
- 可选运行 headless 内核，并通过受本机访问限制的管理接口操作。

## 安装

从 [GitHub Releases](https://github.com/lazygophers/aidog/releases/latest) 下载与你的系统相符的安装包。各系统可用的安装包、架构和要求以该版本发布页为准。

启动 AiDog 后，本地代理默认监听 `127.0.0.1:9890`。默认仅监听本机回环地址；不要把管理接口转发到公网。

## 快速开始

1. 在“AI 平台”中添加平台，填写协议、Base URL 和上游 API Key。
2. 在平台页创建分组，并把平台加入分组。
3. 在客户端使用该分组生成的连接命令，或按客户端接入文档配置本地代理地址和分组凭据。
4. 发送一次请求，并在代理日志中确认分组、上游平台、模型和结果。

[安装指南](https://lazygophers.github.io/aidog/getting-started/installation) · [首次配置](https://lazygophers.github.io/aidog/getting-started/first-config) · [验证首个请求](https://lazygophers.github.io/aidog/getting-started/first-request)

## 数据与隐私

AiDog 将设置和数据库保存在本机。代理请求会把必要内容转发给所选上游服务。请求日志是否保留请求正文和响应正文，取决于日志设置；关闭原始内容记录后，状态、模型、用量等元数据仍可记录。详见[数据与隐私](https://lazygophers.github.io/aidog/core-concepts/data-privacy)。

## 从源码运行

前置工具与系统依赖请先按 [Tauri 官方先决条件](https://v2.tauri.app/start/prerequisites/)安装。

```bash
git clone https://github.com/lazygophers/aidog.git
cd aidog
yarn install
yarn tauri dev
```

构建安装包：

```bash
yarn tauri build
```

## 许可

AiDog 使用 [GNU AGPL-3.0-or-later](LICENSE)。
