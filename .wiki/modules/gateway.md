# Gateway 模块详解

Gateway 是 AiDog 的 Rust 后端核心，包含 6 个主要模块。

## proxy.rs — 代理服务器

Axum HTTP 代理服务器，监听本地端口（默认 7860）。

### 核心职责
- HTTP 请求接收与转发
- SSE 流式透传
- 渐进式日志记录（请求开始时记录基础信息，完成时补充用量/费用）
- 超时级联（请求超时 → 故障转移 → 504）
- Log settings 感知（根据 ProxyLogSettings 控制记录粒度）

### 关键实现
- 使用 Axum router 注册所有 API 路径
- reqwest 客户端发送上游请求
- tokio 异步流处理 SSE

## router.rs — 路由引擎

分组匹配 + 模型映射 + 平台选择。

### 分组匹配
1. 遍历所有分组
2. 检查分组内模型映射表是否包含请求模型名
3. 命中 → 使用该分组
4. 未命中 → 默认分组 / 第一个分组

### 平台选择策略
- **顺序**：按排列顺序
- **随机**：随机选择
- **加权**：按权重比例

### 故障转移
当前平台失败（429/5xx/超时）→ 尝试组内下一个平台。

## db.rs — 数据库层

SQLite CRUD 操作。

### 主要表
- `platforms` — 平台配置
- `groups` — 分组配置
- `group_platforms` — 分组-平台关联
- `proxy_logs` — 代理日志
- `settings` — 应用设置

### 三级 retention 清理
- user_request_retention_days (7d) — UPDATE SET 清空字段
- upstream_request_retention_days (7d) — UPDATE SET 清空字段
- retention_days (90d) — DELETE 整行

## models.rs — 数据模型

所有 Rust 数据模型定义，包括：
- `Protocol` 枚举（53 变体）
- `Platform`、`Group`、`ProxyLog` 结构体
- `ProxyLogSettings` 三级日志控制
- `PlatformStats` 统计数据

## quota.rs — 余额查询

支持平台余额 & Coding Plan 配额查询。

### 支持的平台
- DeepSeek / StepFun / SiliconFlow / OpenRouter / Novita
- Kimi / GLM / MiniMax
- OpenAI
- NewAPI 兼容平台

### 查询方式
各平台 API 不同，quota.rs 封装了各平台的余额查询 HTTP 接口。

## estimate.rs — 费用估算

基于 `resolve_price` 回退链的费用计算。

### resolve_price 回退链
1. 自定义定价
2. 内置定价
3. 默认价

### est_cost 持久化
每次请求后，est_cost 写入 proxy_logs 表。today_stats 通过 `SUM(est_cost)` 聚合。
