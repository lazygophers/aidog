<div align="center">

# 🐕 AiDog

**统一管理你的 AI API 网关**

本地桌面应用 · 无需上云 · 50+ 平台一键聚合 · 智能路由 · 用量统计

[![Documentation](https://img.shields.io/badge/docs-lazygophers.github.io/aidog-0a66c2?logo=sphinx&logoColor=white)](https://lazygophers.github.io/aidog/zh/)
[![GitHub Release](https://img.shields.io/github/v/release/lazygophers/aidog?logo=github&label=release)](https://github.com/lazygophers/aidog/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-blue)](#license)
[![Platforms](https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey?logo=tauri&logoColor=white)](https://github.com/lazygophers/aidog/releases/latest)
[![认可LINUX DO](https://ld.xh.do/ld-badge.svg)](https://linux.do)
[![GitHub Stars](https://img.shields.io/github/stars/lazygophers/aidog?style=social)](https://github.com/lazygophers/aidog/stargazers)
[![Downloads](https://img.shields.io/github/downloads/lazygophers/aidog/total?logo=github)](https://github.com/lazygophers/aidog/releases)
[![Last Commit](https://img.shields.io/github/last-commit/lazygophers/aidog?logo=git&logoColor=white)](https://github.com/lazygophers/aidog/commits)
[![Issues](https://img.shields.io/github/issues/lazygophers/aidog?logo=github)](https://github.com/lazygophers/aidog/issues)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen?logo=github)](https://github.com/lazygophers/aidog/pulls)

`简体中文` · [English](README.en.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Русский](README.ru.md) · [العربية](README.ar.md) · [Español](README.es.md) · [日本語](README.ja.md)

</div>

---

> 📖 **完整文档**：<https://lazygophers.github.io/aidog/zh/>

AiDog 是一款基于 Tauri 的**桌面端 AI API 网关**。它在你的电脑本地统一管理、路由和监控 50+ AI 平台的请求 —— 把分散在各处的 API Key、模型映射、负载均衡、用量统计、编程助手配置收拢到一个应用。无需后台服务、无需上云，所有数据本地 SQLite 存储。

![AiDog — 主面板](screenshots/dashboard.png)

## 它解决什么问题

| 你的痛点 | AiDog 怎么做 |
| --- | --- |
| API Key 散落在十几个平台，切换麻烦 | **多平台聚合** — 50+ 预设平台，一处管理所有 Key |
| 单平台挂了整个流程停摆 | **故障转移 + 负载均衡** — 多平台自动重试、熔断、调度 |
| Claude Code / Codex / 各客户端配置各自为政 | **编程助手原生集成** — 一键导出配置，统一走代理 |
| 不知道每月花了多少、哪个平台快用完 | **用量监控** — Token + 费用估算 + 余额 + Coding Plan 配额 |
| 数据不想上云、不想给第三方 | **纯本地** — 代理 + 数据库都在你机器上，零外传 |

## 核心功能

### 🌐 网关与路由
- **多平台聚合** — 50+ 平台预设（Anthropic / OpenAI / DeepSeek / GLM / Kimi / MiniMax / 通义千问 / SiliconFlow / OpenRouter 等），一键配置
- **智能分组** — 按 Bearer token / 路径匹配请求，支持 Failover（故障转移）与 Load Balance（负载均衡）
- **模型映射** — 透明替换模型名（如 `claude-sonnet-4` → `deepseek-chat`）
- **协议转换** — OpenAI Chat / Completions / Responses、Anthropic、Gemini 协议双向互转
- **熔断与调度** — 异常平台自动熔断、三态管理、指数退避、组内智能调度
- **中间件规则引擎** — 入站/出站规则：整流、覆写、脱敏、注入、敏感词过滤、错误检测，内置预设

### 📊 监控与统计
- **用量监控** — Token 统计、费用估算（自动价格同步 + 手动预算）
- **余额查询** — 各平台余额实时拉取
- **Coding Plan 配额** — DeepSeek / Kimi / GLM 等 Coding Plan 配额展示与倒计时
- **请求日志** — 三级粒度（用户原始请求 / 上游请求 / 摘要），分别可配开关与保留期

### 🤖 编程助手集成
- **Claude Code** — 原生集成：配置编辑、一键导入/导出、StatusLine 脚本、Hooks、分组配置同步
- **OpenAI Codex** — 原生集成：`~/.codex/config.toml` 编辑器、Responses API 自动路由
- **MCP 管理** — DB 集中存 + per-agent 启用切换 + 扫描导入 + 敏感脱敏
- **Skills 管理** — 基于 npx 的全平台统一 skills 列表 + per-item 启用切换
- **系统通知** — TTS 播报 / 弹窗 / 收件箱 + Claude Code/Codex hook 一键注入

### 🎨 个性化
- **主题系统** — 3 轴：9 种 style（Liquid Glass / Flat / Soft / Sharp / Aurora / Paper / Terminal / Bento / Sketchy）× 12 种命名调色板（Apple Blue / Nord / Dracula / Catppuccin / Gruvbox / Tokyo Night / One Dark / Material / GitHub / Night Owl 等）× 亮暗双模式
- **国际化** — 8 种语言（含阿拉伯语 RTL）
- **导入导出** — AES-256-GCM 加密单文件容器 `.aidogx`，7 种范围逐项冲突决策
- **托盘 + 状态栏** — 系统托盘快速操作 + 可定制状态栏脚本（Python + uv）

## 安装

### 系统要求

| 系统 | 最低版本 | 备注 |
| --- | --- | --- |
| macOS | 12.0 (Monterey) | Intel + Apple Silicon |
| Windows | Windows 10 | x64 |
| Linux | x86_64 / aarch64 | 需 WebKit2GTK |

**下载入口** 👉 <https://github.com/lazygophers/aidog/releases/latest>

### macOS

1. 从 [Releases Latest](https://github.com/lazygophers/aidog/releases/latest) 下载 `.dmg` 文件
2. 双击打开，将 **AiDog** 拖入 `Applications` 文件夹
3. 首次启动时，**右键点击**应用 → 选择「打开」（绕过 Gatekeeper，因为应用未签名）

> ⚠️ 首次启动若提示"无法验证开发者"，前往 `系统设置 → 隐私与安全性 → 仍要打开`。

### Windows

1. 从 [Releases Latest](https://github.com/lazygophers/aidog/releases/latest) 下载 `.msi` 安装包
2. 双击运行安装程序，按提示完成
3. 若 SmartScreen 拦截，点击「更多信息 → 仍要运行」

### Linux

```bash
# DEB 包
sudo dpkg -i aidog_0.1.0_amd64.deb

# 或 AppImage
chmod +x aidog_0.1.0_amd64.AppImage
./aidog_0.1.0_amd64.AppImage
```

> Linux 需先安装 WebKit2GTK 依赖：`sudo apt install libwebkit2gtk-4.1-dev`（Debian/Ubuntu）。

### 首次启动

安装后启动 AiDog，应用会自动：

1. 在本地启动代理服务器（默认 `http://127.0.0.1:9876`）
2. 创建本地 SQLite 数据库（`~/.aidog/aidog.db`）
3. 显示主界面，引导你添加第一个平台

## 快速上手（3 步）

### 第 1 步：添加平台

![AiDog — 添加平台](screenshots/add-platform.png)

1. 左侧导航点 **「平台」**
2. 点 **「+ 添加平台」**
3. 填写：**名称**（如 `My OpenAI`）、**Base URL**（如 `https://api.openai.com/v1`，已含 `/v1` 版本前缀）、**API Key**
4. 保存

> 💡 Base URL 已含版本前缀，AiDog 自动追加 `/chat/completions`，无需手动拼路径。

### 第 2 步：把客户端指向代理

在要使用 AI API 的应用中，把 API 地址改为 AiDog 代理地址：

```
http://127.0.0.1:9876/proxy/v1
```

API Key 可填**任意值** —— AiDog 会用你配置的真实 Key 转发。

### 第 3 步：验证

```bash
curl http://127.0.0.1:9876/proxy/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer any-value" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello!"}]}'
```

返回正常 AI 响应 = 配置成功。请求会自动路由、计量、记录。

## 客户端接入详解

### Claude Code

AiDog 在 **「设置 → Claude Code」** 标签页提供完整集成（编辑模型/权限/沙箱/插件/Hooks/StatusLine、一键导入/导出）。

**方式一：环境变量（最快）**

```bash
export ANTHROPIC_BASE_URL="http://127.0.0.1:9876"
export ANTHROPIC_API_KEY="any-value"
claude
```

**方式二：一键导出配置**

在「设置 → Claude Code」点「导出到 Claude Code」，AiDog 会写入 `~/.claude.json`：

```json
{ "apiBaseUrl": "http://127.0.0.1:9876" }
```

**分组隔离** — 点「同步分组设置」为每个分组生成独立配置（`~/.aidog/settings.<分组名>.json`），分组卡片「Claude」按钮复制启动命令。

### OpenAI Codex

编辑 `~/.codex/config.toml`（或在「设置 → Codex」标签页内编辑）：

```toml
[provider]
name = "openai"
base_url = "http://127.0.0.1:9876/proxy/v1"
api_key = "any-value"

[model]
name = "o3"
```

> Codex 使用 Responses API（`/v1/responses`），AiDog 自动检测并路由。

### 任意 OpenAI / Anthropic 兼容客户端

把客户端的 `base_url` / `OPENAI_API_BASE` / `ANTHROPIC_BASE_URL` 指向 `http://127.0.0.1:9876/proxy/v1`，Key 填任意值即可。

> 🔐 **分组认证** — 代理地址中 Key 填**分组名**（Group Name），AiDog 按 Bearer token 路由到对应分组：`Authorization: Bearer <group_name>`。

![AiDog — 设置页](screenshots/settings.png)

## 从源码构建

```bash
# 克隆
git clone https://github.com/lazygophers/aidog.git
cd aidog

# 装依赖
yarn install

# 开发模式
yarn tauri dev

# 构建生产版本
yarn tauri build
```

**前置依赖** — Node.js ≥ 18、Yarn 4.x、Rust toolchain（rustup）、Tauri CLI、各平台系统依赖（见 [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)）。

## 发布与版本管理

**版本唯一可信源 = 根目录 `.version`**（单行 semver，如 `0.1.0`）。所有 manifest（`package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` / `docs/package.json`）由脚本从 `.version` 同步：

```bash
node scripts/sync-version.mjs          # 写入各 manifest（= yarn version:sync）
node scripts/sync-version.mjs --check  # 校验一致性，CI 用（= yarn version:check）
```

**发版流程**：改 `.version` → `yarn version:sync` → 提交推送 master。`.version` 变更触发两条 CI：

- `.github/workflows/release.yml` — macOS(arm64+x64) + Windows(x64) 多平台构建 + minisign 签名 + 发布 GitHub Release（tag `v<version>`）+ 生成 updater `latest.json`。
- `.github/workflows/deploy-docs.yml` — 重新部署文档站点。

**自动更新**：客户端「关于」页「检查更新」→ 命中 `releases/latest/download/latest.json` → 下载安装并重启。

**首次配置（仓库维护者一次性）** — 生成 updater 签名密钥并配 GitHub Secrets：

```bash
yarn tauri signer generate -w ~/.tauri/aidog_updater.key
# 公钥 → src-tauri/tauri.conf.json 的 plugins.updater.pubkey（已内置）
# 私钥（~/.tauri/aidog_updater.key 内容）→ GitHub Secret: TAURI_SIGNING_PRIVATE_KEY
# 密钥密码（如有）→ GitHub Secret: TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

> ⚠️ 私钥**绝不入库**（`.gitignore` 已忽略 `*.key`）。pubkey 公开安全。

## 技术栈

| 层 | 技术 |
| --- | --- |
| 桌面框架 | Tauri 2.0 |
| 前端 | React 19 + TypeScript + Vite |
| 后端 | Rust + Axum 代理 + SQLite 存储 |
| 文档 | Rspress（8 语言站点） |
| 构建 | Yarn 4 + Vite + cargo |

## 文档

完整文档站点 👉 <https://lazygophers.github.io/aidog/zh/>

| 主题 | 链接 |
| --- | --- |
| 快速开始 | [/getting-started/quick-start](https://lazygophers.github.io/aidog/zh/getting-started/quick-start) |
| 安装指南 | [/getting-started/installation](https://lazygophers.github.io/aidog/zh/getting-started/installation) |
| 平台协议 | [/platforms/protocols](https://lazygophers.github.io/aidog/zh/platforms/protocols) |
| 分组与路由 | [/groups/routing-rules](https://lazygophers.github.io/aidog/zh/groups/routing-rules) |
| 智能调度 | [/groups/scheduling](https://lazygophers.github.io/aidog/zh/groups/scheduling) |
| Codex 集成 | [/proxy/codex-integration](https://lazygophers.github.io/aidog/zh/proxy/codex-integration) |
| 中间件规则 | [/middleware](https://lazygophers.github.io/aidog/zh/middleware/) |
| 用量统计与定价 | [/stats/usage-stats](https://lazygophers.github.io/aidog/zh/stats/usage-stats) |
| API 接口 | [/api/api-reference](https://lazygophers.github.io/aidog/zh/api/api-reference) |

## 多语言 README

| 语言 | 文件 |
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

[GNU AGPL-3.0-or-later](LICENSE) © AiDog

本项目以 GNU Affero 通用公共许可证 v3 或更高版本授权。若你修改本软件并通过网络提供服务，须向用户公开对应源码。
