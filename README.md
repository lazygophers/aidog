# AiDog — AI API Gateway Desktop App

桌面端 AI API 网关，管理和路由多平台 AI 请求。支持 50+ 平台预设、自动分组、模型映射、负载均衡/故障转移。

## 功能

- **平台管理**：50+ AI 平台预设（Anthropic/OpenAI/DeepSeek/GLM/Kimi/MiniMax 等），一键配置
- **智能分组**：按 Bearer token / 路径匹配请求，Failover / Load Balance 路由
- **模型映射**：透明替换模型名（如 claude-sonnet-4 → deepseek-chat）
- **代理转发**：支持 Anthropic/OpenAI/Gemini 协议双向转换
- **用量监控**：Token 统计、费用估算、余额查询、Coding Plan 配额展示
- **请求日志**：细粒度记录控制（用户请求/上游请求分别开关和保留期）

## 技术栈

- **桌面框架**：Tauri 2.0
- **前端**：React 19 + TypeScript + Vite
- **后端**：Rust（Axum 代理 + SQLite 存储）
- **国际化**：7 种语言，阿拉伯语 RTL
- **主题**：Liquid Glass 风格，每主题 light/dark 双模式

## 开发

```bash
# 安装依赖
yarn install

# 开发模式
yarn tauri dev

# 构建
yarn tauri build
```

## 推荐 IDE

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
