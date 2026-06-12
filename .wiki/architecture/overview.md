# 架构全景

## 分层架构

AiDog 采用 Tauri 2.0 的经典双层架构：

```
┌─────────────────────────────────────┐
│          React Frontend             │
│  (TypeScript + Vite + i18next)      │
│  pages/ components/ themes/         │
├─────────────────────────────────────┤
│          Tauri IPC Bridge           │
│  lib.rs (~50 invoke commands)       │
├─────────────────────────────────────┤
│          Rust Backend               │
│  gateway/ (Axum proxy + SQLite)     │
│  adapter/ (protocol conversion)     │
└─────────────────────────────────────┘
```

## 核心组件

### 前端（React 19）
- 无 react-router，导航是 `App.tsx` + `AppSettings.tsx` 的本地 state
- 离页拦截用 `utils/navGuard.ts` 注册表
- 数值格式化统一走 `utils/formatters.ts`
- i18n: i18next + 7 语言 JSON 文件

### Tauri IPC
- 约 50 个 `#[tauri::command]` 函数
- 前端通过 `@tauri-apps/api` 的 `invoke()` 调用
- 命令覆盖：CRUD 操作、设置读写、代理控制、余额查询

### 后端（Rust + Axum）
- **proxy.rs**: Axum HTTP 代理服务器（支持 SSE 流式、超时级联）
- **router.rs**: 分组匹配 + 模型映射 + 平台选择算法
- **db.rs**: SQLite CRUD（settings、proxy_logs、usage stats）
- **adapter/**: 协议转换（入站检测 → 内部格式 → 出站转换）
- **quota.rs**: 平台余额 & Coding Plan 配额查询
- **estimate.rs**: 基于 `resolve_price` 回退链的费用估算

## 请求流

```
客户端 → Axum proxy (127.0.0.1:7860)
  → 协议检测 (converter.rs detect)
  → 分组匹配 (router.rs)
  → 模型映射 (group → platform 级联)
  → 协议转换 (adapter/)
  → 上游平台请求
  → SSE 流式透传
  → 日志记录 + 费用估算
  → 返回客户端
```

## 数据存储

| 数据 | 路径 | 格式 |
|------|------|------|
| 应用数据库 | `~/.aidog/aidog.db` | SQLite |
| Codex 配置 | `~/.codex/config.toml` | TOML |

所有数据本地存储，不上传任何服务器。
