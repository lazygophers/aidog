# PRD: Claude Code 原始订阅平台支持（纯透传）

## 背景
支持 Claude Code 原始订阅：用户用 Claude Code CLI（已登录 claude.ai 订阅）把 CLI 指向 aidog 代理，aidog **纯透传** —— 客户端请求原样转发到上游，aidog 不碰认证/body/header。订阅 OAuth 凭证由客户端自带在请求 header 里。

## 核心语义（已确认）：纯透传 relay

| 项 | 行为 |
| --- | --- |
| body 转换 | **不转换**，原样转发客户端请求 body（跳过 convert_request） |
| header 转换 | **不转换**，原样转发客户端全部 header（含客户端自带的 Authorization OAuth；跳过 build_upstream_headers / apply_client_headers / ClientType 模拟） |
| 认证注入 | **不注入** api_key/token（客户端自带订阅 OAuth header） |
| OAuth 登录/refresh | **不做**（作废之前决策；aidog 不管理 token） |
| 路由 | 转发到 platform.base_url + 客户端原始 path/query |
| 请求记录 | **正常记** proxy_log（request/response/token/status 等） |
| 响应 | 原样透传（流式 + 非流式都原样 relay，不回转协议） |

## 涉及面（待调研精确化）
- 后端: Protocol enum 加 CC 订阅平台类型（命名 design 定，如 `claude_code`）；proxy.rs 加纯透传分支（类似 mock 在拦截点判 platform_type → bypass 所有转换，用原始 client method/path/header/body relay 到 base_url，记 log）
- 前端: 平台类型选项 + 配置（base_url；api_key 可空，因客户端自带认证）

## 关键技术点（调研）
- proxy.rs 原始 client header 捕获点（透传需原始 header，不能用 build_upstream_headers 重建）
- proxy.rs 原始 body bytes 捕获点（已读 body 提取 model + log，透传复用原始 bytes）
- Host header 处理（reqwest 转发到 base_url 时 Host 应为 upstream host，非 aidog；通常 reqwest 自动设）
- 透传分支插入点（convert_request 之前，类似 mock proxy.rs:340）
- proxy_log 记录：透传模式下 source_protocol/target_protocol 如何标（都 = 客户端协议，不转换）

## 验收标准
- CC 订阅平台类型可选 + 配 base_url
- 路由到 CC 平台：客户端请求 header/body 原样转发到 base_url（含客户端 Authorization）
- 响应原样透传（流式 + 非流式）
- proxy_log 正常记录
- cargo build + tsc + 测试通过

## 关联（独立后续 task，勿并入）
- **分组路由 AI 平台拖动排序**：group 内 platform priority 拖拽。用户已确认独立 task，本 task finish 后建。
